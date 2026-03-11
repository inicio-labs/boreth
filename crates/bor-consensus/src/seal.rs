//! Seal verification: recover block signer via ECRECOVER.
//!
//! Also provides seal hash computation for Bor headers (keccak256 of the RLP-encoded
//! header with the 65-byte seal stripped from extra data).

use alloy_primitives::{Address, Bytes, B256, Signature, U256, keccak256};
use alloy_rlp::Encodable;
use bor_chainspec::constants::EXTRADATA_SEAL_LEN;
use reth_primitives_traits::BlockHeader;

/// Errors during seal verification.
#[derive(Debug, thiserror::Error)]
pub enum SealError {
    #[error("invalid signature length: expected 65, got {0}")]
    InvalidSignatureLength(usize),
    #[error("recovery failed: {0}")]
    RecoveryFailed(String),
}

/// Compute the seal hash for a Bor header.
///
/// The seal hash is `keccak256(RLP(header))` where the header's extra data has
/// the last 65 bytes (the ECDSA seal) stripped. This is what the block producer
/// signs when sealing a block.
///
/// The RLP encoding matches `alloy_consensus::Header`'s encoding exactly, but with
/// `extra_data` truncated before the seal.
pub fn compute_seal_hash<H: BlockHeader>(header: &H) -> B256 {
    let extra = header.extra_data();
    let trimmed_extra = Bytes::copy_from_slice(
        &extra[..extra.len().saturating_sub(EXTRADATA_SEAL_LEN)],
    );

    // Build RLP list content — matches alloy_consensus::Header::encode() exactly
    let mut list_content = Vec::with_capacity(700);
    header.parent_hash().encode(&mut list_content);
    header.ommers_hash().encode(&mut list_content);
    header.beneficiary().encode(&mut list_content);
    header.state_root().encode(&mut list_content);
    header.transactions_root().encode(&mut list_content);
    header.receipts_root().encode(&mut list_content);
    header.logs_bloom().encode(&mut list_content);
    header.difficulty().encode(&mut list_content);
    // alloy Header encodes number/gas_limit/gas_used as U256
    U256::from(header.number()).encode(&mut list_content);
    U256::from(header.gas_limit()).encode(&mut list_content);
    U256::from(header.gas_used()).encode(&mut list_content);
    header.timestamp().encode(&mut list_content);
    trimmed_extra.encode(&mut list_content);
    // mix_hash and nonce are always present in Bor headers
    header.mix_hash().unwrap_or_default().encode(&mut list_content);
    header.nonce().unwrap_or_default().encode(&mut list_content);

    // base_fee_per_gas (post-London, always present on Polygon)
    if let Some(base_fee) = header.base_fee_per_gas() {
        U256::from(base_fee).encode(&mut list_content);
    }

    // Bor does not use these post-Shanghai/Cancun fields, but encode if present
    // to stay compatible with the standard header encoding
    if let Some(root) = header.withdrawals_root() {
        root.encode(&mut list_content);
    }
    if let Some(blob_gas_used) = header.blob_gas_used() {
        U256::from(blob_gas_used).encode(&mut list_content);
    }
    if let Some(excess_blob_gas) = header.excess_blob_gas() {
        U256::from(excess_blob_gas).encode(&mut list_content);
    }
    if let Some(root) = header.parent_beacon_block_root() {
        root.encode(&mut list_content);
    }
    if let Some(hash) = header.requests_hash() {
        hash.encode(&mut list_content);
    }

    // Wrap in RLP list header
    let mut buf = Vec::with_capacity(list_content.len() + 5);
    alloy_rlp::Header { list: true, payload_length: list_content.len() }.encode(&mut buf);
    buf.extend_from_slice(&list_content);

    keccak256(&buf)
}

/// Recover the signer address from a seal hash and 65-byte signature.
///
/// The signature layout is `[r (32 bytes) | s (32 bytes) | v (1 byte)]`.
/// The seal hash is keccak256 of the RLP-encoded header without the seal bytes.
pub fn ecrecover_seal(seal_hash: &B256, signature: &[u8]) -> Result<Address, SealError> {
    if signature.len() != 65 {
        return Err(SealError::InvalidSignatureLength(signature.len()));
    }

    let sig = Signature::from_raw(signature)
        .map_err(|e| SealError::RecoveryFailed(e.to_string()))?;

    sig.recover_address_from_prehash(seal_hash)
        .map_err(|e| SealError::RecoveryFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_signature_length() {
        let hash = B256::ZERO;
        let short_sig = [0u8; 64];
        let err = ecrecover_seal(&hash, &short_sig).unwrap_err();
        assert!(matches!(err, SealError::InvalidSignatureLength(64)));
    }

    #[test]
    fn test_zero_signature_recovery_fails() {
        let hash = B256::ZERO;
        let sig = [0u8; 65];
        // Zero signature should fail recovery
        assert!(ecrecover_seal(&hash, &sig).is_err());
    }

    #[test]
    fn test_valid_signature_recovery() {
        // Known test vector: sign a hash with a known private key
        // Using the secp256k1 identity: signing keccak256("test") with a known key
        use alloy_primitives::keccak256;
        use k256::ecdsa::SigningKey;

        let msg_hash = keccak256(b"test message for seal verification");

        // Create a signing key from deterministic bytes
        let secret_bytes: [u8; 32] = keccak256(b"test secret key").0;
        let signing_key = SigningKey::from_bytes((&secret_bytes).into()).unwrap();
        let (sig, recid) = signing_key.sign_prehash_recoverable(msg_hash.as_ref()).unwrap();

        let mut sig_bytes = [0u8; 65];
        sig_bytes[..64].copy_from_slice(&sig.to_bytes());
        sig_bytes[64] = recid.to_byte();

        let recovered = ecrecover_seal(&msg_hash, &sig_bytes).unwrap();

        // Derive expected address from public key
        let verify_key = signing_key.verifying_key();
        let pubkey_bytes = verify_key.to_encoded_point(false);
        let expected_addr = Address::from_raw_public_key(&pubkey_bytes.as_bytes()[1..]);

        assert_eq!(recovered, expected_addr);
    }

    #[test]
    fn test_compute_seal_hash_deterministic() {
        use alloy_consensus::Header;
        use alloy_consensus::EMPTY_OMMER_ROOT_HASH;
        use alloy_primitives::{Bytes, B64};

        // Create a Bor-like header with 97 bytes of extra data (32 vanity + 65 seal)
        let header = Header {
            parent_hash: B256::ZERO,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            number: 100,
            gas_limit: 30_000_000,
            gas_used: 1_000_000,
            timestamp: 1700000000,
            nonce: B64::ZERO,
            extra_data: Bytes::from(vec![0u8; 97]),
            base_fee_per_gas: Some(7),
            ..Default::default()
        };

        // Seal hash should be deterministic
        let hash1 = compute_seal_hash(&header);
        let hash2 = compute_seal_hash(&header);
        assert_eq!(hash1, hash2, "seal hash must be deterministic");

        // Seal hash should differ from block hash (because seal is included in block hash)
        let block_hash = alloy_primitives::Sealable::hash_slow(&header);
        assert_ne!(hash1, block_hash, "seal hash must differ from block hash");
    }

    #[test]
    fn test_seal_hash_round_trip_with_ecrecover() {
        use alloy_consensus::Header;
        use alloy_consensus::EMPTY_OMMER_ROOT_HASH;
        use alloy_primitives::{Bytes, B64};
        use k256::ecdsa::SigningKey;

        // Create a signing key
        let secret_bytes: [u8; 32] = keccak256(b"bor validator key").0;
        let signing_key = SigningKey::from_bytes((&secret_bytes).into()).unwrap();
        let verify_key = signing_key.verifying_key();
        let signer_addr = Address::from_raw_public_key(
            &verify_key.to_encoded_point(false).as_bytes()[1..],
        );

        // Create header with placeholder extra data (32 vanity + 65 zero seal)
        let mut extra = vec![0u8; 97];

        // Compute seal hash first with zero seal
        let header = Header {
            parent_hash: B256::ZERO,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            number: 42,
            gas_limit: 30_000_000,
            timestamp: 1700000000,
            nonce: B64::ZERO,
            extra_data: Bytes::from(extra.clone()),
            base_fee_per_gas: Some(7),
            ..Default::default()
        };
        let seal_hash = compute_seal_hash(&header);

        // Sign the seal hash
        let (sig, recid) = signing_key.sign_prehash_recoverable(seal_hash.as_ref()).unwrap();
        let mut seal_bytes = [0u8; 65];
        seal_bytes[..64].copy_from_slice(&sig.to_bytes());
        seal_bytes[64] = recid.to_byte();

        // Put the seal into extra data
        extra[32..97].copy_from_slice(&seal_bytes);

        // Create the final header with real seal
        let header_with_seal = Header {
            extra_data: Bytes::from(extra),
            ..header
        };

        // The seal hash should be the same regardless of seal content
        // (seal is stripped before hashing)
        let seal_hash2 = compute_seal_hash(&header_with_seal);
        assert_eq!(seal_hash, seal_hash2, "seal hash must not depend on seal bytes");

        // Recover signer from the seal
        let recovered = ecrecover_seal(&seal_hash2, &seal_bytes).unwrap();
        assert_eq!(recovered, signer_addr, "recovered signer must match original key");
    }
}
