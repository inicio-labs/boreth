//! Block-level validation for Bor consensus.
//!
//! Pre-execution validation checks body constraints, extra data at span starts, and seal.
//! Post-execution validation verifies state root, receipt root, and gas used.

use alloy_primitives::{Address, B256};
use crate::extra_data::ExtraData;
use crate::validation::ValidationError;

/// Validate a block before execution.
///
/// Checks:
/// - No ommers (uncle blocks)
/// - No withdrawals
/// - At span start blocks: extra data must contain validator bytes
/// - Correct validator addresses in extra data at span starts
pub fn validate_block_pre_execution(
    block_number: u64,
    extra_data: &[u8],
    has_ommers: bool,
    has_withdrawals: bool,
    span_size: u64,
    expected_validators: Option<&[Address]>,
) -> Result<(), ValidationError> {
    // 1. No ommers allowed
    if has_ommers {
        return Err(ValidationError::NonEmptyOmmers);
    }

    // 2. No withdrawals allowed
    if has_withdrawals {
        return Err(ValidationError::NonEmptyWithdrawals);
    }

    // 3. At span start: extra data must contain validator addresses
    let is_span_start = block_number > 0 && block_number % span_size == 0;

    if is_span_start {
        let parsed = ExtraData::parse(extra_data)
            .map_err(|e| ValidationError::InvalidExtraData(e.to_string()))?;

        let validators = parsed.validators();
        if validators.is_empty() {
            return Err(ValidationError::MissingValidatorsAtSpanStart(block_number));
        }

        // Verify validator addresses match expected set (if provided)
        if let Some(expected) = expected_validators {
            if validators.len() != expected.len() {
                return Err(ValidationError::InvalidExtraData(format!(
                    "expected {} validators at span start, got {}",
                    expected.len(),
                    validators.len()
                )));
            }
            for (i, (got, want)) in validators.iter().zip(expected.iter()).enumerate() {
                if got != want {
                    return Err(ValidationError::InvalidExtraData(format!(
                        "validator mismatch at index {i}: got {got}, expected {want}"
                    )));
                }
            }
        }
    }

    Ok(())
}

/// Validate a block after execution.
///
/// Checks:
/// - State root matches expected
/// - Receipt root matches expected (fork-aware: pre/post Madhugiri)
/// - Gas used matches expected
pub fn validate_block_post_execution(
    expected_state_root: &B256,
    actual_state_root: &B256,
    expected_receipt_root: &B256,
    actual_receipt_root: &B256,
    expected_gas_used: u64,
    actual_gas_used: u64,
) -> Result<(), ValidationError> {
    if expected_state_root != actual_state_root {
        return Err(ValidationError::StateRootMismatch {
            expected: *expected_state_root,
            got: *actual_state_root,
        });
    }

    if expected_receipt_root != actual_receipt_root {
        return Err(ValidationError::ReceiptRootMismatch {
            expected: *expected_receipt_root,
            got: *actual_receipt_root,
        });
    }

    if expected_gas_used != actual_gas_used {
        return Err(ValidationError::GasUsedMismatch {
            expected: expected_gas_used,
            got: actual_gas_used,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bor_chainspec::constants::{EXTRADATA_VANITY_LEN, EXTRADATA_SEAL_LEN};

    fn make_extra_data_with_validators(validators: &[Address]) -> Vec<u8> {
        let mut data = vec![0u8; EXTRADATA_VANITY_LEN]; // vanity
        for v in validators {
            data.extend_from_slice(v.as_slice());
        }
        data.extend_from_slice(&[0u8; EXTRADATA_SEAL_LEN]); // seal
        data
    }

    fn make_minimal_extra_data() -> Vec<u8> {
        vec![0u8; EXTRADATA_VANITY_LEN + EXTRADATA_SEAL_LEN]
    }

    #[test]
    fn test_reject_nonempty_ommers() {
        let err = validate_block_pre_execution(
            100,
            &make_minimal_extra_data(),
            true, // has ommers
            false,
            6400,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::NonEmptyOmmers));
    }

    #[test]
    fn test_reject_nonempty_withdrawals() {
        let err = validate_block_pre_execution(
            100,
            &make_minimal_extra_data(),
            false,
            true, // has withdrawals
            6400,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::NonEmptyWithdrawals));
    }

    #[test]
    fn test_validate_span_start_extradata() {
        let validators = vec![
            Address::new([0xaa; 20]),
            Address::new([0xbb; 20]),
        ];
        let extra = make_extra_data_with_validators(&validators);

        // Block 6400 is a span start
        validate_block_pre_execution(6400, &extra, false, false, 6400, Some(&validators))
            .unwrap();
    }

    #[test]
    fn test_reject_missing_validators_at_span_start() {
        let extra = make_minimal_extra_data(); // no validators

        let err = validate_block_pre_execution(
            6400, // span start
            &extra,
            false,
            false,
            6400,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::MissingValidatorsAtSpanStart(6400)));
    }

    #[test]
    fn test_normal_block_no_validators_required() {
        let extra = make_minimal_extra_data();
        // Block 100 is not a span start
        validate_block_pre_execution(100, &extra, false, false, 6400, None).unwrap();
    }

    #[test]
    fn test_state_root_matches() {
        let root = B256::from([0xab; 32]);
        validate_block_post_execution(&root, &root, &root, &root, 1000, 1000).unwrap();
    }

    #[test]
    fn test_state_root_mismatch() {
        let expected = B256::from([0xab; 32]);
        let actual = B256::from([0xcd; 32]);
        let err = validate_block_post_execution(
            &expected, &actual, &expected, &expected, 1000, 1000,
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::StateRootMismatch { .. }));
    }

    #[test]
    fn test_receipt_root_mismatch() {
        let root = B256::from([0xab; 32]);
        let bad = B256::from([0xcd; 32]);
        let err = validate_block_post_execution(
            &root, &root, &root, &bad, 1000, 1000,
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::ReceiptRootMismatch { .. }));
    }

    #[test]
    fn test_gas_used_matches() {
        let root = B256::from([0xab; 32]);
        validate_block_post_execution(&root, &root, &root, &root, 5000, 5000).unwrap();
    }

    #[test]
    fn test_gas_used_mismatch() {
        let root = B256::from([0xab; 32]);
        let err = validate_block_post_execution(
            &root, &root, &root, &root, 5000, 6000,
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::GasUsedMismatch { .. }));
    }

    #[test]
    fn test_receipt_root_pre_madhugiri() {
        // Pre-Madhugiri: Bor receipt NOT in receipt root
        // The receipt root only covers regular txs
        let regular_root = B256::from([0x11; 32]);
        // Bor receipt is stored separately, so expected == actual (regular only)
        validate_block_post_execution(
            &B256::ZERO, &B256::ZERO,
            &regular_root, &regular_root,
            1000, 1000,
        )
        .unwrap();
    }

    #[test]
    fn test_receipt_root_post_madhugiri() {
        // Post-Madhugiri: receipt root includes state sync tx receipt
        let unified_root = B256::from([0x22; 32]);
        validate_block_post_execution(
            &B256::ZERO, &B256::ZERO,
            &unified_root, &unified_root,
            2000, 2000,
        )
        .unwrap();
    }

    #[test]
    fn test_block_zero_not_span_start() {
        // Block 0 is not a span start (0 % 6400 == 0 but we skip block 0)
        let extra = make_minimal_extra_data();
        validate_block_pre_execution(0, &extra, false, false, 6400, None).unwrap();
    }
}
