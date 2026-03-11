//! CometBFT/Tendermint weighted round-robin proposer selection algorithm.

use alloy_primitives::Address;
use bor_primitives::ValidatorSet;

/// Select the next proposer using CometBFT weighted round-robin.
///
/// The algorithm:
/// 1. Increment every validator's `proposer_priority` by their `voting_power`.
/// 2. The validator with the highest `proposer_priority` is selected as proposer.
/// 3. Subtract the total voting power from the selected proposer's priority.
/// 4. Update the proposer field on the validator set and return the address.
pub fn select_proposer(validator_set: &mut ValidatorSet) -> Address {
    let total_voting_power: i64 = validator_set
        .validators
        .iter()
        .map(|v| v.voting_power)
        .sum();

    // Step 1: increment all priorities by voting_power
    for v in validator_set.validators.iter_mut() {
        v.proposer_priority += v.voting_power;
    }

    // Step 2: find validator with highest proposer_priority
    let selected_idx = validator_set
        .validators
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| v.proposer_priority)
        .map(|(i, _)| i)
        .expect("validator set must not be empty");

    // Step 3: subtract total voting power from selected proposer
    validator_set.validators[selected_idx].proposer_priority -= total_voting_power;

    // Step 4: set proposer and return address
    let proposer = validator_set.validators[selected_idx].clone();
    let address = proposer.signer;
    validator_set.proposer = Some(proposer);

    address
}

/// Get the block producer for a specific sprint within a span.
///
/// Runs `select_proposer` for the given sprint, advancing the validator set state.
pub fn get_sprint_producer(
    validator_set: &mut ValidatorSet,
    _sprint_number: u64,
) -> Address {
    select_proposer(validator_set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use bor_primitives::Validator;

    fn make_validator(id: u64, addr_byte: u8, power: i64) -> Validator {
        Validator {
            id,
            address: Address::new([addr_byte; 20]),
            voting_power: power,
            signer: Address::new([addr_byte; 20]),
            proposer_priority: 0,
        }
    }

    fn make_validator_set(validators: Vec<Validator>) -> ValidatorSet {
        ValidatorSet {
            validators,
            proposer: None,
        }
    }

    #[test]
    fn test_proposer_single_validator() {
        let v = make_validator(1, 0xaa, 100);
        let mut vs = make_validator_set(vec![v]);

        let proposer = select_proposer(&mut vs);
        assert_eq!(proposer, Address::new([0xaa; 20]));

        // Single validator is always selected
        let proposer2 = select_proposer(&mut vs);
        assert_eq!(proposer2, Address::new([0xaa; 20]));
    }

    #[test]
    fn test_proposer_equal_power() {
        let v1 = make_validator(1, 0xaa, 100);
        let v2 = make_validator(2, 0xbb, 100);
        let v3 = make_validator(3, 0xcc, 100);
        let mut vs = make_validator_set(vec![v1, v2, v3]);

        // With equal power, each validator should be selected once per cycle
        let mut selected = Vec::new();
        for _ in 0..3 {
            selected.push(select_proposer(&mut vs));
        }

        // All three validators should appear exactly once in a full cycle
        let addr_a = Address::new([0xaa; 20]);
        let addr_b = Address::new([0xbb; 20]);
        let addr_c = Address::new([0xcc; 20]);
        assert!(selected.contains(&addr_a));
        assert!(selected.contains(&addr_b));
        assert!(selected.contains(&addr_c));
    }

    #[test]
    fn test_proposer_weighted() {
        let v1 = make_validator(1, 0xaa, 300);
        let v2 = make_validator(2, 0xbb, 100);
        let mut vs = make_validator_set(vec![v1, v2]);

        let mut count_a = 0u64;
        let mut count_b = 0u64;
        let rounds = 400;

        for _ in 0..rounds {
            let proposer = select_proposer(&mut vs);
            if proposer == Address::new([0xaa; 20]) {
                count_a += 1;
            } else {
                count_b += 1;
            }
        }

        // With 3:1 power ratio, validator A should be selected ~3x as often
        assert_eq!(count_a, 300);
        assert_eq!(count_b, 100);
    }

    #[test]
    fn test_proposer_deterministic() {
        let make_set = || {
            let v1 = make_validator(1, 0xaa, 200);
            let v2 = make_validator(2, 0xbb, 100);
            make_validator_set(vec![v1, v2])
        };

        let mut vs1 = make_set();
        let mut vs2 = make_set();

        for _ in 0..20 {
            let p1 = select_proposer(&mut vs1);
            let p2 = select_proposer(&mut vs2);
            assert_eq!(p1, p2, "proposer selection must be deterministic");
        }
    }
}
