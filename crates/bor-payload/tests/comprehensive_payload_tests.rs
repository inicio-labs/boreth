//! Comprehensive integration tests for the Bor payload builder.

use alloy_primitives::{Address, Bytes, U256};
use bor_payload::{BorPayloadBuilder, PayloadConfig};
use bor_payload::builder::PayloadTx;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a default `PayloadConfig` for the given block number.
///
/// Defaults: gas_limit=30_000_000, sprint_size=16, span_size=6400,
/// producer=0xaa..aa, timestamp=1000, no pending span data.
fn make_config(block_number: u64) -> PayloadConfig {
    PayloadConfig {
        block_number,
        gas_limit: 30_000_000,
        sprint_size: 16,
        span_size: 6400,
        producer: Address::new([0xaa; 20]),
        timestamp: 1000,
        has_pending_span: false,
        pending_span_id: None,
        pending_validator_bytes: None,
        pending_state_sync_events: vec![],
    }
}

/// Create a user transaction that consumes `gas` units.
fn make_user_tx(gas: u64) -> PayloadTx {
    PayloadTx {
        data: Bytes::from_static(b"user_tx"),
        gas_used: gas,
        is_system_tx: false,
    }
}

/// Create N state sync events with sequential IDs starting from `start_id`.
fn make_state_sync_events(start_id: u64, count: usize) -> Vec<(U256, Bytes)> {
    (0..count)
        .map(|i| {
            let id = U256::from(start_id + i as u64);
            let data = Bytes::copy_from_slice(format!("sync_event_{}", start_id + i as u64).as_bytes());
            (id, data)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Gas limit enforcement - multiple txs, only fitting ones included
// ---------------------------------------------------------------------------

#[test]
fn test_gas_limit_enforcement_partial_inclusion() {
    let mut config = make_config(5); // non-boundary block
    config.gas_limit = 100_000;

    let txs = vec![
        make_user_tx(40_000),
        make_user_tx(40_000),
        make_user_tx(40_000), // total would be 120k > 100k
    ];

    let payload = BorPayloadBuilder::build(&config, txs);

    // Only the first two txs fit (80k <= 100k), third is excluded (120k > 100k).
    assert_eq!(payload.transactions.len(), 2);
    assert_eq!(payload.total_gas_used, 80_000);
    // All included txs are user txs
    assert!(payload.transactions.iter().all(|tx| !tx.is_system_tx));
}

// ---------------------------------------------------------------------------
// 2. Gas limit exactly exhausted (no room for next tx)
// ---------------------------------------------------------------------------

#[test]
fn test_gas_limit_exactly_exhausted() {
    let mut config = make_config(7); // non-boundary block
    config.gas_limit = 60_000;

    let txs = vec![
        make_user_tx(30_000),
        make_user_tx(30_000),
        make_user_tx(1), // would push to 60_001 > 60_000
    ];

    let payload = BorPayloadBuilder::build(&config, txs);

    assert_eq!(payload.transactions.len(), 2);
    assert_eq!(payload.total_gas_used, 60_000);
}

// ---------------------------------------------------------------------------
// 3. Zero gas limit - no user txs but system txs still added at boundaries
// ---------------------------------------------------------------------------

#[test]
fn test_zero_gas_limit_system_txs_still_added() {
    let mut config = make_config(16); // sprint boundary
    config.gas_limit = 0;
    config.pending_state_sync_events = vec![
        (U256::from(1), Bytes::from_static(b"sync")),
    ];

    let txs = vec![make_user_tx(21_000)]; // should be excluded

    let payload = BorPayloadBuilder::build(&config, txs);

    // No user txs included (gas limit 0), but state sync system tx is present.
    assert_eq!(payload.total_gas_used, 0);
    // System txs still appended
    assert_eq!(payload.state_sync_count, 1);
    let system_txs: Vec<_> = payload.transactions.iter().filter(|t| t.is_system_tx).collect();
    assert_eq!(system_txs.len(), 1);
    let user_txs: Vec<_> = payload.transactions.iter().filter(|t| !t.is_system_tx).collect();
    assert!(user_txs.is_empty());
}

// ---------------------------------------------------------------------------
// 4. Span + sprint boundary with both commitSpan and state sync events
// ---------------------------------------------------------------------------

#[test]
fn test_span_and_sprint_boundary_both_commit_span_and_state_sync() {
    // 6400 is divisible by both 16 (sprint) and 6400 (span)
    let mut config = make_config(6400);
    config.has_pending_span = true;
    config.pending_span_id = Some(U256::from(1));
    config.pending_validator_bytes = Some(Bytes::from_static(&[0xbb; 20]));
    config.pending_state_sync_events = vec![
        (U256::from(100), Bytes::from_static(b"event_a")),
        (U256::from(101), Bytes::from_static(b"event_b")),
    ];

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    // 1 user tx + 1 commitSpan + 2 state sync = 4 transactions
    assert_eq!(payload.block_number, 6400);
    assert!(payload.commit_span_executed);
    assert_eq!(payload.state_sync_count, 2);
    assert_eq!(payload.total_gas_used, 21_000); // only user tx gas

    // First tx is user tx
    assert!(!payload.transactions[0].is_system_tx);
    // Remaining are system txs
    let system_count = payload.transactions.iter().filter(|t| t.is_system_tx).count();
    assert_eq!(system_count, 3); // 1 commitSpan + 2 state sync
}

// ---------------------------------------------------------------------------
// 5. Sprint boundary only (state sync events but no commitSpan)
// ---------------------------------------------------------------------------

#[test]
fn test_sprint_boundary_only_state_sync() {
    // 32 is divisible by 16 (sprint) but NOT by 6400 (span)
    let mut config = make_config(32);
    config.pending_state_sync_events = vec![
        (U256::from(5), Bytes::from_static(b"sync_only")),
    ];

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    assert!(!payload.commit_span_executed);
    assert_eq!(payload.state_sync_count, 1);
    assert_eq!(payload.transactions.len(), 2); // 1 user + 1 state sync
    assert!(!payload.transactions[0].is_system_tx);
    assert!(payload.transactions[1].is_system_tx);
}

// ---------------------------------------------------------------------------
// 6. No boundary (non-sprint block) - no system txs even with pending events
// ---------------------------------------------------------------------------

#[test]
fn test_no_boundary_no_system_txs() {
    let mut config = make_config(17); // 17 % 16 != 0
    config.has_pending_span = true;
    config.pending_span_id = Some(U256::from(2));
    config.pending_validator_bytes = Some(Bytes::from_static(&[0xcc; 20]));
    config.pending_state_sync_events = vec![
        (U256::from(50), Bytes::from_static(b"should_not_appear")),
    ];

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    // No system txs injected because not at any boundary
    assert_eq!(payload.transactions.len(), 1);
    assert!(!payload.commit_span_executed);
    assert_eq!(payload.state_sync_count, 0);
    assert_eq!(payload.total_gas_used, 21_000);
}

// ---------------------------------------------------------------------------
// 7. Empty user txs at sprint boundary - only system txs
// ---------------------------------------------------------------------------

#[test]
fn test_empty_user_txs_at_sprint_boundary() {
    let mut config = make_config(48); // 48 % 16 == 0
    config.pending_state_sync_events = vec![
        (U256::from(10), Bytes::from_static(b"sync_a")),
        (U256::from(11), Bytes::from_static(b"sync_b")),
        (U256::from(12), Bytes::from_static(b"sync_c")),
    ];

    let payload = BorPayloadBuilder::build(&config, vec![]);

    assert_eq!(payload.total_gas_used, 0);
    assert_eq!(payload.state_sync_count, 3);
    assert!(payload.transactions.iter().all(|t| t.is_system_tx));
    assert_eq!(payload.transactions.len(), 3);
}

// ---------------------------------------------------------------------------
// 8. System txs are appended AFTER user txs (ordering check)
// ---------------------------------------------------------------------------

#[test]
fn test_system_txs_appended_after_user_txs() {
    let mut config = make_config(6400); // span + sprint boundary
    config.has_pending_span = true;
    config.pending_span_id = Some(U256::from(1));
    config.pending_validator_bytes = Some(Bytes::from_static(&[0xdd; 20]));
    config.pending_state_sync_events = vec![
        (U256::from(20), Bytes::from_static(b"event")),
    ];

    let txs = vec![
        make_user_tx(10_000),
        make_user_tx(20_000),
        make_user_tx(30_000),
    ];

    let payload = BorPayloadBuilder::build(&config, txs);

    // 3 user txs first, then system txs
    let user_count = payload.transactions.iter().filter(|t| !t.is_system_tx).count();
    let system_count = payload.transactions.iter().filter(|t| t.is_system_tx).count();
    assert_eq!(user_count, 3);
    assert!(system_count >= 1); // at least commitSpan + state sync

    // Verify ordering: all user txs come before all system txs
    let first_system_idx = payload
        .transactions
        .iter()
        .position(|t| t.is_system_tx)
        .expect("should have system txs");
    let last_user_idx = payload
        .transactions
        .iter()
        .rposition(|t| !t.is_system_tx)
        .expect("should have user txs");

    assert!(
        last_user_idx < first_system_idx,
        "all user txs (last at {last_user_idx}) must come before all system txs (first at {first_system_idx})",
    );
}

// ---------------------------------------------------------------------------
// 9. Post-Rio span size (1600) boundaries
// ---------------------------------------------------------------------------

#[test]
fn test_post_rio_span_size_1600() {
    // Post-Rio uses span_size=1600
    let mut config = make_config(1600);
    config.span_size = 1600;
    config.has_pending_span = true;
    config.pending_span_id = Some(U256::from(1));
    config.pending_validator_bytes = Some(Bytes::from_static(&[0xee; 20]));
    config.pending_state_sync_events = vec![
        (U256::from(200), Bytes::from_static(b"rio_event")),
    ];

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    // 1600 % 16 == 0 (sprint) and 1600 % 1600 == 0 (span)
    assert!(payload.commit_span_executed);
    assert_eq!(payload.state_sync_count, 1);
    assert_eq!(payload.total_gas_used, 21_000);

    // Non-span boundary with post-Rio span size
    let config2 = PayloadConfig {
        block_number: 3200,
        span_size: 1600,
        has_pending_span: true,
        pending_span_id: Some(U256::from(2)),
        pending_validator_bytes: Some(Bytes::from_static(&[0xff; 20])),
        ..make_config(3200)
    };

    let payload2 = BorPayloadBuilder::build(&config2, vec![make_user_tx(21_000)]);
    // 3200 % 1600 == 0 so commitSpan should execute
    assert!(payload2.commit_span_executed);
}

// ---------------------------------------------------------------------------
// 10. Large number of state sync events (50 events)
// ---------------------------------------------------------------------------

#[test]
fn test_large_number_of_state_sync_events() {
    let mut config = make_config(16); // sprint boundary
    config.pending_state_sync_events = make_state_sync_events(1, 50);

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    assert_eq!(payload.state_sync_count, 50);
    assert!(!payload.commit_span_executed); // 16 % 6400 != 0

    // 1 user tx + 50 system txs
    let system_count = payload.transactions.iter().filter(|t| t.is_system_tx).count();
    assert_eq!(system_count, 50);
    assert_eq!(payload.total_gas_used, 21_000); // system txs use 0 gas

    // All system txs report 0 gas
    for tx in &payload.transactions {
        if tx.is_system_tx {
            assert_eq!(tx.gas_used, 0);
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Block 0 is never a boundary
// ---------------------------------------------------------------------------

#[test]
fn test_block_zero_is_never_a_boundary() {
    let mut config = make_config(0);
    config.has_pending_span = true;
    config.pending_span_id = Some(U256::from(0));
    config.pending_validator_bytes = Some(Bytes::from_static(&[0xaa; 20]));
    config.pending_state_sync_events = vec![
        (U256::from(1), Bytes::from_static(b"genesis_event")),
    ];

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    // Block 0 should NOT trigger any system transactions
    assert!(!payload.commit_span_executed);
    assert_eq!(payload.state_sync_count, 0);
    assert_eq!(payload.transactions.len(), 1);
    assert!(!payload.transactions[0].is_system_tx);
}

// ---------------------------------------------------------------------------
// 12. Multiple user txs with various gas amounts, verifying partial inclusion
// ---------------------------------------------------------------------------

#[test]
fn test_partial_inclusion_various_gas_amounts() {
    let mut config = make_config(10); // non-boundary
    config.gas_limit = 100_000;

    let txs = vec![
        make_user_tx(25_000), // cumulative: 25k
        make_user_tx(25_000), // cumulative: 50k
        make_user_tx(25_000), // cumulative: 75k
        make_user_tx(25_000), // cumulative: 100k -- exactly at limit
        make_user_tx(1),      // cumulative: 100_001 -- exceeds limit
    ];

    let payload = BorPayloadBuilder::build(&config, txs);

    assert_eq!(payload.transactions.len(), 4);
    assert_eq!(payload.total_gas_used, 100_000);
}

#[test]
fn test_partial_inclusion_mixed_sizes() {
    let mut config = make_config(3); // non-boundary
    config.gas_limit = 70_000;

    // Transactions of varying sizes; builder includes in order until one doesn't fit.
    let txs = vec![
        make_user_tx(10_000), // cumulative: 10k  -> fits
        make_user_tx(20_000), // cumulative: 30k  -> fits
        make_user_tx(30_000), // cumulative: 60k  -> fits
        make_user_tx(15_000), // cumulative: 75k  -> exceeds 70k, breaks
    ];

    let payload = BorPayloadBuilder::build(&config, txs);

    assert_eq!(payload.transactions.len(), 3);
    assert_eq!(payload.total_gas_used, 60_000);
}

// ---------------------------------------------------------------------------
// Additional edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_system_txs_use_zero_gas() {
    let mut config = make_config(6400);
    config.has_pending_span = true;
    config.pending_span_id = Some(U256::from(1));
    config.pending_validator_bytes = Some(Bytes::from_static(&[0xbb; 20]));
    config.pending_state_sync_events = make_state_sync_events(1, 5);

    let payload = BorPayloadBuilder::build(&config, vec![]);

    // All txs are system txs with 0 gas
    assert_eq!(payload.total_gas_used, 0);
    for tx in &payload.transactions {
        assert!(tx.is_system_tx);
        assert_eq!(tx.gas_used, 0);
    }
}

#[test]
fn test_sprint_boundary_no_pending_events() {
    // Sprint boundary but no pending state sync events
    let config = make_config(32); // 32 % 16 == 0

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    // No system txs because there are no pending events
    assert_eq!(payload.transactions.len(), 1);
    assert!(!payload.commit_span_executed);
    assert_eq!(payload.state_sync_count, 0);
}

#[test]
fn test_span_boundary_without_pending_span() {
    // At span boundary but has_pending_span is false
    let mut config = make_config(6400);
    config.has_pending_span = false; // no pending span
    config.pending_state_sync_events = vec![
        (U256::from(1), Bytes::from_static(b"sync")),
    ];

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    // commitSpan should NOT execute (no pending span)
    assert!(!payload.commit_span_executed);
    // State sync should still work at sprint boundary
    assert_eq!(payload.state_sync_count, 1);
}

#[test]
fn test_single_user_tx_exactly_at_gas_limit() {
    let mut config = make_config(5);
    config.gas_limit = 21_000;

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    assert_eq!(payload.transactions.len(), 1);
    assert_eq!(payload.total_gas_used, 21_000);
}

#[test]
fn test_single_user_tx_exceeds_gas_limit() {
    let mut config = make_config(5);
    config.gas_limit = 20_999;

    let txs = vec![make_user_tx(21_000)];
    let payload = BorPayloadBuilder::build(&config, txs);

    assert_eq!(payload.transactions.len(), 0);
    assert_eq!(payload.total_gas_used, 0);
}
