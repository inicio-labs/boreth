//! Seal verification: recover block signer via ECRECOVER.

use alloy_primitives::{Address, B256, Signature};

/// Errors during seal verification.
#[derive(Debug, thiserror::Error)]
pub enum SealError {
    #[error("invalid signature length: expected 65, got {0}")]
    InvalidSignatureLength(usize),
    #[error("recovery failed: {0}")]
    RecoveryFailed(String),
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
}
