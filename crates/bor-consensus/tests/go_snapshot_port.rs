//! Port of Go Bor `snapshot_test.go` tests to Rust.
//!
//! These tests cover the BorSnapshot, Recents, and difficulty modules,
//! mapping Go test cases to our Rust API where possible.
//!
//! Go tests that cannot be ported:
//! - `isAllowedByValidatorSetOverride`: our code does not implement validator
//!   set override ranges, so there is no equivalent to test.

use alloy_primitives::{Address, B256, U256};
use bor_consensus::difficulty::{calculate_difficulty, diff_inturn, diff_noturn, is_inturn};
use bor_consensus::{BorSnapshot, Recents};
use bor_primitives::{Validator, ValidatorSet};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_validator(id: u64, addr: Address) -> Validator {
    Validator {
        id,
        address: addr,
        voting_power: 100,
        signer: addr,
        proposer_priority: 0,
    }
}

fn make_validator_set(addrs: &[Address]) -> ValidatorSet {
    ValidatorSet {
        validators: addrs
            .iter()
            .enumerate()
            .map(|(i, &a)| make_validator(i as u64 + 1, a))
            .collect(),
        proposer: None,
    }
}

fn addr(byte: u8) -> Address {
    Address::with_last_byte(byte)
}

// ---------------------------------------------------------------------------
// Snapshot tests (ports of Go snapshot_test.go)
// ---------------------------------------------------------------------------

/// 1. Create snapshot with validators, verify is_authorized returns true for
///    known validators and false for an unknown address.
#[test]
fn snapshot_is_authorized() {
    let validators = [addr(1), addr(2), addr(3)];
    let vs = make_validator_set(&validators);
    let snap = BorSnapshot::new(100, B256::ZERO, vs);

    for v in &validators {
        assert!(snap.is_authorized(v), "validator {v} should be authorized");
    }
    let unknown = addr(0xFF);
    assert!(
        !snap.is_authorized(&unknown),
        "unknown address should not be authorized"
    );
}

/// 2. Apply blocks, verify recents are tracked.
#[test]
fn snapshot_apply_tracks_recents() {
    let validators = [addr(1), addr(2), addr(3)];
    let vs = make_validator_set(&validators);
    let mut snap = BorSnapshot::new(100, B256::ZERO, vs);

    snap.apply(101, addr(1));
    snap.apply(102, addr(2));
    snap.apply(103, addr(3));

    assert_eq!(snap.number, 103);
    assert_eq!(snap.recents.get(&101), Some(&addr(1)));
    assert_eq!(snap.recents.get(&102), Some(&addr(2)));
    assert_eq!(snap.recents.get(&103), Some(&addr(3)));
}

/// 3. Encode/decode roundtrip - snapshot survives JSON serialization.
#[test]
fn snapshot_encode_decode_roundtrip() {
    let validators = [addr(1), addr(2), addr(3)];
    let vs = make_validator_set(&validators);
    let hash = B256::with_last_byte(0xAB);
    let mut snap = BorSnapshot::new(42, hash, vs);
    snap.apply(43, addr(1));
    snap.apply(44, addr(2));

    let encoded = snap.encode();
    let decoded = BorSnapshot::decode(&encoded).expect("decode should succeed");

    assert_eq!(decoded.number, snap.number);
    assert_eq!(decoded.hash, snap.hash);
    assert_eq!(
        decoded.validator_set.validators.len(),
        snap.validator_set.validators.len()
    );
    assert_eq!(decoded.recents.len(), snap.recents.len());
    for (block, signer) in &snap.recents {
        assert_eq!(decoded.recents.get(block), Some(signer));
    }
}

/// 4. Decode garbage data returns error.
#[test]
fn snapshot_decode_garbage_returns_error() {
    let garbage = b"this is not valid json at all!!!";
    let result = BorSnapshot::decode(garbage);
    assert!(result.is_err(), "decoding garbage should return Err");
}

/// 5. Snapshot tracks number and hash correctly.
#[test]
fn snapshot_tracks_number_and_hash() {
    let hash = B256::with_last_byte(0x42);
    let vs = make_validator_set(&[addr(1)]);
    let snap = BorSnapshot::new(999, hash, vs);

    assert_eq!(snap.number, 999);
    assert_eq!(snap.hash, hash);
}

/// Verify that apply updates the snapshot number.
#[test]
fn snapshot_apply_updates_number() {
    let vs = make_validator_set(&[addr(1), addr(2)]);
    let mut snap = BorSnapshot::new(0, B256::ZERO, vs);

    snap.apply(1, addr(1));
    assert_eq!(snap.number, 1);

    snap.apply(5, addr(2));
    assert_eq!(snap.number, 5);
}

/// Verify that apply prunes recents outside the window.
/// With 3 validators: window = 3 / 2 + 1 = 2.
#[test]
fn snapshot_apply_prunes_old_recents() {
    let vs = make_validator_set(&[addr(1), addr(2), addr(3)]);
    let mut snap = BorSnapshot::new(0, B256::ZERO, vs);

    // Apply blocks 1..=5
    for i in 1u64..=5 {
        let signer = addr((((i - 1) % 3) + 1) as u8);
        snap.apply(i, signer);
    }

    // Window = 2, cutoff = 5 - 2 = 3, split_off keeps keys >= 3
    // So blocks 1 and 2 should be pruned; blocks 3, 4, 5 may remain.
    assert!(
        !snap.recents.contains_key(&1),
        "block 1 should be pruned from recents"
    );
    assert!(
        !snap.recents.contains_key(&2),
        "block 2 should be pruned from recents"
    );
}

// ---------------------------------------------------------------------------
// Recents tests (Go: anti-double-sign via recents window)
// ---------------------------------------------------------------------------

/// 6. Add 3 signers, check is_recently_signed within window.
///    With 6 validators: window = 6 / 2 + 1 = 4.
#[test]
fn recents_is_recently_signed_within_window() {
    let mut recents = Recents::new();
    let validator_count = 6;

    recents.add_signer(10, addr(1));
    recents.add_signer(11, addr(2));
    recents.add_signer(12, addr(3));

    // Current block = 13, window = 4, start = 9, range 9..13
    // All three signers (at blocks 10, 11, 12) should be recently signed.
    assert!(recents.is_recently_signed(&addr(1), 13, validator_count));
    assert!(recents.is_recently_signed(&addr(2), 13, validator_count));
    assert!(recents.is_recently_signed(&addr(3), 13, validator_count));

    // A signer that was never added should not be recently signed.
    assert!(!recents.is_recently_signed(&addr(99), 13, validator_count));
}

/// 7. Prune removes entries outside window.
#[test]
fn recents_prune_removes_old_entries() {
    let mut recents = Recents::new();
    let validator_count = 4; // window = 4 / 2 + 1 = 3

    recents.add_signer(1, addr(1));
    recents.add_signer(2, addr(2));
    recents.add_signer(5, addr(3));
    recents.add_signer(6, addr(1));

    // Prune at block 6: cutoff = 6 - 3 = 3, keeps keys >= 3
    recents.prune(6, validator_count);

    // Blocks 1 and 2 should be gone.
    assert!(!recents.is_recently_signed(&addr(2), 7, validator_count));
    // Block 5 and 6 should still be present.
    assert!(recents.is_recently_signed(&addr(3), 7, validator_count));
    assert!(recents.is_recently_signed(&addr(1), 7, validator_count));
}

/// 8. Same signer at different blocks.
#[test]
fn recents_same_signer_different_blocks() {
    let mut recents = Recents::new();
    let validator_count = 10; // window = 6

    recents.add_signer(10, addr(1));
    recents.add_signer(15, addr(1));

    // At block 16, window start = 10; both entries are in range 10..16.
    assert!(recents.is_recently_signed(&addr(1), 16, validator_count));

    // At block 17, window start = 11; block 10 is outside the window.
    // But block 15 is still inside (11..17 contains 15).
    assert!(recents.is_recently_signed(&addr(1), 17, validator_count));

    // At block 22, window start = 16; block 15 is outside (16..22).
    assert!(!recents.is_recently_signed(&addr(1), 22, validator_count));
}

/// 9. Window size = validator_count / 2 + 1.
///    Verify the window boundary precisely.
#[test]
fn recents_window_size_boundary() {
    let mut recents = Recents::new();

    // 10 validators => window = 10 / 2 + 1 = 6
    let vc = 10;
    recents.add_signer(5, addr(1));

    // current_block = 11: start = 11 - 6 = 5, range 5..11 includes 5.
    assert!(
        recents.is_recently_signed(&addr(1), 11, vc),
        "block 5 should be within window at block 11"
    );

    // current_block = 12: start = 12 - 6 = 6, range 6..12 excludes 5.
    assert!(
        !recents.is_recently_signed(&addr(1), 12, vc),
        "block 5 should be outside window at block 12"
    );
}

// ---------------------------------------------------------------------------
// Difficulty tests (maps to Go's GetSignerSuccessionNumber)
// ---------------------------------------------------------------------------

/// 10. Single validator is always inturn.
#[test]
fn difficulty_single_validator_always_inturn() {
    let validators = vec![addr(1)];

    for block in 0u64..10 {
        assert!(
            is_inturn(&addr(1), &validators, block),
            "single validator should be inturn at block {block}"
        );
        assert_eq!(
            calculate_difficulty(&addr(1), &validators, block),
            diff_inturn(1)
        );
    }
}

/// 11. 4 validators rotating - verify which is inturn at each block.
#[test]
fn difficulty_four_validators_rotation() {
    let validators = vec![addr(1), addr(2), addr(3), addr(4)];

    for block in 0u64..12 {
        let expected_idx = (block as usize) % 4;
        for (i, v) in validators.iter().enumerate() {
            if i == expected_idx {
                assert!(
                    is_inturn(v, &validators, block),
                    "validator {i} should be inturn at block {block}"
                );
                assert_eq!(
                    calculate_difficulty(v, &validators, block),
                    diff_inturn(4),
                    "inturn validator at block {block} should have diff_inturn"
                );
            } else {
                assert!(
                    !is_inturn(v, &validators, block),
                    "validator {i} should NOT be inturn at block {block}"
                );
            }
        }
    }
}

/// 12. diff_inturn > diff_noturn for any non-zero distance.
#[test]
fn difficulty_inturn_greater_than_noturn() {
    for count in 2usize..=20 {
        let inturn = diff_inturn(count);
        for distance in 1..count {
            let noturn = diff_noturn(count, distance);
            assert!(
                inturn > noturn,
                "diff_inturn({count}) should be > diff_noturn({count}, {distance})"
            );
        }
    }
}

/// 13. diff_noturn decreases with increasing distance.
#[test]
fn difficulty_noturn_decreases_with_distance() {
    for count in 3usize..=20 {
        let mut prev = diff_noturn(count, 1);
        for distance in 2..count {
            let current = diff_noturn(count, distance);
            assert!(
                current <= prev,
                "diff_noturn({count}, {distance}) should be <= diff_noturn({count}, {})",
                distance - 1
            );
            prev = current;
        }
    }
}

/// Verify calculate_difficulty for a signer not in the validator set returns 1.
#[test]
fn difficulty_unknown_signer() {
    let validators = vec![addr(1), addr(2), addr(3)];
    let unknown = addr(0xFF);
    assert_eq!(
        calculate_difficulty(&unknown, &validators, 0),
        U256::from(1)
    );
}

/// Verify calculate_difficulty with empty validators returns 1.
#[test]
fn difficulty_empty_validators() {
    assert_eq!(
        calculate_difficulty(&addr(1), &[], 0),
        U256::from(1)
    );
    assert!(!is_inturn(&addr(1), &[], 0));
}

// ---------------------------------------------------------------------------
// Go tests that cannot be ported
// ---------------------------------------------------------------------------

// NOTE: Go's `TestIsAllowedByValidatorSetOverride` tests validator override
// ranges (e.g., allowing specific validators for specific block ranges).
// Our Rust implementation does not have an equivalent
// `isAllowedByValidatorSetOverride` function, so these tests are omitted.
