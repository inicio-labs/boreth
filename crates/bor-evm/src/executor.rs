//! Bor block executor: processes full blocks with system transactions.
//!
//! Execution order is critical for correct state roots:
//! 1. Execute user transactions
//! 2. Execute commitSpan (at span boundaries)
//! 3. Execute onStateReceive (at sprint boundaries, block % sprint_size == 0)
//!
//! Pre-Madhugiri: Bor system tx receipts stored separately.
//! Post-Madhugiri: Bor system tx receipts unified with regular receipts.

use alloy_primitives::{Address, Bytes, U256};
use crate::system_call::{CommitSpanCall, StateReceiveCall};

/// Result of executing a block's system transactions.
#[derive(Debug, Clone, Default)]
pub struct SystemTxResult {
    /// Whether a commitSpan was executed.
    pub commit_span_executed: bool,
    /// Number of state sync events processed.
    pub state_sync_count: usize,
    /// Call data for each system call executed (for receipt generation).
    pub system_calls: Vec<SystemCallRecord>,
}

/// Record of a system call execution.
#[derive(Debug, Clone)]
pub struct SystemCallRecord {
    /// Target contract address.
    pub to: Address,
    /// Caller address (SYSTEM_ADDRESS).
    pub from: Address,
    /// ABI-encoded call data.
    pub data: Bytes,
}

/// Determines which system transactions to execute for a given block.
pub fn plan_system_txs(
    block_number: u64,
    sprint_size: u64,
    span_size: u64,
    has_pending_span: bool,
    pending_state_sync_events: &[(U256, Bytes)],
) -> SystemTxPlan {
    let is_sprint_boundary = block_number > 0 && block_number % sprint_size == 0;
    let is_span_boundary = block_number > 0 && block_number % span_size == 0;

    SystemTxPlan {
        execute_commit_span: is_span_boundary && has_pending_span,
        state_sync_events: if is_sprint_boundary {
            pending_state_sync_events.to_vec()
        } else {
            vec![]
        },
    }
}

/// Plan describing which system transactions to execute.
#[derive(Debug, Clone)]
pub struct SystemTxPlan {
    /// Whether to execute commitSpan.
    pub execute_commit_span: bool,
    /// State sync events to process via onStateReceive.
    pub state_sync_events: Vec<(U256, Bytes)>,
}

/// Execute the system transaction plan, returning records of what was executed.
pub fn execute_system_tx_plan(
    plan: &SystemTxPlan,
    span_id: Option<U256>,
    validator_bytes: Option<Bytes>,
) -> SystemTxResult {
    let mut result = SystemTxResult::default();
    let mut calls = Vec::new();

    // 1. commitSpan (at span boundaries)
    if plan.execute_commit_span {
        if let (Some(span_id), Some(val_bytes)) = (span_id, validator_bytes) {
            let call = CommitSpanCall {
                span_id,
                validator_bytes: val_bytes,
            };
            calls.push(SystemCallRecord {
                to: CommitSpanCall::to_address(),
                from: CommitSpanCall::caller(),
                data: call.call_data(),
            });
            result.commit_span_executed = true;
        }
    }

    // 2. onStateReceive (at sprint boundaries)
    for (state_id, data) in &plan.state_sync_events {
        let call = StateReceiveCall {
            state_id: *state_id,
            data: data.clone(),
        };
        calls.push(SystemCallRecord {
            to: StateReceiveCall::to_address(),
            from: StateReceiveCall::caller(),
            data: call.call_data(),
        });
        result.state_sync_count += 1;
    }

    result.system_calls = calls;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_block_no_system_txs() {
        // Block 5 with sprint_size=16, span_size=6400: not a boundary
        let plan = plan_system_txs(5, 16, 6400, false, &[]);
        assert!(!plan.execute_commit_span);
        assert!(plan.state_sync_events.is_empty());
    }

    #[test]
    fn test_sprint_boundary_block() {
        // Block 16 is a sprint boundary (16 % 16 == 0)
        let events = vec![
            (U256::from(1), Bytes::from_static(b"event1")),
            (U256::from(2), Bytes::from_static(b"event2")),
        ];
        let plan = plan_system_txs(16, 16, 6400, false, &events);
        assert!(!plan.execute_commit_span);
        assert_eq!(plan.state_sync_events.len(), 2);
    }

    #[test]
    fn test_span_boundary_block() {
        // Block 6400 is both span and sprint boundary
        let events = vec![(U256::from(1), Bytes::from_static(b"event1"))];
        let plan = plan_system_txs(6400, 16, 6400, true, &events);
        assert!(plan.execute_commit_span);
        assert_eq!(plan.state_sync_events.len(), 1);
    }

    #[test]
    fn test_empty_block_valid() {
        // Block 7 (not a boundary), no events, no pending span
        let plan = plan_system_txs(7, 16, 6400, false, &[]);
        assert!(!plan.execute_commit_span);
        assert!(plan.state_sync_events.is_empty());

        let result = execute_system_tx_plan(&plan, None, None);
        assert!(!result.commit_span_executed);
        assert_eq!(result.state_sync_count, 0);
        assert!(result.system_calls.is_empty());
    }

    #[test]
    fn test_execution_order() {
        // At a span+sprint boundary, commitSpan comes before onStateReceive
        let events = vec![
            (U256::from(10), Bytes::from_static(b"sync1")),
            (U256::from(11), Bytes::from_static(b"sync2")),
        ];
        let plan = plan_system_txs(6400, 16, 6400, true, &events);

        let result = execute_system_tx_plan(
            &plan,
            Some(U256::from(1)),
            Some(Bytes::from_static(&[0xaa; 20])),
        );

        assert!(result.commit_span_executed);
        assert_eq!(result.state_sync_count, 2);
        assert_eq!(result.system_calls.len(), 3);

        // First call should be commitSpan (to 0x1000)
        assert_eq!(result.system_calls[0].to, CommitSpanCall::to_address());
        // Second and third should be onStateReceive (to 0x1001)
        assert_eq!(result.system_calls[1].to, StateReceiveCall::to_address());
        assert_eq!(result.system_calls[2].to, StateReceiveCall::to_address());
    }

    #[test]
    fn test_block_zero_not_boundary() {
        // Block 0 should not trigger system txs even though 0 % 16 == 0
        let plan = plan_system_txs(0, 16, 6400, true, &[(U256::from(1), Bytes::from_static(b"x"))]);
        assert!(!plan.execute_commit_span);
        assert!(plan.state_sync_events.is_empty());
    }

    #[test]
    fn test_sprint_boundary_no_events() {
        // Sprint boundary but no pending events → empty state sync
        let plan = plan_system_txs(32, 16, 6400, false, &[]);
        assert!(!plan.execute_commit_span);
        assert!(plan.state_sync_events.is_empty());

        let result = execute_system_tx_plan(&plan, None, None);
        assert_eq!(result.state_sync_count, 0);
    }

    #[test]
    fn test_span_boundary_no_pending_span() {
        // Span boundary but no pending span data → no commitSpan
        let plan = plan_system_txs(6400, 16, 6400, false, &[]);
        assert!(!plan.execute_commit_span);
    }
}
