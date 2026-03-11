//! Comprehensive EVM and system transaction tests.
//!
//! Covers: system tx execution order, ABI encoding, boundary detection,
//! system caller address, state sync event ordering.

use alloy_primitives::{Address, Bytes, U256};
use bor_chainspec::constants::{
    BOR_VALIDATOR_SET_ADDRESS, STATE_RECEIVER_ADDRESS, SYSTEM_ADDRESS,
};
use bor_evm::system_call::{CommitSpanCall, StateReceiveCall, prepare_state_sync_calls};
use bor_evm::{execute_system_tx_plan, plan_system_txs};

// ===== 5.1 System tx execution ORDER =====

#[test]
fn test_execution_order_commit_span_before_state_sync() {
    // At span+sprint boundary: commitSpan MUST come before onStateReceive
    let events = vec![
        (U256::from(10), Bytes::from_static(b"sync1")),
        (U256::from(11), Bytes::from_static(b"sync2")),
        (U256::from(12), Bytes::from_static(b"sync3")),
    ];
    let plan = plan_system_txs(6400, 16, 6400, true, &events);

    let result = execute_system_tx_plan(
        &plan,
        Some(U256::from(1)),
        Some(Bytes::from_static(&[0xaa; 20])),
    );

    assert!(result.commit_span_executed);
    assert_eq!(result.state_sync_count, 3);
    assert_eq!(result.system_calls.len(), 4); // 1 commitSpan + 3 onStateReceive

    // First call MUST be commitSpan (to 0x1000)
    assert_eq!(result.system_calls[0].to, BOR_VALIDATOR_SET_ADDRESS);
    // Remaining calls MUST be onStateReceive (to 0x1001)
    assert_eq!(result.system_calls[1].to, STATE_RECEIVER_ADDRESS);
    assert_eq!(result.system_calls[2].to, STATE_RECEIVER_ADDRESS);
    assert_eq!(result.system_calls[3].to, STATE_RECEIVER_ADDRESS);
}

#[test]
fn test_sprint_only_boundary_no_commit_span() {
    // Sprint boundary but NOT span boundary: only state sync, no commitSpan
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(32, 16, 6400, false, &events);

    assert!(!plan.execute_commit_span);
    assert_eq!(plan.state_sync_events.len(), 1);

    let result = execute_system_tx_plan(&plan, None, None);
    assert!(!result.commit_span_executed);
    assert_eq!(result.state_sync_count, 1);
    assert_eq!(result.system_calls[0].to, STATE_RECEIVER_ADDRESS);
}

// ===== 5.2 commitSpan ABI encoding =====

#[test]
fn test_commit_span_selector() {
    let call = CommitSpanCall {
        span_id: U256::from(42),
        validator_bytes: Bytes::from_static(&[0xaa; 20]),
    };
    let data = call.call_data();
    // Selector for commitSpan(uint256,bytes) = 0x60cc80d8
    assert_eq!(&data[..4], &[0x60, 0xcc, 0x80, 0xd8]);
}

#[test]
fn test_commit_span_abi_encoding_roundtrip() {
    let call = CommitSpanCall {
        span_id: U256::from(42),
        validator_bytes: Bytes::from_static(&[0xaa; 20]),
    };
    let data = call.call_data();

    // Starts with selector
    assert_eq!(&data[..4], &[0x60, 0xcc, 0x80, 0xd8]);
    // Data should contain span_id encoded as uint256
    assert!(data.len() > 4 + 32);
}

#[test]
fn test_commit_span_zero_span_id() {
    let call = CommitSpanCall {
        span_id: U256::ZERO,
        validator_bytes: Bytes::new(),
    };
    let data = call.call_data();
    assert_eq!(&data[..4], &[0x60, 0xcc, 0x80, 0xd8]);
    assert!(data.len() >= 4);
}

#[test]
fn test_commit_span_large_span_id() {
    let call = CommitSpanCall {
        span_id: U256::MAX,
        validator_bytes: Bytes::from_static(&[0xbb; 20]),
    };
    let data = call.call_data();
    assert_eq!(&data[..4], &[0x60, 0xcc, 0x80, 0xd8]);
}

#[test]
fn test_commit_span_100_validators() {
    // 100 validators = 2000 bytes
    let val_bytes = vec![0xcc; 2000];
    let call = CommitSpanCall {
        span_id: U256::from(100),
        validator_bytes: Bytes::from(val_bytes),
    };
    let data = call.call_data();
    assert_eq!(&data[..4], &[0x60, 0xcc, 0x80, 0xd8]);
    assert!(data.len() > 4 + 32 + 2000);
}

// ===== 5.3 onStateReceive ABI encoding =====

#[test]
fn test_state_receive_selector() {
    let call = StateReceiveCall {
        state_id: U256::from(100),
        data: Bytes::from_static(b"state_data"),
    };
    let data = call.call_data();
    // Selector for onStateReceive(uint256,bytes) = 0x26c53bea
    assert_eq!(&data[..4], &[0x26, 0xc5, 0x3b, 0xea]);
}

#[test]
fn test_state_receive_empty_data() {
    let call = StateReceiveCall {
        state_id: U256::from(1),
        data: Bytes::new(),
    };
    let data = call.call_data();
    assert_eq!(&data[..4], &[0x26, 0xc5, 0x3b, 0xea]);
    assert!(data.len() >= 4);
}

#[test]
fn test_state_receive_large_data() {
    // 10KB+ data
    let big_data = vec![0xab; 10_240];
    let call = StateReceiveCall {
        state_id: U256::from(999),
        data: Bytes::from(big_data),
    };
    let data = call.call_data();
    assert_eq!(&data[..4], &[0x26, 0xc5, 0x3b, 0xea]);
    assert!(data.len() > 10_240);
}

// ===== 5.4 System calls use SYSTEM_ADDRESS =====

#[test]
fn test_commit_span_caller_is_system_address() {
    assert_eq!(
        CommitSpanCall::caller(),
        SYSTEM_ADDRESS,
        "commitSpan caller must be SYSTEM_ADDRESS"
    );
}

#[test]
fn test_state_receive_caller_is_system_address() {
    assert_eq!(
        StateReceiveCall::caller(),
        SYSTEM_ADDRESS,
        "onStateReceive caller must be SYSTEM_ADDRESS"
    );
}

#[test]
fn test_commit_span_target_is_validator_set_contract() {
    assert_eq!(
        CommitSpanCall::to_address(),
        BOR_VALIDATOR_SET_ADDRESS,
    );
}

#[test]
fn test_state_receive_target_is_state_receiver_contract() {
    assert_eq!(
        StateReceiveCall::to_address(),
        STATE_RECEIVER_ADDRESS,
    );
}

#[test]
fn test_system_calls_record_correct_addresses() {
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(6400, 16, 6400, true, &events);
    let result = execute_system_tx_plan(
        &plan,
        Some(U256::from(1)),
        Some(Bytes::from_static(&[0xaa; 20])),
    );

    for call in &result.system_calls {
        assert_eq!(call.from, SYSTEM_ADDRESS, "all system calls must use SYSTEM_ADDRESS as caller");
    }
}

// ===== 5.5 State sync event ordering =====

#[test]
fn test_prepare_state_sync_preserves_input_order() {
    let events = vec![
        (U256::from(5), Bytes::from_static(b"e5")),
        (U256::from(3), Bytes::from_static(b"e3")),
        (U256::from(7), Bytes::from_static(b"e7")),
        (U256::from(1), Bytes::from_static(b"e1")),
        (U256::from(9), Bytes::from_static(b"e9")),
    ];
    let calls = prepare_state_sync_calls(&events);

    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0].state_id, U256::from(5));
    assert_eq!(calls[1].state_id, U256::from(3));
    assert_eq!(calls[2].state_id, U256::from(7));
    assert_eq!(calls[3].state_id, U256::from(1));
    assert_eq!(calls[4].state_id, U256::from(9));
}

#[test]
fn test_empty_events() {
    let calls = prepare_state_sync_calls(&[]);
    assert!(calls.is_empty());
}

#[test]
fn test_sparse_state_ids() {
    // Gap in state_ids (1, 2, 5) is valid
    let events = vec![
        (U256::from(1), Bytes::from_static(b"e1")),
        (U256::from(2), Bytes::from_static(b"e2")),
        (U256::from(5), Bytes::from_static(b"e5")),
    ];
    let calls = prepare_state_sync_calls(&events);
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].state_id, U256::from(1));
    assert_eq!(calls[2].state_id, U256::from(5));
}

// ===== 5.6 Sprint vs Span boundary detection =====

#[test]
fn test_block_6400_is_both_sprint_and_span_boundary() {
    // 6400 % 16 == 0 (sprint) and 6400 % 6400 == 0 (span)
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(6400, 16, 6400, true, &events);
    assert!(plan.execute_commit_span);
    assert_eq!(plan.state_sync_events.len(), 1);
}

#[test]
fn test_block_16_is_sprint_not_span() {
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(16, 16, 6400, true, &events);
    assert!(!plan.execute_commit_span); // 16 % 6400 != 0
    assert_eq!(plan.state_sync_events.len(), 1);
}

#[test]
fn test_block_5_is_neither() {
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(5, 16, 6400, false, &events);
    assert!(!plan.execute_commit_span);
    assert!(plan.state_sync_events.is_empty());
}

#[test]
fn test_block_0_is_never_boundary() {
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(0, 16, 6400, true, &events);
    assert!(!plan.execute_commit_span);
    assert!(plan.state_sync_events.is_empty());
}

// Post-Rio boundaries
#[test]
fn test_post_rio_span_boundary_at_1600() {
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(1600, 16, 1600, true, &events);
    // 1600 % 1600 == 0 and 1600 % 16 == 0
    assert!(plan.execute_commit_span);
    assert_eq!(plan.state_sync_events.len(), 1);
}

#[test]
fn test_post_rio_sprint_at_16() {
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(16, 16, 1600, false, &events);
    assert!(!plan.execute_commit_span);
    assert_eq!(plan.state_sync_events.len(), 1);
}

#[test]
fn test_span_boundary_without_pending_span_no_commit() {
    // Span boundary but has_pending_span=false → no commitSpan
    let plan = plan_system_txs(6400, 16, 6400, false, &[]);
    assert!(!plan.execute_commit_span);
}

#[test]
fn test_span_boundary_with_pending_but_no_data() {
    // has_pending_span=true but no span_id/validator_bytes → commitSpan skipped
    let plan = plan_system_txs(6400, 16, 6400, true, &[]);
    let result = execute_system_tx_plan(&plan, None, None);
    assert!(!result.commit_span_executed);
}

// ===== Additional edge cases =====

#[test]
fn test_multiple_sprint_boundaries() {
    // Verify consecutive sprint boundaries
    for block in [16u64, 32, 48, 64, 80, 96] {
        let plan = plan_system_txs(block, 16, 6400, false, &[(U256::from(1), Bytes::from_static(b"e"))]);
        assert_eq!(plan.state_sync_events.len(), 1, "block {block} should be sprint boundary");
    }
}

#[test]
fn test_non_sprint_blocks_no_events() {
    for block in [1u64, 2, 3, 7, 9, 11, 13, 15, 17] {
        let plan = plan_system_txs(block, 16, 6400, false, &[(U256::from(1), Bytes::from_static(b"e"))]);
        assert!(plan.state_sync_events.is_empty(), "block {block} should NOT be sprint boundary");
    }
}

#[test]
fn test_system_address_is_correct() {
    // 0xfffffffffffffffffffffffffffffffffffffffe
    let mut bytes = [0xff; 20];
    bytes[19] = 0xfe;
    let expected = Address::new(bytes);
    assert_eq!(SYSTEM_ADDRESS, expected);
}
