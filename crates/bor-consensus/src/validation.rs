//! Bor consensus header and block validation.
//!
//! Implements Bor-specific consensus rules for header validation:
//! - Nonce must be zero
//! - MixHash must be zero
//! - Ommers must be empty
//! - Extra data must be valid (vanity + optional validators + seal)
//! - Difficulty must match INTURN/NOTURN calculation
//! - Block timestamp must not be too far in the future
//! - Signer must be authorized (in the current validator set)
//! - Signer must not have signed recently (anti-double-sign)

use alloy_primitives::{Address, B256, U256};
use crate::difficulty::calculate_difficulty;
use crate::extra_data::ExtraData;
use crate::seal::ecrecover_seal;

/// Maximum allowed clock drift for block timestamps (15 seconds).
const MAX_FUTURE_BLOCK_TIME: u64 = 15;

/// Errors during consensus validation.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("non-zero nonce: expected 0x0000000000000000")]
    NonZeroNonce,
    #[error("non-zero mix hash")]
    NonZeroMixHash,
    #[error("non-empty ommers list")]
    NonEmptyOmmers,
    #[error("invalid extra data: {0}")]
    InvalidExtraData(String),
    #[error("wrong difficulty: expected {expected}, got {got}")]
    WrongDifficulty { expected: U256, got: U256 },
    #[error("block timestamp {block_time} is too far in the future (now={now})")]
    FutureBlock { block_time: u64, now: u64 },
    #[error("unauthorized signer: {0}")]
    UnauthorizedSigner(Address),
    #[error("signer {0} signed recently")]
    RecentlySigned(Address),
    #[error("seal recovery failed: {0}")]
    SealError(String),
    #[error("invalid gas limit: expected {expected}, got {got}")]
    InvalidGasLimit { expected: u64, got: u64 },
    #[error("invalid base fee: expected {expected}, got {got}")]
    InvalidBaseFee { expected: U256, got: U256 },
    #[error("block timestamp must be greater than parent: block={block_time}, parent={parent_time}")]
    TimestampNotIncreasing { block_time: u64, parent_time: u64 },
    #[error("non-empty withdrawals")]
    NonEmptyWithdrawals,
    #[error("missing validators at span start block {0}")]
    MissingValidatorsAtSpanStart(u64),
    #[error("state root mismatch: expected {expected}, got {got}")]
    StateRootMismatch { expected: B256, got: B256 },
    #[error("receipt root mismatch: expected {expected}, got {got}")]
    ReceiptRootMismatch { expected: B256, got: B256 },
    #[error("gas used mismatch: expected {expected}, got {got}")]
    GasUsedMismatch { expected: u64, got: u64 },
}

/// Parameters for validating a Bor header.
pub struct HeaderValidationParams {
    /// The block number.
    pub number: u64,
    /// The block timestamp.
    pub timestamp: u64,
    /// The nonce (must be zero).
    pub nonce: u64,
    /// The mix hash (must be zero).
    pub mix_hash: B256,
    /// The difficulty.
    pub difficulty: U256,
    /// The extra data bytes.
    pub extra_data: Vec<u8>,
    /// The gas limit.
    pub gas_limit: u64,
    /// The seal hash (keccak256 of RLP-encoded header without seal).
    pub seal_hash: B256,
    /// Whether the block has ommers.
    pub has_ommers: bool,
}

/// Parameters for parent-based validation.
pub struct ParentValidationParams {
    /// The parent block timestamp.
    pub parent_timestamp: u64,
}

/// Validate a Bor block header (standalone checks, no parent needed).
pub fn validate_header(
    params: &HeaderValidationParams,
    authorized_signers: &[Address],
    recent_signers: &std::collections::BTreeMap<u64, Address>,
    current_time: u64,
) -> Result<Address, ValidationError> {
    // 1. Nonce must be zero
    if params.nonce != 0 {
        return Err(ValidationError::NonZeroNonce);
    }

    // 2. Mix hash must be zero
    if params.mix_hash != B256::ZERO {
        return Err(ValidationError::NonZeroMixHash);
    }

    // 3. No ommers
    if params.has_ommers {
        return Err(ValidationError::NonEmptyOmmers);
    }

    // 4. Parse extra data
    let extra = ExtraData::parse(&params.extra_data)
        .map_err(|e| ValidationError::InvalidExtraData(e.to_string()))?;

    // 5. Block timestamp must not be too far in the future
    if params.timestamp > current_time + MAX_FUTURE_BLOCK_TIME {
        return Err(ValidationError::FutureBlock {
            block_time: params.timestamp,
            now: current_time,
        });
    }

    // 6. Recover signer from seal
    let signer = ecrecover_seal(&params.seal_hash, &extra.seal)
        .map_err(|e| ValidationError::SealError(e.to_string()))?;

    // 7. Verify signer is authorized
    if !authorized_signers.contains(&signer) {
        return Err(ValidationError::UnauthorizedSigner(signer));
    }

    // 8. Anti-double-sign: signer must not have signed recently
    let limit = (authorized_signers.len() / 2 + 1) as u64;
    let cutoff = params.number.saturating_sub(limit);
    for (&_block, &recent_signer) in recent_signers.range(cutoff..params.number) {
        if recent_signer == signer {
            return Err(ValidationError::RecentlySigned(signer));
        }
    }

    // 9. Verify difficulty
    let expected_diff = calculate_difficulty(&signer, authorized_signers, params.number);
    if params.difficulty != expected_diff {
        return Err(ValidationError::WrongDifficulty {
            expected: expected_diff,
            got: params.difficulty,
        });
    }

    Ok(signer)
}

/// Validate a header against its parent.
pub fn validate_header_against_parent(
    params: &HeaderValidationParams,
    parent: &ParentValidationParams,
) -> Result<(), ValidationError> {
    // Timestamp must be strictly increasing
    if params.timestamp <= parent.parent_timestamp {
        return Err(ValidationError::TimestampNotIncreasing {
            block_time: params.timestamp,
            parent_time: parent.parent_timestamp,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{keccak256, Address};
    use std::collections::BTreeMap;



    fn make_extra_data_with_seal(seal: &[u8; 65]) -> Vec<u8> {
        let mut data = vec![0u8; 32]; // vanity
        data.extend_from_slice(seal); // seal
        data
    }

    #[test]
    fn test_reject_nonzero_nonce() {
        let params = HeaderValidationParams {
            number: 100,
            timestamp: 1000,
            nonce: 1, // Non-zero!
            mix_hash: B256::ZERO,
            difficulty: U256::from(1),
            extra_data: vec![0u8; 97],
            gas_limit: 30_000_000,
            seal_hash: B256::ZERO,
            has_ommers: false,
        };
        let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
        assert!(matches!(err, ValidationError::NonZeroNonce));
    }

    #[test]
    fn test_reject_nonzero_mixhash() {
        let params = HeaderValidationParams {
            number: 100,
            timestamp: 1000,
            nonce: 0,
            mix_hash: B256::from([0xff; 32]), // Non-zero!
            difficulty: U256::from(1),
            extra_data: vec![0u8; 97],
            gas_limit: 30_000_000,
            seal_hash: B256::ZERO,
            has_ommers: false,
        };
        let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
        assert!(matches!(err, ValidationError::NonZeroMixHash));
    }

    #[test]
    fn test_reject_nonempty_ommers() {
        let params = HeaderValidationParams {
            number: 100,
            timestamp: 1000,
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty: U256::from(1),
            extra_data: vec![0u8; 97],
            gas_limit: 30_000_000,
            seal_hash: B256::ZERO,
            has_ommers: true, // Has ommers!
        };
        let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
        assert!(matches!(err, ValidationError::NonEmptyOmmers));
    }

    #[test]
    fn test_reject_future_block() {
        let params = HeaderValidationParams {
            number: 100,
            timestamp: 2000, // Far in the future
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty: U256::from(1),
            extra_data: vec![0u8; 97],
            gas_limit: 30_000_000,
            seal_hash: B256::ZERO,
            has_ommers: false,
        };
        let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
        assert!(matches!(err, ValidationError::FutureBlock { .. }));
    }

    #[test]
    fn test_reject_wrong_difficulty() {
        // Use a real key to create a valid signature
        use k256::ecdsa::SigningKey;

        let secret_bytes: [u8; 32] = keccak256(b"test_difficulty_key").0;
        let signing_key = SigningKey::from_bytes((&secret_bytes).into()).unwrap();
        let verify_key = signing_key.verifying_key();
        let signer_addr = Address::from_raw_public_key(
            &verify_key.to_encoded_point(false).as_bytes()[1..],
        );

        let seal_hash = keccak256(b"test seal hash for difficulty");
        let (sig, recid) = signing_key.sign_prehash_recoverable(seal_hash.as_ref()).unwrap();
        let mut seal = [0u8; 65];
        seal[..64].copy_from_slice(&sig.to_bytes());
        seal[64] = recid.to_byte();

        let extra_data = make_extra_data_with_seal(&seal);
        let signers = vec![signer_addr];

        let params = HeaderValidationParams {
            number: 0,
            timestamp: 1000,
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty: U256::from(999), // Wrong!
            extra_data,
            gas_limit: 30_000_000,
            seal_hash,
            has_ommers: false,
        };

        let err = validate_header(&params, &signers, &BTreeMap::new(), 1000).unwrap_err();
        assert!(matches!(err, ValidationError::WrongDifficulty { .. }));
    }

    #[test]
    fn test_validate_known_header() {
        // Create a valid header with known key
        use k256::ecdsa::SigningKey;

        let secret_bytes: [u8; 32] = keccak256(b"known_validator_key").0;
        let signing_key = SigningKey::from_bytes((&secret_bytes).into()).unwrap();
        let verify_key = signing_key.verifying_key();
        let signer_addr = Address::from_raw_public_key(
            &verify_key.to_encoded_point(false).as_bytes()[1..],
        );

        let seal_hash = keccak256(b"known block seal hash");
        let (sig, recid) = signing_key.sign_prehash_recoverable(seal_hash.as_ref()).unwrap();
        let mut seal = [0u8; 65];
        seal[..64].copy_from_slice(&sig.to_bytes());
        seal[64] = recid.to_byte();

        let extra_data = make_extra_data_with_seal(&seal);
        let signers = vec![signer_addr];

        // Signer at idx 0, block 0: inturn, difficulty = 1
        let params = HeaderValidationParams {
            number: 0,
            timestamp: 1000,
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty: U256::from(1), // inturn with 1 validator
            extra_data,
            gas_limit: 30_000_000,
            seal_hash,
            has_ommers: false,
        };

        let recovered = validate_header(&params, &signers, &BTreeMap::new(), 1000).unwrap();
        assert_eq!(recovered, signer_addr);
    }

    #[test]
    fn test_validate_header_against_parent_ok() {
        let params = HeaderValidationParams {
            number: 101,
            timestamp: 1002,
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty: U256::from(1),
            extra_data: vec![0u8; 97],
            gas_limit: 30_000_000,
            seal_hash: B256::ZERO,
            has_ommers: false,
        };
        let parent = ParentValidationParams {
            parent_timestamp: 1000,
        };
        validate_header_against_parent(&params, &parent).unwrap();
    }

    #[test]
    fn test_validate_header_against_parent_reject_non_increasing() {
        let params = HeaderValidationParams {
            number: 101,
            timestamp: 999, // Before parent
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty: U256::from(1),
            extra_data: vec![0u8; 97],
            gas_limit: 30_000_000,
            seal_hash: B256::ZERO,
            has_ommers: false,
        };
        let parent = ParentValidationParams {
            parent_timestamp: 1000,
        };
        let err = validate_header_against_parent(&params, &parent).unwrap_err();
        assert!(matches!(err, ValidationError::TimestampNotIncreasing { .. }));
    }

    #[test]
    fn test_reject_unauthorized_signer() {
        use k256::ecdsa::SigningKey;

        let secret_bytes: [u8; 32] = keccak256(b"unauthorized_key").0;
        let signing_key = SigningKey::from_bytes((&secret_bytes).into()).unwrap();

        let seal_hash = keccak256(b"unauthorized block");
        let (sig, recid) = signing_key.sign_prehash_recoverable(seal_hash.as_ref()).unwrap();
        let mut seal = [0u8; 65];
        seal[..64].copy_from_slice(&sig.to_bytes());
        seal[64] = recid.to_byte();

        let extra_data = make_extra_data_with_seal(&seal);
        // Empty signer set — no one is authorized
        let signers: Vec<Address> = vec![Address::new([0xaa; 20])];

        let params = HeaderValidationParams {
            number: 0,
            timestamp: 1000,
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty: U256::from(1),
            extra_data,
            gas_limit: 30_000_000,
            seal_hash,
            has_ommers: false,
        };

        let err = validate_header(&params, &signers, &BTreeMap::new(), 1000).unwrap_err();
        assert!(matches!(err, ValidationError::UnauthorizedSigner(_)));
    }
}
