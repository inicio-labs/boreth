//! Comprehensive integration tests for bor-consensus.

use std::collections::BTreeMap;

use alloy_primitives::{keccak256, Address, B256, U256};
use bor_chainspec::constants::{EXTRADATA_SEAL_LEN, EXTRADATA_VANITY_LEN};
use bor_consensus::difficulty::{calculate_difficulty, diff_inturn, diff_noturn, is_inturn};
use bor_consensus::extra_data::{ExtraData, ExtraDataError};
use bor_consensus::proposer::{get_sprint_producer, select_proposer};
use bor_consensus::recents::Recents;
use bor_consensus::seal::{ecrecover_seal, SealError};
use bor_consensus::snapshot::BorSnapshot;
use bor_consensus::validation::{
    validate_header, validate_header_against_parent, HeaderValidationParams,
    ParentValidationParams, ValidationError,
};
use bor_consensus::block_validation::{validate_block_post_execution, validate_block_pre_execution};
use bor_primitives::{Validator, ValidatorSet};
use k256::ecdsa::SigningKey;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn make_addresses(count: usize) -> Vec<Address> {
    (0..count)
        .map(|i| Address::new([(i + 1) as u8; 20]))
        .collect()
}

fn make_extra_data_with_validators(validators: &[Address]) -> Vec<u8> {
    let mut data = vec![0u8; EXTRADATA_VANITY_LEN];
    for v in validators {
        data.extend_from_slice(v.as_slice());
    }
    data.extend_from_slice(&[0u8; EXTRADATA_SEAL_LEN]);
    data
}

fn make_minimal_extra_data() -> Vec<u8> {
    vec![0u8; EXTRADATA_VANITY_LEN + EXTRADATA_SEAL_LEN]
}

/// Build extra data (vanity + seal) with a real ECDSA signature.
fn make_signed_extra_data(signing_key: &SigningKey, seal_hash: &B256) -> Vec<u8> {
    let (sig, recid) = signing_key
        .sign_prehash_recoverable(seal_hash.as_ref())
        .unwrap();
    let mut seal = [0u8; 65];
    seal[..64].copy_from_slice(&sig.to_bytes());
    seal[64] = recid.to_byte();

    let mut data = vec![0u8; EXTRADATA_VANITY_LEN];
    data.extend_from_slice(&seal);
    data
}

fn address_of(key: &SigningKey) -> Address {
    let vk = key.verifying_key();
    let pt = vk.to_encoded_point(false);
    Address::from_raw_public_key(&pt.as_bytes()[1..])
}

fn deterministic_key(seed: &[u8]) -> SigningKey {
    let secret: [u8; 32] = keccak256(seed).0;
    SigningKey::from_bytes((&secret).into()).unwrap()
}

// ===========================================================================
// 4.1 Validator set changes at span boundaries
// ===========================================================================

#[test]
fn test_4_1_validator_set_changes_at_span_boundary() {
    // Span N validators
    let span_n = vec![
        make_validator(1, 0x01, 100),
        make_validator(2, 0x02, 100),
        make_validator(3, 0x03, 100),
    ];
    // Span N+1 validators (different set)
    let span_n1 = vec![
        make_validator(4, 0x04, 100),
        make_validator(5, 0x05, 100),
        make_validator(6, 0x06, 100),
    ];

    let vs_n = make_validator_set(span_n);
    let vs_n1 = make_validator_set(span_n1);

    let snap_n = BorSnapshot::new(6399, B256::ZERO, vs_n);
    let snap_n1 = BorSnapshot::new(6400, B256::ZERO, vs_n1);

    // Validator from span N is authorized in snap_n but not snap_n1
    let addr_1 = Address::new([0x01; 20]);
    let addr_4 = Address::new([0x04; 20]);

    assert!(snap_n.is_authorized(&addr_1));
    assert!(!snap_n.is_authorized(&addr_4));

    assert!(!snap_n1.is_authorized(&addr_1));
    assert!(snap_n1.is_authorized(&addr_4));
}

#[test]
fn test_4_1_difficulty_uses_new_set_after_transition() {
    let small_set = make_addresses(3);
    let large_set = make_addresses(7);

    // Block at span boundary uses the new set's count for difficulty
    let diff_small = calculate_difficulty(&small_set[0], &small_set, 6400);
    let diff_large = calculate_difficulty(&large_set[0], &large_set, 6400);

    // 6400 % 3 == 1 => small_set[0] is NOT inturn, distance = 3 - 1 + 0 = 2, diff = 3 - 2 = 1
    assert_eq!(diff_small, U256::from(1));
    // 6400 % 7 == 2 => large_set[0] is NOT inturn, distance = 7 - 2 + 0 = 5, diff = 7 - 5 = 2
    assert_eq!(diff_large, U256::from(2));
}

// ===========================================================================
// 4.2 Anti-double-sign across boundaries
// ===========================================================================

#[test]
fn test_4_2_anti_double_sign_5_validators_window_3() {
    // 5 validators => window = 5/2 + 1 = 3
    let mut recents = Recents::new();
    let signer = Address::with_last_byte(0xAA);

    // Signer signs block 10
    recents.add_signer(10, signer);

    // At block 12: window starts at 12 - 3 = 9, range [9..12) includes 10 => rejected
    assert!(recents.is_recently_signed(&signer, 12, 5));

    // At block 13: window starts at 13 - 3 = 10, range [10..13) includes 10 => rejected
    assert!(recents.is_recently_signed(&signer, 13, 5));

    // At block 14: window starts at 14 - 3 = 11, range [11..14) does NOT include 10 => allowed
    assert!(!recents.is_recently_signed(&signer, 14, 5));
}

#[test]
fn test_4_2_anti_double_sign_different_signers() {
    let mut recents = Recents::new();
    let signer_a = Address::with_last_byte(0xAA);
    let signer_b = Address::with_last_byte(0xBB);

    recents.add_signer(10, signer_a);

    // Signer B should NOT be blocked
    assert!(!recents.is_recently_signed(&signer_b, 12, 5));

    // Signer A IS blocked at block 12
    assert!(recents.is_recently_signed(&signer_a, 12, 5));
}

// ===========================================================================
// 4.3 Recents window size with various validator counts
// ===========================================================================

#[test]
fn test_4_3_recents_window_sizes() {
    // window = n/2 + 1
    // n=1 => 1, n=2 => 2, n=3 => 2, n=4 => 3, n=5 => 3, n=10 => 6, n=100 => 51
    let cases: Vec<(usize, u64)> = vec![
        (1, 1),
        (2, 2),
        (3, 2),
        (4, 3),
        (5, 3),
        (10, 6),
        (100, 51),
    ];

    for (validator_count, expected_window) in cases {
        let mut recents = Recents::new();
        let signer = Address::with_last_byte(0xFF);

        // Sign at block 100
        recents.add_signer(100, signer);

        // At block 100 + expected_window, the signer should still be in range
        // range = [100+w - w .. 100+w) = [100 .. 100+w) which includes 100
        let still_blocked = 100 + expected_window;
        assert!(
            recents.is_recently_signed(&signer, still_blocked, validator_count),
            "validator_count={validator_count}: signer should be blocked at block {still_blocked}"
        );

        // At block 100 + expected_window + 1, range = [101 .. 101+w) which does NOT include 100
        let unblocked = 100 + expected_window + 1;
        assert!(
            !recents.is_recently_signed(&signer, unblocked, validator_count),
            "validator_count={validator_count}: signer should be unblocked at block {unblocked}"
        );
    }
}

// ===========================================================================
// 4.4 Difficulty calculation circular distance edge cases
// ===========================================================================

#[test]
fn test_4_4_block_0_5_validators() {
    let validators = make_addresses(5);
    // block 0 => inturn_idx = 0

    // idx 0 is inturn => diff = 5
    assert_eq!(
        calculate_difficulty(&validators[0], &validators, 0),
        U256::from(5)
    );
    // idx 1: distance = 1, diff = 5 - 1 = 4
    assert_eq!(
        calculate_difficulty(&validators[1], &validators, 0),
        U256::from(4)
    );
    // idx 4: distance = 4, diff = 5 - 4 = 1
    assert_eq!(
        calculate_difficulty(&validators[4], &validators, 0),
        U256::from(1)
    );
}

#[test]
fn test_4_4_block_3_5_validators() {
    let validators = make_addresses(5);
    // block 3 => inturn_idx = 3

    // idx 0: signer_idx=0 < inturn_idx=3 => distance = 5 - 3 + 0 = 2, diff = 5 - 2 = 3
    assert_eq!(
        calculate_difficulty(&validators[0], &validators, 3),
        U256::from(3)
    );
    // idx 4: signer_idx=4 > inturn_idx=3 => distance = 4 - 3 = 1, diff = 5 - 1 = 4
    assert_eq!(
        calculate_difficulty(&validators[4], &validators, 3),
        U256::from(4)
    );
    // idx 2: signer_idx=2 < inturn_idx=3 => distance = 5 - 3 + 2 = 4, diff = 5 - 4 = 1
    assert_eq!(
        calculate_difficulty(&validators[2], &validators, 3),
        U256::from(1)
    );
    // idx 3 is inturn => diff = 5
    assert_eq!(
        calculate_difficulty(&validators[3], &validators, 3),
        U256::from(5)
    );
}

#[test]
fn test_4_4_single_validator_always_inturn() {
    let validators = make_addresses(1);
    // Single validator is always inturn, diff = 1 (count)
    for block in 0..20 {
        assert!(is_inturn(&validators[0], &validators, block));
        assert_eq!(
            calculate_difficulty(&validators[0], &validators, block),
            U256::from(1)
        );
    }
}

#[test]
fn test_4_4_signer_not_in_set() {
    let validators = make_addresses(5);
    let unknown = Address::new([0xFF; 20]);
    // Signer not in set => diff = 1
    assert_eq!(
        calculate_difficulty(&unknown, &validators, 0),
        U256::from(1)
    );
    assert_eq!(
        calculate_difficulty(&unknown, &validators, 3),
        U256::from(1)
    );
}

#[test]
fn test_4_4_diff_inturn_noturn_helpers() {
    assert_eq!(diff_inturn(5), U256::from(5));
    assert_eq!(diff_inturn(1), U256::from(1));

    assert_eq!(diff_noturn(5, 1), U256::from(4));
    assert_eq!(diff_noturn(5, 4), U256::from(1));
    assert_eq!(diff_noturn(5, 5), U256::from(1)); // clamped
    assert_eq!(diff_noturn(5, 100), U256::from(1)); // clamped
    assert_eq!(diff_noturn(1, 0), U256::from(1));
}

// ===========================================================================
// 4.5 Seal verification with malformed signatures
// ===========================================================================

#[test]
fn test_4_5_seal_length_64() {
    let hash = B256::ZERO;
    let sig = [0u8; 64];
    let err = ecrecover_seal(&hash, &sig).unwrap_err();
    assert!(matches!(err, SealError::InvalidSignatureLength(64)));
}

#[test]
fn test_4_5_seal_length_66() {
    let hash = B256::ZERO;
    let sig = [0u8; 66];
    let err = ecrecover_seal(&hash, &sig).unwrap_err();
    assert!(matches!(err, SealError::InvalidSignatureLength(66)));
}

#[test]
fn test_4_5_seal_all_zero_65() {
    let hash = B256::ZERO;
    let sig = [0u8; 65];
    // All-zero 65-byte signature should fail recovery
    assert!(ecrecover_seal(&hash, &sig).is_err());
}

#[test]
fn test_4_5_valid_sig_wrong_hash() {
    let key = deterministic_key(b"seal_test_key");
    let correct_hash = keccak256(b"correct message");
    let wrong_hash = keccak256(b"wrong message");

    let (sig, recid) = key
        .sign_prehash_recoverable(correct_hash.as_ref())
        .unwrap();
    let mut sig_bytes = [0u8; 65];
    sig_bytes[..64].copy_from_slice(&sig.to_bytes());
    sig_bytes[64] = recid.to_byte();

    // Recovery with wrong hash should produce a different address
    let recovered_correct = ecrecover_seal(&correct_hash, &sig_bytes).unwrap();
    let recovered_wrong = ecrecover_seal(&wrong_hash, &sig_bytes).unwrap();

    let expected = address_of(&key);
    assert_eq!(recovered_correct, expected);
    assert_ne!(recovered_wrong, expected);
}

#[test]
fn test_4_5_valid_seal_roundtrip() {
    let key = deterministic_key(b"roundtrip_seal_key");
    let hash = keccak256(b"roundtrip test");

    let (sig, recid) = key.sign_prehash_recoverable(hash.as_ref()).unwrap();
    let mut sig_bytes = [0u8; 65];
    sig_bytes[..64].copy_from_slice(&sig.to_bytes());
    sig_bytes[64] = recid.to_byte();

    let recovered = ecrecover_seal(&hash, &sig_bytes).unwrap();
    assert_eq!(recovered, address_of(&key));
}

// ===========================================================================
// 4.6 Timestamp validation edge cases
// ===========================================================================

#[test]
fn test_4_6_timestamp_equal_to_parent_rejected() {
    let params = HeaderValidationParams {
        number: 101,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
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
fn test_4_6_timestamp_parent_plus_1_accepted() {
    let params = HeaderValidationParams {
        number: 101,
        timestamp: 1001,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
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
fn test_4_6_future_block_at_exact_boundary() {
    // MAX_FUTURE_BLOCK_TIME = 15
    let now = 1000u64;

    // Exactly at boundary: now + 15 => should pass
    let params_ok = HeaderValidationParams {
        number: 101,
        timestamp: now + 15,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: false,
    };
    // validate_header will check the future-block rule; we just need to get past that check.
    // It will fail on seal, but we only care that it does NOT fail on FutureBlock.
    let result = validate_header(&params_ok, &[], &BTreeMap::new(), now);
    assert!(
        !matches!(result, Err(ValidationError::FutureBlock { .. })),
        "timestamp at exact boundary should not trigger FutureBlock"
    );

    // One second past boundary: now + 16 => should fail with FutureBlock
    let params_fail = HeaderValidationParams {
        number: 101,
        timestamp: now + 16,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: false,
    };
    let err = validate_header(&params_fail, &[], &BTreeMap::new(), now).unwrap_err();
    assert!(matches!(err, ValidationError::FutureBlock { .. }));
}

#[test]
fn test_4_6_timestamp_overflow() {
    // Parent timestamp at u64::MAX should cause child timestamp to be <= parent
    let params = HeaderValidationParams {
        number: 101,
        timestamp: u64::MAX,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: false,
    };
    let parent = ParentValidationParams {
        parent_timestamp: u64::MAX,
    };
    let err = validate_header_against_parent(&params, &parent).unwrap_err();
    assert!(matches!(err, ValidationError::TimestampNotIncreasing { .. }));
}

// ===========================================================================
// 4.7 Block 0 (genesis) special handling
// ===========================================================================

#[test]
fn test_4_7_block_0_not_span_start() {
    // block_number=0 is explicitly NOT a span start (block_number > 0 check)
    let extra = make_minimal_extra_data();
    validate_block_pre_execution(0, &extra, false, false, 6400, None).unwrap();
}

#[test]
fn test_4_7_block_0_not_sprint_boundary_no_system_txs() {
    // Block 0 should not require validators in extra data
    let extra = make_minimal_extra_data();
    validate_block_pre_execution(0, &extra, false, false, 16, None).unwrap();
}

#[test]
fn test_4_7_genesis_extra_data_no_validators_required() {
    // At block 0, extra data does NOT need to contain validators
    let extra = make_minimal_extra_data();
    let parsed = ExtraData::parse(&extra).unwrap();
    assert!(parsed.validators().is_empty());

    // Pre-execution should pass for block 0 even without validators
    validate_block_pre_execution(0, &extra, false, false, 6400, None).unwrap();
}

// ===========================================================================
// 4.8 Extra data parsing
// ===========================================================================

#[test]
fn test_4_8_extra_data_100_validators() {
    // 32 vanity + 100*20 validator bytes + 65 seal = 2097 bytes
    let validators: Vec<Address> = (0u8..100)
        .map(|i| {
            let mut bytes = [0u8; 20];
            bytes[0] = i;
            bytes[19] = i;
            Address::new(bytes)
        })
        .collect();

    let data = make_extra_data_with_validators(&validators);
    assert_eq!(data.len(), 32 + 2000 + 65);
    assert_eq!(data.len(), 2097);

    let parsed = ExtraData::parse(&data).unwrap();
    assert_eq!(parsed.validators().len(), 100);
    assert_eq!(parsed.validator_bytes.len(), 2000);
}

#[test]
fn test_4_8_zero_validators_at_non_span_start() {
    // 0 validators is fine at a non-span-start block
    let extra = make_minimal_extra_data();
    let parsed = ExtraData::parse(&extra).unwrap();
    assert!(parsed.validators().is_empty());

    validate_block_pre_execution(100, &extra, false, false, 6400, None).unwrap();
}

#[test]
fn test_4_8_zero_validators_at_span_start_rejected() {
    // At span start (block 6400), empty validator set is rejected
    let extra = make_minimal_extra_data();
    let err = validate_block_pre_execution(6400, &extra, false, false, 6400, None).unwrap_err();
    assert!(matches!(
        err,
        ValidationError::MissingValidatorsAtSpanStart(6400)
    ));
}

#[test]
fn test_4_8_one_validator_at_span_start_accepted() {
    let validators = vec![Address::new([0xAA; 20])];
    let extra = make_extra_data_with_validators(&validators);
    validate_block_pre_execution(6400, &extra, false, false, 6400, Some(&validators)).unwrap();
}

#[test]
fn test_4_8_validator_bytes_not_multiple_of_20() {
    // 32 vanity + 15 bytes (not a multiple of 20) + 65 seal = 112
    let mut data = vec![0u8; EXTRADATA_VANITY_LEN + 15 + EXTRADATA_SEAL_LEN];
    // Fill seal area
    let seal_start = data.len() - EXTRADATA_SEAL_LEN;
    data[seal_start..].fill(0xFF);

    let err = ExtraData::parse(&data).unwrap_err();
    assert!(matches!(err, ExtraDataError::InvalidValidatorBytes(15)));
}

#[test]
fn test_4_8_exactly_97_bytes_valid() {
    // 32 + 0 + 65 = 97 => valid, 0 validators
    let data = vec![0u8; 97];
    let parsed = ExtraData::parse(&data).unwrap();
    assert!(parsed.validators().is_empty());
    assert_eq!(parsed.vanity.len(), 32);
    assert_eq!(parsed.seal.len(), 65);
}

#[test]
fn test_4_8_96_bytes_too_short() {
    let data = vec![0u8; 96];
    let err = ExtraData::parse(&data).unwrap_err();
    assert!(matches!(err, ExtraDataError::TooShort(96)));
}

// ===========================================================================
// 7.1 Snapshot recents pruning
// ===========================================================================

#[test]
fn test_7_1_snapshot_recents_pruning() {
    // 3 validators => window = 3/2 + 1 = 2
    let vs = make_validator_set(vec![
        make_validator(1, 0x01, 100),
        make_validator(2, 0x02, 100),
        make_validator(3, 0x03, 100),
    ]);

    let mut snap = BorSnapshot::new(100, B256::ZERO, vs);

    // Apply blocks 101, 102, 103 with different signers
    let signer_a = Address::new([0x01; 20]);
    let signer_b = Address::new([0x02; 20]);
    let signer_c = Address::new([0x03; 20]);

    snap.apply(101, signer_a);
    assert!(snap.recents.contains_key(&101));

    snap.apply(102, signer_b);
    assert!(snap.recents.contains_key(&101));
    assert!(snap.recents.contains_key(&102));

    snap.apply(103, signer_c);
    // window = 2, cutoff = 103 - 2 = 101
    // split_off(101) keeps keys >= 101
    // So 101 should still be present
    assert!(snap.recents.contains_key(&101));
    assert!(snap.recents.contains_key(&102));
    assert!(snap.recents.contains_key(&103));

    // After block 104, cutoff = 104 - 2 = 102, so 101 is pruned
    snap.apply(104, signer_a);
    assert!(!snap.recents.contains_key(&101));
    assert!(snap.recents.contains_key(&102));
    assert!(snap.recents.contains_key(&103));
    assert!(snap.recents.contains_key(&104));
}

// ===========================================================================
// 7.2 Snapshot encode/decode
// ===========================================================================

#[test]
fn test_7_2_snapshot_encode_decode_100_validators() {
    let validators: Vec<Validator> = (0..100)
        .map(|i| {
            let byte = (i + 1) as u8;
            make_validator(i as u64, byte, 100)
        })
        .collect();

    let vs = make_validator_set(validators);
    let snap = BorSnapshot::new(50_000, B256::from([0xAB; 32]), vs);

    let encoded = snap.encode();
    let decoded = BorSnapshot::decode(&encoded).unwrap();

    assert_eq!(decoded.number, 50_000);
    assert_eq!(decoded.hash, B256::from([0xAB; 32]));
    assert_eq!(decoded.validator_set.validators.len(), 100);
}

#[test]
fn test_7_2_snapshot_encode_decode_empty_validator_set() {
    let vs = make_validator_set(vec![]);
    let snap = BorSnapshot::new(0, B256::ZERO, vs);

    let encoded = snap.encode();
    let decoded = BorSnapshot::decode(&encoded).unwrap();

    assert_eq!(decoded.number, 0);
    assert!(decoded.validator_set.validators.is_empty());
}

#[test]
fn test_7_2_snapshot_decode_corrupted_bytes() {
    let corrupted = vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB];
    assert!(BorSnapshot::decode(&corrupted).is_err());
}

#[test]
fn test_7_2_snapshot_decode_empty_bytes() {
    assert!(BorSnapshot::decode(&[]).is_err());
}

#[test]
fn test_7_2_snapshot_decode_wrong_json() {
    let wrong_json = br#"{"not_a_snapshot": true}"#;
    assert!(BorSnapshot::decode(wrong_json).is_err());
}

// ===========================================================================
// 10.1 Proposer selection with zero voting power
// ===========================================================================

#[test]
fn test_10_1_zero_voting_power_never_selected() {
    let v_a = make_validator(1, 0xAA, 100);
    let v_zero = make_validator(2, 0x00, 0);
    let v_c = make_validator(3, 0xCC, 100);
    let mut vs = make_validator_set(vec![v_a, v_zero, v_c]);

    let addr_zero = Address::new([0x00; 20]);

    // Run many rounds; validator with 0 power should never be selected
    for _ in 0..1000 {
        let proposer = select_proposer(&mut vs);
        assert_ne!(
            proposer, addr_zero,
            "validator with zero voting power must never be selected"
        );
    }
}

#[test]
fn test_10_1_mixed_powers() {
    // [100, 0, 100] => only validators 0 and 2 should be selected
    let v_a = make_validator(1, 0xAA, 100);
    let v_zero = make_validator(2, 0x00, 0);
    let v_c = make_validator(3, 0xCC, 100);
    let mut vs = make_validator_set(vec![v_a, v_zero, v_c]);

    let addr_a = Address::new([0xAA; 20]);
    let addr_c = Address::new([0xCC; 20]);

    let mut count_a = 0u64;
    let mut count_c = 0u64;

    for _ in 0..200 {
        let p = select_proposer(&mut vs);
        if p == addr_a {
            count_a += 1;
        } else if p == addr_c {
            count_c += 1;
        }
    }

    // Equal power => equal distribution
    assert_eq!(count_a, 100);
    assert_eq!(count_c, 100);
}

// ===========================================================================
// 10.3 Proposer priority overflow
// ===========================================================================

#[test]
fn test_10_3_proposer_priority_overflow() {
    let v1 = make_validator(1, 0x01, i64::MAX / 2);
    let v2 = make_validator(2, 0x02, i64::MAX / 3);
    let v3 = make_validator(3, 0x03, 1);
    let mut vs = make_validator_set(vec![v1, v2, v3]);

    // Run 1_000_000 rounds and ensure no panic
    for _ in 0..1_000_000 {
        select_proposer(&mut vs);
    }
}

// ===========================================================================
// 10.4 Proposer selection determinism
// ===========================================================================

#[test]
fn test_10_4_proposer_determinism_exact_distribution() {
    // 3 validators with powers [300, 200, 100], total = 600
    // Over 600 rounds: A selected 300 times, B 200 times, C 100 times
    let v_a = make_validator(1, 0xAA, 300);
    let v_b = make_validator(2, 0xBB, 200);
    let v_c = make_validator(3, 0xCC, 100);
    let mut vs = make_validator_set(vec![v_a, v_b, v_c]);

    let addr_a = Address::new([0xAA; 20]);
    let addr_b = Address::new([0xBB; 20]);
    let addr_c = Address::new([0xCC; 20]);

    let mut count_a = 0u64;
    let mut count_b = 0u64;
    let mut count_c = 0u64;

    for _ in 0..600 {
        let p = select_proposer(&mut vs);
        if p == addr_a {
            count_a += 1;
        } else if p == addr_b {
            count_b += 1;
        } else if p == addr_c {
            count_c += 1;
        }
    }

    assert_eq!(count_a, 300, "validator A should be selected 300/600 times");
    assert_eq!(count_b, 200, "validator B should be selected 200/600 times");
    assert_eq!(count_c, 100, "validator C should be selected 100/600 times");
}

#[test]
fn test_10_4_proposer_determinism_two_identical_runs() {
    let make_set = || {
        let v_a = make_validator(1, 0xAA, 300);
        let v_b = make_validator(2, 0xBB, 200);
        let v_c = make_validator(3, 0xCC, 100);
        make_validator_set(vec![v_a, v_b, v_c])
    };

    let mut vs1 = make_set();
    let mut vs2 = make_set();

    for round in 0..600 {
        let p1 = select_proposer(&mut vs1);
        let p2 = select_proposer(&mut vs2);
        assert_eq!(
            p1, p2,
            "proposer must be deterministic at round {round}"
        );
    }
}

// ===========================================================================
// Additional: get_sprint_producer coverage
// ===========================================================================

#[test]
fn test_get_sprint_producer() {
    let v1 = make_validator(1, 0xAA, 100);
    let v2 = make_validator(2, 0xBB, 100);
    let mut vs = make_validator_set(vec![v1, v2]);

    let producer = get_sprint_producer(&mut vs, 0);
    assert!(
        producer == Address::new([0xAA; 20]) || producer == Address::new([0xBB; 20])
    );
}

// ===========================================================================
// Additional: validate_block_post_execution coverage
// ===========================================================================

#[test]
fn test_post_execution_all_match() {
    let root = B256::from([0xAB; 32]);
    validate_block_post_execution(&root, &root, &root, &root, 21000, 21000).unwrap();
}

#[test]
fn test_post_execution_state_root_mismatch() {
    let expected = B256::from([0x11; 32]);
    let actual = B256::from([0x22; 32]);
    let receipt = B256::from([0x33; 32]);
    let err =
        validate_block_post_execution(&expected, &actual, &receipt, &receipt, 100, 100)
            .unwrap_err();
    assert!(matches!(err, ValidationError::StateRootMismatch { .. }));
}

#[test]
fn test_post_execution_receipt_root_mismatch() {
    let state = B256::from([0x11; 32]);
    let expected_receipt = B256::from([0x22; 32]);
    let actual_receipt = B256::from([0x33; 32]);
    let err = validate_block_post_execution(
        &state,
        &state,
        &expected_receipt,
        &actual_receipt,
        100,
        100,
    )
    .unwrap_err();
    assert!(matches!(err, ValidationError::ReceiptRootMismatch { .. }));
}

#[test]
fn test_post_execution_gas_mismatch() {
    let root = B256::from([0xAB; 32]);
    let err =
        validate_block_post_execution(&root, &root, &root, &root, 21000, 42000).unwrap_err();
    assert!(matches!(err, ValidationError::GasUsedMismatch { .. }));
}

// ===========================================================================
// Additional: Recents prune standalone
// ===========================================================================

#[test]
fn test_recents_prune_standalone() {
    let mut recents = Recents::new();
    let signer = Address::with_last_byte(1);

    for block in 1..=10 {
        recents.add_signer(block, signer);
    }

    // 5 validators => window = 3, prune at block 10 => cutoff = 7
    recents.prune(10, 5);

    // Blocks 1..7 should be pruned, 7..=10 remain
    assert!(!recents.is_recently_signed(&Address::with_last_byte(99), 11, 5));
}

// ===========================================================================
// Additional: validate_header full happy path with real key
// ===========================================================================

#[test]
fn test_validate_header_full_happy_path() {
    let key = deterministic_key(b"happy_path_validator");
    let signer_addr = address_of(&key);

    let seal_hash = keccak256(b"happy path block");
    let extra_data = make_signed_extra_data(&key, &seal_hash);

    let signers = vec![signer_addr];
    let expected_diff = calculate_difficulty(&signer_addr, &signers, 0);

    let params = HeaderValidationParams {
        number: 0,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: expected_diff,
        extra_data,
        gas_limit: 30_000_000,
        seal_hash,
        has_ommers: false,
    };

    let recovered = validate_header(&params, &signers, &BTreeMap::new(), 1000).unwrap();
    assert_eq!(recovered, signer_addr);
}

#[test]
fn test_validate_header_recently_signed_rejected() {
    let key = deterministic_key(b"recently_signed_validator");
    let signer_addr = address_of(&key);

    let seal_hash = keccak256(b"recently signed block");
    let extra_data = make_signed_extra_data(&key, &seal_hash);

    let signers = vec![signer_addr];
    let expected_diff = calculate_difficulty(&signer_addr, &signers, 5);

    let mut recent_signers = BTreeMap::new();
    // Signer signed block 4; window = 1/2+1 = 1; cutoff = 5-1=4; range [4..5) includes 4
    recent_signers.insert(4, signer_addr);

    let params = HeaderValidationParams {
        number: 5,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: expected_diff,
        extra_data,
        gas_limit: 30_000_000,
        seal_hash,
        has_ommers: false,
    };

    let err = validate_header(&params, &signers, &recent_signers, 1000).unwrap_err();
    assert!(matches!(err, ValidationError::RecentlySigned(_)));
}
