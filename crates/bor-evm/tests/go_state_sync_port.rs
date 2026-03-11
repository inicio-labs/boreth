// These tests port behaviors from Go Bor's tx_state_sync_test.go. Go tests
// StateSyncTx RLP encoding; we test ABI call_data encoding which serves a
// similar purpose.

use alloy_primitives::{Bytes, U256};
use bor_evm::{CommitSpanCall, StateReceiveCall, prepare_state_sync_calls};

// ---------------------------------------------------------------------------
// 1. Encoding determinism: same input to StateReceiveCall::call_data() produces
//    identical output
// ---------------------------------------------------------------------------
#[test]
fn state_receive_encoding_determinism() {
    let call = StateReceiveCall {
        state_id: U256::from(42),
        data: Bytes::from_static(b"hello state sync"),
    };
    let encoded1 = call.call_data();
    let encoded2 = call.call_data();
    assert_eq!(encoded1, encoded2, "same input must produce identical call_data");
}

// ---------------------------------------------------------------------------
// 2. Field sensitivity: changing state_id produces different call_data
// ---------------------------------------------------------------------------
#[test]
fn state_receive_state_id_sensitivity() {
    let call_a = StateReceiveCall {
        state_id: U256::from(1),
        data: Bytes::from_static(b"same_data"),
    };
    let call_b = StateReceiveCall {
        state_id: U256::from(2),
        data: Bytes::from_static(b"same_data"),
    };
    assert_ne!(
        call_a.call_data(),
        call_b.call_data(),
        "different state_id must produce different call_data"
    );
}

// ---------------------------------------------------------------------------
// 3. Field sensitivity: changing data produces different call_data
// ---------------------------------------------------------------------------
#[test]
fn state_receive_data_sensitivity() {
    let call_a = StateReceiveCall {
        state_id: U256::from(100),
        data: Bytes::from_static(b"data_alpha"),
    };
    let call_b = StateReceiveCall {
        state_id: U256::from(100),
        data: Bytes::from_static(b"data_bravo"),
    };
    assert_ne!(
        call_a.call_data(),
        call_b.call_data(),
        "different data must produce different call_data"
    );
}

// ---------------------------------------------------------------------------
// 4. Empty data: StateReceiveCall with empty data still produces valid call_data
// ---------------------------------------------------------------------------
#[test]
fn state_receive_empty_data_valid() {
    let call = StateReceiveCall {
        state_id: U256::from(7),
        data: Bytes::new(),
    };
    let encoded = call.call_data();
    // Must at least contain the 4-byte selector plus ABI-encoded state_id and offset
    assert!(
        encoded.len() >= 4 + 64,
        "empty data call_data must still have selector + ABI header, got {} bytes",
        encoded.len()
    );
    // First 4 bytes are the onStateReceive selector
    assert_eq!(&encoded[..4], &[0x26, 0xc5, 0x3b, 0xea]);
}

// ---------------------------------------------------------------------------
// 5. CommitSpan encoding determinism: same input produces same output
// ---------------------------------------------------------------------------
#[test]
fn commit_span_encoding_determinism() {
    let call = CommitSpanCall {
        span_id: U256::from(99),
        validator_bytes: Bytes::from_static(&[0xaa; 60]),
    };
    let encoded1 = call.call_data();
    let encoded2 = call.call_data();
    assert_eq!(encoded1, encoded2, "same input must produce identical call_data");
}

// ---------------------------------------------------------------------------
// 6. CommitSpan field sensitivity: different span_id -> different call_data
// ---------------------------------------------------------------------------
#[test]
fn commit_span_span_id_sensitivity() {
    let call_a = CommitSpanCall {
        span_id: U256::from(10),
        validator_bytes: Bytes::from_static(&[0xbb; 20]),
    };
    let call_b = CommitSpanCall {
        span_id: U256::from(11),
        validator_bytes: Bytes::from_static(&[0xbb; 20]),
    };
    assert_ne!(
        call_a.call_data(),
        call_b.call_data(),
        "different span_id must produce different call_data"
    );
}

// ---------------------------------------------------------------------------
// 7. CommitSpan field sensitivity: different validator_bytes -> different call_data
// ---------------------------------------------------------------------------
#[test]
fn commit_span_validator_bytes_sensitivity() {
    let call_a = CommitSpanCall {
        span_id: U256::from(5),
        validator_bytes: Bytes::from_static(&[0xaa; 20]),
    };
    let call_b = CommitSpanCall {
        span_id: U256::from(5),
        validator_bytes: Bytes::from_static(&[0xbb; 20]),
    };
    assert_ne!(
        call_a.call_data(),
        call_b.call_data(),
        "different validator_bytes must produce different call_data"
    );
}

// ---------------------------------------------------------------------------
// 8. prepare_state_sync_calls preserves order and maps fields correctly
// ---------------------------------------------------------------------------
#[test]
fn prepare_state_sync_calls_preserves_order_and_fields() {
    let events = vec![
        (U256::from(10), Bytes::from_static(b"first")),
        (U256::from(20), Bytes::from_static(b"second")),
        (U256::from(30), Bytes::from_static(b"third")),
        (U256::from(40), Bytes::from_static(b"fourth")),
    ];

    let calls = prepare_state_sync_calls(&events);
    assert_eq!(calls.len(), 4);

    for (i, (expected_id, expected_data)) in events.iter().enumerate() {
        assert_eq!(
            calls[i].state_id, *expected_id,
            "state_id mismatch at index {i}"
        );
        assert_eq!(
            calls[i].data, *expected_data,
            "data mismatch at index {i}"
        );
        // Each call must produce valid call_data
        let cd = calls[i].call_data();
        assert!(cd.len() >= 4 + 64, "call_data too short at index {i}");
    }
}

// ---------------------------------------------------------------------------
// 9. prepare_state_sync_calls with empty input returns empty
// ---------------------------------------------------------------------------
#[test]
fn prepare_state_sync_calls_empty_returns_empty() {
    let calls = prepare_state_sync_calls(&[]);
    assert!(calls.is_empty(), "empty input must produce empty output");
}

// ---------------------------------------------------------------------------
// 10. Encoding is selector-prefixed: first 4 bytes are always the function selector
// ---------------------------------------------------------------------------
#[test]
fn encoding_is_selector_prefixed() {
    // onStateReceive selector: 0x26c53bea
    let state_call = StateReceiveCall {
        state_id: U256::from(1),
        data: Bytes::from_static(b"x"),
    };
    let state_cd = state_call.call_data();
    assert_eq!(
        &state_cd[..4],
        &[0x26, 0xc5, 0x3b, 0xea],
        "StateReceiveCall must start with onStateReceive selector"
    );

    // commitSpan selector: 0x60cc80d8
    let span_call = CommitSpanCall {
        span_id: U256::from(1),
        validator_bytes: Bytes::from_static(&[0xcc; 20]),
    };
    let span_cd = span_call.call_data();
    assert_eq!(
        &span_cd[..4],
        &[0x60, 0xcc, 0x80, 0xd8],
        "CommitSpanCall must start with commitSpan selector"
    );

    // Verify selector is stable across different inputs
    let state_call_2 = StateReceiveCall {
        state_id: U256::from(999),
        data: Bytes::from_static(b"completely different data"),
    };
    assert_eq!(
        &state_call_2.call_data()[..4],
        &state_cd[..4],
        "selector must be the same regardless of input"
    );

    let span_call_2 = CommitSpanCall {
        span_id: U256::from(999),
        validator_bytes: Bytes::from_static(&[0xff; 100]),
    };
    assert_eq!(
        &span_call_2.call_data()[..4],
        &span_cd[..4],
        "selector must be the same regardless of input"
    );
}
