//! Port of Go Bor `verify_header_test.go` and `errors_test.go` to Rust.
//!
//! This file maps Go's table-driven TestVerifyHeader cases onto our Rust
//! validation API (`validate_header`, `validate_header_against_parent`,
//! `validate_block_pre_execution`, `validate_block_post_execution`).
//!
//! ## Go tests that cannot be ported (no corresponding Rust API):
//! - "nil header number → errUnknownBlock": Our API takes `u64` block number,
//!   so nil is not representable.
//! - "Rio/Bhilai-specific future block modes": We have a single FutureBlock
//!   check with MAX_FUTURE_BLOCK_TIME=15s; no mode-based variants.
//! - "VerifyHeaders batch verification": Our API validates one header at a time.
//! - "Signer caching / LRU snapshot tests": Not part of our stateless validation.
//! - "Gas limit exceeds maximum": Our `validate_header` does not enforce gas
//!   limit bounds (that is handled at the EVM layer); InvalidGasLimit is only
//!   used in parent-based gas limit delta checks which are not yet wired.
//! - "Unexpected requests hash (ErrUnexpectedRequests)": No corresponding
//!   variant in our ValidationError; Bor does not use EIP-7685 requests.

use std::collections::BTreeMap;

use alloy_primitives::{keccak256, Address, B256, U256};
use bor_chainspec::constants::{EXTRADATA_SEAL_LEN, EXTRADATA_VANITY_LEN};
use bor_consensus::{
    validate_block_post_execution, validate_block_pre_execution, validate_header,
    validate_header_against_parent, HeaderValidationParams, ParentValidationParams,
    SealError, ValidationError,
};
use bor_consensus::extra_data::ExtraDataError;
use k256::ecdsa::SigningKey;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive (signing_key, address) from a deterministic seed.
fn make_signer(seed: &[u8]) -> (SigningKey, Address) {
    let secret: [u8; 32] = keccak256(seed).0;
    let sk = SigningKey::from_bytes((&secret).into()).unwrap();
    let vk = sk.verifying_key();
    let addr = Address::from_raw_public_key(&vk.to_encoded_point(false).as_bytes()[1..]);
    (sk, addr)
}

/// Build a 65-byte ECDSA seal over `seal_hash` using `sk`.
fn sign_seal(sk: &SigningKey, seal_hash: &B256) -> [u8; 65] {
    let (sig, recid) = sk.sign_prehash_recoverable(seal_hash.as_ref()).unwrap();
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(&sig.to_bytes());
    out[64] = recid.to_byte();
    out
}

/// Build extra data: 32-byte vanity + seal.
fn make_extra_data_with_seal(seal: &[u8; 65]) -> Vec<u8> {
    let mut data = vec![0u8; EXTRADATA_VANITY_LEN];
    data.extend_from_slice(seal);
    data
}

/// Build minimal extra data (vanity + zero seal) — 97 bytes.
fn make_minimal_extra_data() -> Vec<u8> {
    vec![0u8; EXTRADATA_VANITY_LEN + EXTRADATA_SEAL_LEN]
}

/// Create a valid `HeaderValidationParams` that will pass all standalone
/// checks when paired with the returned signer address and authorized set.
///
/// Returns `(params, signer_address, authorized_signers)`.
fn make_valid_header() -> (HeaderValidationParams, Address, Vec<Address>) {
    let (sk, addr) = make_signer(b"valid_header_signer");
    let seal_hash = keccak256(b"valid header seal hash");
    let seal = sign_seal(&sk, &seal_hash);
    let extra_data = make_extra_data_with_seal(&seal);
    let signers = vec![addr];

    // With 1 validator, block 0 is inturn → difficulty = 1
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

    (params, addr, signers)
}

// ===========================================================================
// Go TestVerifyHeader case 1: nil header number → errUnknownBlock
// ===========================================================================
// SKIPPED — our API accepts `u64`, nil is not representable in Rust.

// ===========================================================================
// Go TestVerifyHeader case 2: future block (various modes) → ErrFutureBlock
// ===========================================================================

#[test]
fn test_future_block() {
    // Go test: header.Time = time.Now().Add(1 * time.Hour)
    // Our API: timestamp > current_time + 15 → FutureBlock
    let (sk, addr) = make_signer(b"future_block_signer");
    let seal_hash = keccak256(b"future block seal hash");
    let seal = sign_seal(&sk, &seal_hash);

    let params = HeaderValidationParams {
        number: 0,
        timestamp: 5000, // far in the future relative to current_time=1000
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_extra_data_with_seal(&seal),
        gas_limit: 30_000_000,
        seal_hash,
        has_ommers: false,
    };

    let err = validate_header(&params, &[addr], &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::FutureBlock { block_time: 5000, now: 1000 }),
        "expected FutureBlock, got: {err}"
    );
}

#[test]
fn test_future_block_boundary_allowed() {
    // Exactly current_time + 15 should be allowed (not strictly greater).
    let (sk, addr) = make_signer(b"future_boundary_signer");
    let seal_hash = keccak256(b"future boundary seal hash");
    let seal = sign_seal(&sk, &seal_hash);

    let params = HeaderValidationParams {
        number: 0,
        timestamp: 1015, // current_time(1000) + 15 = boundary
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_extra_data_with_seal(&seal),
        gas_limit: 30_000_000,
        seal_hash,
        has_ommers: false,
    };

    // Should succeed (not FutureBlock)
    let result = validate_header(&params, &[addr], &BTreeMap::new(), 1000);
    assert!(result.is_ok(), "boundary timestamp should be allowed: {result:?}");
}

// Note: Go tests multiple "future block" modes for Rio and Bhilai forks.
// Our implementation has a single MAX_FUTURE_BLOCK_TIME=15s check with no
// fork-specific modes, so those sub-cases are not portable.

// ===========================================================================
// Go TestVerifyHeader case 3: missing vanity → errMissingVanity
// ===========================================================================

#[test]
fn test_missing_vanity_short_extra_data() {
    // Go: extra data too short to contain the 32-byte vanity prefix.
    // Our API: ExtraData::parse returns TooShort → InvalidExtraData.
    let params = HeaderValidationParams {
        number: 100,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: vec![0u8; 10], // way too short
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: false,
    };

    let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::InvalidExtraData(_)),
        "expected InvalidExtraData for short extra data, got: {err}"
    );
}

// ===========================================================================
// Go TestVerifyHeader case 4: missing signature → errMissingSignature
// ===========================================================================

#[test]
fn test_missing_signature_short_extra_data() {
    // Go: extra data has vanity but not enough bytes for the 65-byte seal.
    // Our API: same as above — ExtraData::parse TooShort → InvalidExtraData.
    let params = HeaderValidationParams {
        number: 100,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: vec![0u8; EXTRADATA_VANITY_LEN + 10], // vanity present, seal too short
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: false,
    };

    let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::InvalidExtraData(_)),
        "expected InvalidExtraData for missing signature, got: {err}"
    );
}

// ===========================================================================
// Go TestVerifyHeader case 5: invalid mix digest (non-zero) → errInvalidMixDigest
// ===========================================================================

#[test]
fn test_nonzero_mix_hash() {
    let params = HeaderValidationParams {
        number: 100,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::from([0xff; 32]),
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: false,
    };

    let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::NonZeroMixHash),
        "expected NonZeroMixHash, got: {err}"
    );
}

// ===========================================================================
// Go TestVerifyHeader case 6: invalid uncle hash → errInvalidUncleHash
// ===========================================================================

#[test]
fn test_nonempty_ommers_via_validate_header() {
    // Go: uncle hash != EmptyUncleHash → errInvalidUncleHash
    // Our API: has_ommers=true → NonEmptyOmmers
    let params = HeaderValidationParams {
        number: 100,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: true,
    };

    let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::NonEmptyOmmers),
        "expected NonEmptyOmmers, got: {err}"
    );
}

#[test]
fn test_nonempty_ommers_via_block_pre_execution() {
    let err = validate_block_pre_execution(
        100,
        &make_minimal_extra_data(),
        true, // has_ommers
        false,
        6400,
        None,
    )
    .unwrap_err();
    assert!(
        matches!(err, ValidationError::NonEmptyOmmers),
        "expected NonEmptyOmmers from pre-execution, got: {err}"
    );
}

// ===========================================================================
// Go TestVerifyHeader case 7: nil difficulty → errInvalidDifficulty
// ===========================================================================

#[test]
fn test_wrong_difficulty() {
    // Go: difficulty == nil → errInvalidDifficulty
    // Our API: difficulty is U256 (no nil), so we test wrong value → WrongDifficulty.
    let (sk, addr) = make_signer(b"wrong_diff_signer");
    let seal_hash = keccak256(b"wrong diff seal hash");
    let seal = sign_seal(&sk, &seal_hash);

    let params = HeaderValidationParams {
        number: 0,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(999), // wrong — should be 1 for single validator inturn
        extra_data: make_extra_data_with_seal(&seal),
        gas_limit: 30_000_000,
        seal_hash,
        has_ommers: false,
    };

    let err = validate_header(&params, &[addr], &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::WrongDifficulty { .. }),
        "expected WrongDifficulty, got: {err}"
    );
}

// ===========================================================================
// Go TestVerifyHeader case 8: gas limit exceeds maximum → invalid gasLimit
// ===========================================================================
// SKIPPED — Our `validate_header` does not enforce gas limit bounds.
// InvalidGasLimit exists but is only used for parent-based gas limit delta
// checks, which are not yet implemented.

// ===========================================================================
// Go TestVerifyHeader case 9: unexpected withdrawals hash → ErrUnexpectedWithdrawals
// ===========================================================================

#[test]
fn test_nonempty_withdrawals_via_block_pre_execution() {
    let err = validate_block_pre_execution(
        100,
        &make_minimal_extra_data(),
        false,
        true, // has_withdrawals
        6400,
        None,
    )
    .unwrap_err();
    assert!(
        matches!(err, ValidationError::NonEmptyWithdrawals),
        "expected NonEmptyWithdrawals, got: {err}"
    );
}

// ===========================================================================
// Go TestVerifyHeader case 10: unexpected requests hash → ErrUnexpectedRequests
// ===========================================================================
// SKIPPED — No corresponding variant in our ValidationError.
// Bor does not use EIP-7685 requests.

// ===========================================================================
// Additional tests: NonZeroNonce
// ===========================================================================

#[test]
fn test_nonzero_nonce() {
    let params = HeaderValidationParams {
        number: 100,
        timestamp: 1000,
        nonce: 1,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_minimal_extra_data(),
        gas_limit: 30_000_000,
        seal_hash: B256::ZERO,
        has_ommers: false,
    };

    let err = validate_header(&params, &[], &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::NonZeroNonce),
        "expected NonZeroNonce, got: {err}"
    );
}

// ===========================================================================
// Additional tests: UnauthorizedSigner
// ===========================================================================

#[test]
fn test_unauthorized_signer() {
    let (sk, _signer_addr) = make_signer(b"unauthorized_signer");
    let seal_hash = keccak256(b"unauthorized seal hash");
    let seal = sign_seal(&sk, &seal_hash);

    // Authorized set contains a different address
    let authorized = vec![Address::new([0xaa; 20])];

    let params = HeaderValidationParams {
        number: 0,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        difficulty: U256::from(1),
        extra_data: make_extra_data_with_seal(&seal),
        gas_limit: 30_000_000,
        seal_hash,
        has_ommers: false,
    };

    let err = validate_header(&params, &authorized, &BTreeMap::new(), 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::UnauthorizedSigner(_)),
        "expected UnauthorizedSigner, got: {err}"
    );
}

// ===========================================================================
// Additional tests: RecentlySigned
// ===========================================================================

#[test]
fn test_recently_signed() {
    let (sk, addr) = make_signer(b"recently_signed_signer");
    let seal_hash = keccak256(b"recently signed seal hash");
    let seal = sign_seal(&sk, &seal_hash);

    // Two authorized signers so limit = 2/2+1 = 2
    let other = Address::new([0xbb; 20]);
    let signers = vec![addr, other];

    // Record that `addr` signed block 9 (recent)
    let mut recents = BTreeMap::new();
    recents.insert(9u64, addr);

    let params = HeaderValidationParams {
        number: 10,
        timestamp: 1000,
        nonce: 0,
        mix_hash: B256::ZERO,
        // block 10 % 2 = 0 → inturn idx 0 = addr → difficulty = 2
        difficulty: U256::from(2),
        extra_data: make_extra_data_with_seal(&seal),
        gas_limit: 30_000_000,
        seal_hash,
        has_ommers: false,
    };

    let err = validate_header(&params, &signers, &recents, 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::RecentlySigned(_)),
        "expected RecentlySigned, got: {err}"
    );
}

// ===========================================================================
// validate_header_against_parent: TimestampNotIncreasing
// ===========================================================================

#[test]
fn test_timestamp_not_increasing() {
    let params = HeaderValidationParams {
        number: 101,
        timestamp: 999, // before parent
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
    assert!(
        matches!(
            err,
            ValidationError::TimestampNotIncreasing {
                block_time: 999,
                parent_time: 1000
            }
        ),
        "expected TimestampNotIncreasing, got: {err}"
    );
}

#[test]
fn test_timestamp_equal_to_parent_rejected() {
    let params = HeaderValidationParams {
        number: 101,
        timestamp: 1000, // equal to parent — must be strictly greater
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
    assert!(
        matches!(err, ValidationError::TimestampNotIncreasing { .. }),
        "equal timestamps should be rejected, got: {err}"
    );
}

// ===========================================================================
// validate_block_post_execution: StateRootMismatch
// ===========================================================================

#[test]
fn test_state_root_mismatch() {
    let expected = B256::from([0xaa; 32]);
    let actual = B256::from([0xbb; 32]);
    let root = B256::from([0xcc; 32]);

    let err =
        validate_block_post_execution(&expected, &actual, &root, &root, 1000, 1000).unwrap_err();
    assert!(
        matches!(err, ValidationError::StateRootMismatch { .. }),
        "expected StateRootMismatch, got: {err}"
    );
}

// ===========================================================================
// validate_block_post_execution: ReceiptRootMismatch
// ===========================================================================

#[test]
fn test_receipt_root_mismatch() {
    let root = B256::from([0xaa; 32]);
    let expected_receipt = B256::from([0xbb; 32]);
    let actual_receipt = B256::from([0xcc; 32]);

    let err = validate_block_post_execution(
        &root,
        &root,
        &expected_receipt,
        &actual_receipt,
        1000,
        1000,
    )
    .unwrap_err();
    assert!(
        matches!(err, ValidationError::ReceiptRootMismatch { .. }),
        "expected ReceiptRootMismatch, got: {err}"
    );
}

// ===========================================================================
// validate_block_post_execution: GasUsedMismatch
// ===========================================================================

#[test]
fn test_gas_used_mismatch() {
    let root = B256::from([0xaa; 32]);

    let err =
        validate_block_post_execution(&root, &root, &root, &root, 5000, 6000).unwrap_err();
    assert!(
        matches!(
            err,
            ValidationError::GasUsedMismatch {
                expected: 5000,
                got: 6000
            }
        ),
        "expected GasUsedMismatch, got: {err}"
    );
}

// ===========================================================================
// Happy path: a fully valid header passes all checks
// ===========================================================================

#[test]
fn test_valid_header_passes() {
    let (params, expected_addr, signers) = make_valid_header();
    let recovered = validate_header(&params, &signers, &BTreeMap::new(), 1000).unwrap();
    assert_eq!(recovered, expected_addr);
}

// ===========================================================================
// Port of Go errors_test.go: ValidationError Display output
// ===========================================================================

#[test]
fn test_validation_error_display_non_zero_nonce() {
    let err = ValidationError::NonZeroNonce;
    let msg = err.to_string();
    assert!(
        msg.contains("nonce"),
        "NonZeroNonce display should mention 'nonce': {msg}"
    );
}

#[test]
fn test_validation_error_display_non_zero_mix_hash() {
    let err = ValidationError::NonZeroMixHash;
    let msg = err.to_string();
    assert!(
        msg.contains("mix hash"),
        "NonZeroMixHash display should mention 'mix hash': {msg}"
    );
}

#[test]
fn test_validation_error_display_non_empty_ommers() {
    let err = ValidationError::NonEmptyOmmers;
    let msg = err.to_string();
    assert!(
        msg.contains("ommers"),
        "NonEmptyOmmers display should mention 'ommers': {msg}"
    );
}

#[test]
fn test_validation_error_display_invalid_extra_data() {
    let err = ValidationError::InvalidExtraData("too short".into());
    let msg = err.to_string();
    assert!(
        msg.contains("extra data") && msg.contains("too short"),
        "InvalidExtraData display should mention 'extra data' and reason: {msg}"
    );
}

#[test]
fn test_validation_error_display_wrong_difficulty() {
    let err = ValidationError::WrongDifficulty {
        expected: U256::from(5),
        got: U256::from(3),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("difficulty") && msg.contains("5") && msg.contains("3"),
        "WrongDifficulty display should include expected and got: {msg}"
    );
}

#[test]
fn test_validation_error_display_future_block() {
    let err = ValidationError::FutureBlock {
        block_time: 2000,
        now: 1000,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("future") && msg.contains("2000") && msg.contains("1000"),
        "FutureBlock display should include timestamps: {msg}"
    );
}

#[test]
fn test_validation_error_display_unauthorized_signer() {
    let addr = Address::new([0xab; 20]);
    let err = ValidationError::UnauthorizedSigner(addr);
    let msg = err.to_string();
    assert!(
        msg.contains("unauthorized"),
        "UnauthorizedSigner display should mention 'unauthorized': {msg}"
    );
}

#[test]
fn test_validation_error_display_recently_signed() {
    let addr = Address::new([0xcd; 20]);
    let err = ValidationError::RecentlySigned(addr);
    let msg = err.to_string();
    assert!(
        msg.contains("recently"),
        "RecentlySigned display should mention 'recently': {msg}"
    );
}

#[test]
fn test_validation_error_display_seal_error() {
    let err = ValidationError::SealError("bad sig".into());
    let msg = err.to_string();
    assert!(
        msg.contains("seal") && msg.contains("bad sig"),
        "SealError display should include reason: {msg}"
    );
}

#[test]
fn test_validation_error_display_invalid_gas_limit() {
    let err = ValidationError::InvalidGasLimit {
        expected: 30_000_000,
        got: 60_000_000,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("gas limit") && msg.contains("30000000") && msg.contains("60000000"),
        "InvalidGasLimit display should include values: {msg}"
    );
}

#[test]
fn test_validation_error_display_invalid_base_fee() {
    let err = ValidationError::InvalidBaseFee {
        expected: U256::from(100),
        got: U256::from(200),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("base fee"),
        "InvalidBaseFee display should mention 'base fee': {msg}"
    );
}

#[test]
fn test_validation_error_display_timestamp_not_increasing() {
    let err = ValidationError::TimestampNotIncreasing {
        block_time: 999,
        parent_time: 1000,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("timestamp") && msg.contains("999") && msg.contains("1000"),
        "TimestampNotIncreasing display should include both times: {msg}"
    );
}

#[test]
fn test_validation_error_display_non_empty_withdrawals() {
    let err = ValidationError::NonEmptyWithdrawals;
    let msg = err.to_string();
    assert!(
        msg.contains("withdrawals"),
        "NonEmptyWithdrawals display should mention 'withdrawals': {msg}"
    );
}

#[test]
fn test_validation_error_display_missing_validators() {
    let err = ValidationError::MissingValidatorsAtSpanStart(6400);
    let msg = err.to_string();
    assert!(
        msg.contains("validators") && msg.contains("6400"),
        "MissingValidatorsAtSpanStart display should include block number: {msg}"
    );
}

#[test]
fn test_validation_error_display_state_root_mismatch() {
    let err = ValidationError::StateRootMismatch {
        expected: B256::from([0xaa; 32]),
        got: B256::from([0xbb; 32]),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("state root"),
        "StateRootMismatch display should mention 'state root': {msg}"
    );
}

#[test]
fn test_validation_error_display_receipt_root_mismatch() {
    let err = ValidationError::ReceiptRootMismatch {
        expected: B256::from([0xcc; 32]),
        got: B256::from([0xdd; 32]),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("receipt root"),
        "ReceiptRootMismatch display should mention 'receipt root': {msg}"
    );
}

#[test]
fn test_validation_error_display_gas_used_mismatch() {
    let err = ValidationError::GasUsedMismatch {
        expected: 5000,
        got: 6000,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("gas used") && msg.contains("5000") && msg.contains("6000"),
        "GasUsedMismatch display should include values: {msg}"
    );
}

// ===========================================================================
// Port of Go errors_test.go: SealError Display
// ===========================================================================

#[test]
fn test_seal_error_display_invalid_length() {
    let err = SealError::InvalidSignatureLength(42);
    let msg = err.to_string();
    assert!(
        msg.contains("65") && msg.contains("42"),
        "InvalidSignatureLength should show expected and actual: {msg}"
    );
}

#[test]
fn test_seal_error_display_recovery_failed() {
    let err = SealError::RecoveryFailed("bad curve point".into());
    let msg = err.to_string();
    assert!(
        msg.contains("recovery") && msg.contains("bad curve point"),
        "RecoveryFailed should include reason: {msg}"
    );
}

// ===========================================================================
// Port of Go errors_test.go: ExtraDataError Display
// ===========================================================================

#[test]
fn test_extra_data_error_display_too_short() {
    let err = ExtraDataError::TooShort(50);
    let msg = err.to_string();
    assert!(
        msg.contains("too short") && msg.contains("50"),
        "TooShort should include actual length: {msg}"
    );
}

#[test]
fn test_extra_data_error_display_invalid_validator_bytes() {
    let err = ExtraDataError::InvalidValidatorBytes(13);
    let msg = err.to_string();
    assert!(
        msg.contains("13") && msg.contains("20"),
        "InvalidValidatorBytes should include length and address size: {msg}"
    );
}

// ===========================================================================
// Extra: all errors implement Debug and Display (compile-time guarantee)
// ===========================================================================

#[test]
fn test_all_errors_are_debug_and_display() {
    // This test ensures every variant can be formatted without panicking.
    let errors: Vec<Box<dyn std::error::Error>> = vec![
        Box::new(ValidationError::NonZeroNonce),
        Box::new(ValidationError::NonZeroMixHash),
        Box::new(ValidationError::NonEmptyOmmers),
        Box::new(ValidationError::InvalidExtraData("test".into())),
        Box::new(ValidationError::WrongDifficulty {
            expected: U256::ZERO,
            got: U256::from(1),
        }),
        Box::new(ValidationError::FutureBlock {
            block_time: 0,
            now: 0,
        }),
        Box::new(ValidationError::UnauthorizedSigner(Address::ZERO)),
        Box::new(ValidationError::RecentlySigned(Address::ZERO)),
        Box::new(ValidationError::SealError("test".into())),
        Box::new(ValidationError::InvalidGasLimit {
            expected: 0,
            got: 0,
        }),
        Box::new(ValidationError::InvalidBaseFee {
            expected: U256::ZERO,
            got: U256::ZERO,
        }),
        Box::new(ValidationError::TimestampNotIncreasing {
            block_time: 0,
            parent_time: 0,
        }),
        Box::new(ValidationError::NonEmptyWithdrawals),
        Box::new(ValidationError::MissingValidatorsAtSpanStart(0)),
        Box::new(ValidationError::StateRootMismatch {
            expected: B256::ZERO,
            got: B256::ZERO,
        }),
        Box::new(ValidationError::ReceiptRootMismatch {
            expected: B256::ZERO,
            got: B256::ZERO,
        }),
        Box::new(ValidationError::GasUsedMismatch {
            expected: 0,
            got: 0,
        }),
    ];

    for err in &errors {
        let display = format!("{err}");
        let debug = format!("{err:?}");
        assert!(!display.is_empty(), "Display should not be empty");
        assert!(!debug.is_empty(), "Debug should not be empty");
    }
}
