//! Bor difficulty calculation (INTURN/NOTURN).
//!
//! In Bor, difficulty determines fork choice priority:
//! - INTURN (primary proposer): difficulty = validator_set_length
//! - NOTURN (backup proposer): difficulty = validator_set_length - distance
//! - Minimum difficulty is always 1.

use alloy_primitives::{Address, U256};

/// Difficulty assigned to the in-turn (primary) proposer.
/// Equal to the length of the validator set.
pub fn diff_inturn(validator_count: usize) -> U256 {
    U256::from(validator_count)
}

/// Difficulty assigned to an out-of-turn (backup) proposer at the given distance.
/// Equal to `validator_count - distance`, clamped to a minimum of 1.
pub fn diff_noturn(validator_count: usize, distance: usize) -> U256 {
    let diff = if distance >= validator_count {
        1
    } else {
        validator_count - distance
    };
    U256::from(diff.max(1))
}

/// Returns `true` if the given signer is the in-turn proposer for the block.
///
/// The in-turn signer is determined by `block_number % validator_count`.
pub fn is_inturn(signer: &Address, validators: &[Address], block_number: u64) -> bool {
    if validators.is_empty() {
        return false;
    }
    let idx = (block_number as usize) % validators.len();
    &validators[idx] == signer
}

/// Calculate the difficulty for a block given the signer and the ordered validator set.
///
/// If the signer is the in-turn proposer: difficulty = validator_count.
/// Otherwise: difficulty = validator_count - distance_from_inturn, minimum 1.
pub fn calculate_difficulty(
    signer: &Address,
    validators: &[Address],
    block_number: u64,
) -> U256 {
    if validators.is_empty() {
        return U256::from(1);
    }

    let count = validators.len();
    let inturn_idx = (block_number as usize) % count;

    // Find the signer's position
    if let Some(signer_idx) = validators.iter().position(|v| v == signer) {
        if signer_idx == inturn_idx {
            diff_inturn(count)
        } else {
            // Distance is the circular distance from the in-turn position
            let distance = if signer_idx > inturn_idx {
                signer_idx - inturn_idx
            } else {
                count - inturn_idx + signer_idx
            };
            diff_noturn(count, distance)
        }
    } else {
        // Signer not in validator set — minimum difficulty
        U256::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;

    fn make_validators(count: usize) -> Vec<Address> {
        (0..count)
            .map(|i| Address::new([(i + 1) as u8; 20]))
            .collect()
    }

    #[test]
    fn test_diff_inturn() {
        assert_eq!(diff_inturn(5), U256::from(5));
        assert_eq!(diff_inturn(1), U256::from(1));
        assert_eq!(diff_inturn(100), U256::from(100));
    }

    #[test]
    fn test_diff_noturn() {
        // Distance 1 from set of 5: 5 - 1 = 4
        assert_eq!(diff_noturn(5, 1), U256::from(4));
        // Distance 4 from set of 5: 5 - 4 = 1
        assert_eq!(diff_noturn(5, 4), U256::from(1));
    }

    #[test]
    fn test_diff_minimum() {
        // Distance >= count should clamp to 1
        assert_eq!(diff_noturn(5, 5), U256::from(1));
        assert_eq!(diff_noturn(5, 10), U256::from(1));
        assert_eq!(diff_noturn(1, 1), U256::from(1));
    }

    #[test]
    fn test_is_inturn() {
        let validators = make_validators(3);
        // block 0 -> idx 0
        assert!(is_inturn(&validators[0], &validators, 0));
        assert!(!is_inturn(&validators[1], &validators, 0));
        // block 1 -> idx 1
        assert!(is_inturn(&validators[1], &validators, 1));
        // block 2 -> idx 2
        assert!(is_inturn(&validators[2], &validators, 2));
        // block 3 -> idx 0 (wraps)
        assert!(is_inturn(&validators[0], &validators, 3));
    }

    #[test]
    fn test_calculate_difficulty_inturn() {
        let validators = make_validators(5);
        // Signer at idx 0 for block 0 (in-turn) -> difficulty = 5
        let diff = calculate_difficulty(&validators[0], &validators, 0);
        assert_eq!(diff, U256::from(5));
    }

    #[test]
    fn test_calculate_difficulty_noturn() {
        let validators = make_validators(5);
        // Block 0: inturn_idx = 0
        // Signer at idx 1: distance = 1, difficulty = 5 - 1 = 4
        let diff = calculate_difficulty(&validators[1], &validators, 0);
        assert_eq!(diff, U256::from(4));

        // Signer at idx 4: distance = 4, difficulty = 5 - 4 = 1
        let diff = calculate_difficulty(&validators[4], &validators, 0);
        assert_eq!(diff, U256::from(1));
    }

    #[test]
    fn test_diff_at_span_boundary() {
        // When validator set changes at span boundary, difficulty uses NEW set length
        let small_set = make_validators(3);
        let large_set = make_validators(10);

        // block 6400 % 3 = 1, so validators[1] is inturn for small_set
        let diff_small = calculate_difficulty(&small_set[1], &small_set, 6400);
        // block 6400 % 10 = 0, so validators[0] is inturn for large_set
        let diff_large = calculate_difficulty(&large_set[0], &large_set, 6400);

        assert_eq!(diff_small, U256::from(3));
        assert_eq!(diff_large, U256::from(10));
    }

    #[test]
    fn test_signer_not_in_set() {
        let validators = make_validators(3);
        let unknown = Address::new([0xff; 20]);
        let diff = calculate_difficulty(&unknown, &validators, 0);
        assert_eq!(diff, U256::from(1));
    }

    #[test]
    fn test_empty_validators() {
        let diff = calculate_difficulty(&Address::ZERO, &[], 0);
        assert_eq!(diff, U256::from(1));
        assert!(!is_inturn(&Address::ZERO, &[], 0));
    }

    #[test]
    fn test_circular_distance() {
        let validators = make_validators(5);
        // Block 3: inturn_idx = 3
        // Signer at idx 1: distance = 5 - 3 + 1 = 3, difficulty = 5 - 3 = 2
        let diff = calculate_difficulty(&validators[1], &validators, 3);
        assert_eq!(diff, U256::from(2));
    }
}
