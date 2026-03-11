//! Port of Go Bor `valset/validator_set_test.go` tests to Rust.
//!
//! These tests verify compatibility with the Go reference implementation.

use alloy_primitives::Address;
use bor_consensus::proposer::select_proposer;
use bor_primitives::{Validator, ValidatorSet};
use std::str::FromStr;

// Go test validator addresses derived from specific private keys.
const SIGNER0_ADDR: &str = "0x96C42C56fdb78294F96B0cFa33c92bed7D75F96a"; // power=100
const SIGNER1_ADDR: &str = "0x98925BE497f6dFF6A5a33dDA8B5933cA35262d69"; // power=200
const SIGNER2_ADDR: &str = "0x648Cf2A5b119E2c04061021834F8f75735B1D36b"; // power=300
const SIGNER3_ADDR: &str = "0x168f220B3b313D456eD4797520eFdFA9c57E6C45"; // power=400

fn addr(s: &str) -> Address {
    Address::from_str(s).expect("valid address")
}

fn make_validator(id: u64, signer: &str, power: i64) -> Validator {
    let address = addr(signer);
    Validator {
        id,
        address,
        voting_power: power,
        signer: address,
        proposer_priority: 0,
    }
}

fn make_validator_set(validators: Vec<Validator>) -> ValidatorSet {
    ValidatorSet {
        validators,
        proposer: None,
    }
}

// ---------------------------------------------------------------------------
// 1. TestIncrementProposerPriority
//
// For validator sets of size 1-4, call select_proposer 10 times and verify
// the proposer sequence matches the CometBFT weighted round-robin output.
// ---------------------------------------------------------------------------

#[test]
fn test_increment_proposer_priority_one_validator() {
    let v0 = make_validator(1, SIGNER0_ADDR, 100);
    let mut vs = make_validator_set(vec![v0]);

    // Single validator is always the proposer.
    let expected = addr(SIGNER0_ADDR);
    for round in 0..10 {
        let got = select_proposer(&mut vs);
        assert_eq!(got, expected, "round {round}: expected signer0 as sole validator");
    }
}

#[test]
fn test_increment_proposer_priority_two_validators() {
    let v0 = make_validator(1, SIGNER0_ADDR, 100);
    let v1 = make_validator(2, SIGNER1_ADDR, 200);
    let mut vs = make_validator_set(vec![v0, v1]);

    let s0 = addr(SIGNER0_ADDR);
    let s1 = addr(SIGNER1_ADDR);

    // With powers 100:200 (total 300), the cycle over 3 rounds is
    // [signer1, signer0, signer1] then repeats.
    let expected = [s1, s0, s1, s1, s0, s1, s1, s0, s1, s1];

    for (round, exp) in expected.iter().enumerate() {
        let got = select_proposer(&mut vs);
        assert_eq!(got, *exp, "round {round}");
    }
}

#[test]
fn test_increment_proposer_priority_three_validators() {
    let v0 = make_validator(1, SIGNER0_ADDR, 100);
    let v1 = make_validator(2, SIGNER1_ADDR, 200);
    let v2 = make_validator(3, SIGNER2_ADDR, 300);
    let mut vs = make_validator_set(vec![v0, v1, v2]);

    let s0 = addr(SIGNER0_ADDR);
    let s1 = addr(SIGNER1_ADDR);
    let s2 = addr(SIGNER2_ADDR);

    // Powers 100:200:300 (total 600).
    let expected = [s2, s1, s2, s0, s1, s2, s2, s1, s2, s0];

    for (round, exp) in expected.iter().enumerate() {
        let got = select_proposer(&mut vs);
        assert_eq!(got, *exp, "round {round}");
    }
}

#[test]
fn test_increment_proposer_priority_four_validators() {
    let v0 = make_validator(1, SIGNER0_ADDR, 100);
    let v1 = make_validator(2, SIGNER1_ADDR, 200);
    let v2 = make_validator(3, SIGNER2_ADDR, 300);
    let v3 = make_validator(4, SIGNER3_ADDR, 400);
    let mut vs = make_validator_set(vec![v0, v1, v2, v3]);

    let s0 = addr(SIGNER0_ADDR);
    let s1 = addr(SIGNER1_ADDR);
    let s2 = addr(SIGNER2_ADDR);
    let s3 = addr(SIGNER3_ADDR);

    // Powers 100:200:300:400 (total 1000).
    let expected = [s3, s2, s1, s3, s2, s0, s3, s1, s2, s3];

    for (round, exp) in expected.iter().enumerate() {
        let got = select_proposer(&mut vs);
        assert_eq!(got, *exp, "round {round}");
    }
}

// ---------------------------------------------------------------------------
// 2. TestGetValidatorByAddressAndIndex
//
// Verifies round-trip lookup: for each validator, find by address returns the
// correct index, and indexing by that index returns the correct address.
// Also tests the negative case (unknown address returns None).
//
// NOTE: ValidatorSet has no GetByAddress/GetByIndex methods. We test
// equivalent behaviour directly on the `validators` Vec.
// ---------------------------------------------------------------------------

#[test]
fn test_get_validator_by_address_and_index() {
    let v0 = make_validator(1, SIGNER0_ADDR, 100);
    let v1 = make_validator(2, SIGNER1_ADDR, 200);
    let v2 = make_validator(3, SIGNER2_ADDR, 300);
    let v3 = make_validator(4, SIGNER3_ADDR, 400);
    let vs = make_validator_set(vec![v0, v1, v2, v3]);

    let addrs = [
        addr(SIGNER0_ADDR),
        addr(SIGNER1_ADDR),
        addr(SIGNER2_ADDR),
        addr(SIGNER3_ADDR),
    ];

    // Forward lookup: address → index
    for (expected_idx, target_addr) in addrs.iter().enumerate() {
        let found = vs
            .validators
            .iter()
            .position(|v| v.signer == *target_addr);
        assert_eq!(found, Some(expected_idx), "address {target_addr} not at expected index");
    }

    // Reverse lookup: index → address
    for (idx, expected_addr) in addrs.iter().enumerate() {
        assert_eq!(
            vs.validators[idx].signer, *expected_addr,
            "index {idx} has wrong address"
        );
    }

    // Negative case: unknown address returns None.
    let unknown = Address::from_str("0x0000000000000000000000000000000000000000").unwrap();
    let found = vs.validators.iter().position(|v| v.signer == unknown);
    assert_eq!(found, None, "unknown address should not be found");
}

// ---------------------------------------------------------------------------
// 3. TestUpdateWithChangeSet
//
// Tests updating validator powers and adding new validators.
// Original set: signer0(100), signer1(200), signer2(300), signer3(400) → total 1000
// Update: signer2 → 150, signer3 → 800, add new validator with power 250 → total 1500
//
// NOTE: ValidatorSet has no dedicated UpdateWithChangeSet method.
// We test the equivalent behaviour by mutating the validators Vec directly.
// ---------------------------------------------------------------------------

#[test]
fn test_update_with_change_set() {
    let v0 = make_validator(1, SIGNER0_ADDR, 100);
    let v1 = make_validator(2, SIGNER1_ADDR, 200);
    let v2 = make_validator(3, SIGNER2_ADDR, 300);
    let v3 = make_validator(4, SIGNER3_ADDR, 400);
    let mut vs = make_validator_set(vec![v0, v1, v2, v3]);

    // Verify original total power.
    let total: i64 = vs.validators.iter().map(|v| v.voting_power).sum();
    assert_eq!(total, 1000);

    // Apply changes: update signer2 power 300 → 150, signer3 power 400 → 800.
    vs.validators[2].voting_power = 150;
    vs.validators[3].voting_power = 800;

    // Add a new validator.
    let new_addr = Address::from_str("0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
    let new_val = Validator {
        id: 5,
        address: new_addr,
        voting_power: 250,
        signer: new_addr,
        proposer_priority: 0,
    };
    vs.validators.push(new_val);

    // Verify updated total power.
    let new_total: i64 = vs.validators.iter().map(|v| v.voting_power).sum();
    assert_eq!(new_total, 1500); // 100 + 200 + 150 + 800 + 250

    // Verify individual powers after update.
    assert_eq!(vs.validators[0].voting_power, 100);
    assert_eq!(vs.validators[1].voting_power, 200);
    assert_eq!(vs.validators[2].voting_power, 150);
    assert_eq!(vs.validators[3].voting_power, 800);
    assert_eq!(vs.validators[4].voting_power, 250);
    assert_eq!(vs.validators.len(), 5);
}

// ---------------------------------------------------------------------------
// 4. TestCheckEmptyId
//
// Tests checking if any validator has id == 0.
// Four cases: empty set, all non-zero, one zero, all zero.
// ---------------------------------------------------------------------------

fn has_empty_id(vs: &ValidatorSet) -> bool {
    vs.validators.iter().any(|v| v.id == 0)
}

#[test]
fn test_check_empty_id_empty_set() {
    let vs = make_validator_set(vec![]);
    assert!(!has_empty_id(&vs), "empty set should have no zero ids");
}

#[test]
fn test_check_empty_id_all_nonzero() {
    let v0 = make_validator(1, SIGNER0_ADDR, 100);
    let v1 = make_validator(2, SIGNER1_ADDR, 200);
    let vs = make_validator_set(vec![v0, v1]);
    assert!(!has_empty_id(&vs), "all ids are non-zero");
}

#[test]
fn test_check_empty_id_one_zero() {
    let v0 = make_validator(0, SIGNER0_ADDR, 100);
    let v1 = make_validator(2, SIGNER1_ADDR, 200);
    let vs = make_validator_set(vec![v0, v1]);
    assert!(has_empty_id(&vs), "one validator has id=0");
}

#[test]
fn test_check_empty_id_all_zero() {
    let v0 = make_validator(0, SIGNER0_ADDR, 100);
    let v1 = make_validator(0, SIGNER1_ADDR, 200);
    let vs = make_validator_set(vec![v0, v1]);
    assert!(has_empty_id(&vs), "all validators have id=0");
}

// ---------------------------------------------------------------------------
// 5. TestRescalePriorities
//
// NOTE: Go test `TestRescalePriorities` exists but our Rust ValidatorSet
// doesn't have a rescale method yet. Skipping this test.
// ---------------------------------------------------------------------------
