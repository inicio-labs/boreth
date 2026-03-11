//! Bor payload builder implementation.
//!
//! Selects transactions from the pool, injects system transactions at
//! sprint/span boundaries, and constructs the complete block payload.

use alloy_primitives::{Address, Bytes, U256};
use bor_evm::{SystemTxPlan, plan_system_txs, execute_system_tx_plan, SystemCallRecord};

/// Configuration for building a payload.
#[derive(Debug, Clone)]
pub struct PayloadConfig {
    /// Block number being built.
    pub block_number: u64,
    /// Block gas limit.
    pub gas_limit: u64,
    /// Sprint size at this block.
    pub sprint_size: u64,
    /// Span size at this block.
    pub span_size: u64,
    /// Block producer (signer) address.
    pub producer: Address,
    /// Block timestamp.
    pub timestamp: u64,
    /// Whether there is a pending span to commit.
    pub has_pending_span: bool,
    /// Pending span ID (if commitSpan needed).
    pub pending_span_id: Option<U256>,
    /// Pending validator bytes for commitSpan.
    pub pending_validator_bytes: Option<Bytes>,
    /// Pending state sync events for onStateReceive.
    pub pending_state_sync_events: Vec<(U256, Bytes)>,
}

/// A transaction in the payload.
#[derive(Debug, Clone)]
pub struct PayloadTx {
    /// Transaction data.
    pub data: Bytes,
    /// Gas used by this transaction.
    pub gas_used: u64,
    /// Whether this is a system transaction.
    pub is_system_tx: bool,
}

/// A built payload ready for sealing.
#[derive(Debug, Clone)]
pub struct BuiltPayload {
    /// Block number.
    pub block_number: u64,
    /// All transactions (user + system).
    pub transactions: Vec<PayloadTx>,
    /// Total gas used.
    pub total_gas_used: u64,
    /// System call records for receipt generation.
    pub system_calls: Vec<SystemCallRecord>,
    /// Whether commitSpan was included.
    pub commit_span_executed: bool,
    /// Number of state sync events included.
    pub state_sync_count: usize,
}

/// Bor payload builder.
pub struct BorPayloadBuilder;

impl BorPayloadBuilder {
    /// Build a payload from the given configuration and user transactions.
    ///
    /// User transactions are included first (up to gas limit), then system
    /// transactions are appended at the appropriate boundaries.
    pub fn build(
        config: &PayloadConfig,
        user_txs: Vec<PayloadTx>,
    ) -> BuiltPayload {
        let mut transactions = Vec::new();
        let mut total_gas_used = 0u64;

        // 1. Include user transactions (respecting gas limit)
        for tx in user_txs {
            if total_gas_used + tx.gas_used > config.gas_limit {
                break;
            }
            total_gas_used += tx.gas_used;
            transactions.push(tx);
        }

        // 2. Plan system transactions
        let plan = plan_system_txs(
            config.block_number,
            config.sprint_size,
            config.span_size,
            config.has_pending_span,
            &config.pending_state_sync_events,
        );

        // 3. Execute system transaction plan
        let result = execute_system_tx_plan(
            &plan,
            config.pending_span_id,
            config.pending_validator_bytes.clone(),
        );

        // 4. Append system transactions (0 gas)
        for call in &result.system_calls {
            transactions.push(PayloadTx {
                data: call.data.clone(),
                gas_used: 0, // System txs use 0 gas
                is_system_tx: true,
            });
        }

        BuiltPayload {
            block_number: config.block_number,
            transactions,
            total_gas_used,
            system_calls: result.system_calls,
            commit_span_executed: result.commit_span_executed,
            state_sync_count: result.state_sync_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn make_user_tx(gas: u64) -> PayloadTx {
        PayloadTx {
            data: Bytes::from_static(b"user_tx"),
            gas_used: gas,
            is_system_tx: false,
        }
    }

    #[test]
    fn test_payload_normal_block() {
        let config = make_config(5);
        let txs = vec![make_user_tx(21000), make_user_tx(21000)];
        let payload = BorPayloadBuilder::build(&config, txs);

        assert_eq!(payload.transactions.len(), 2);
        assert_eq!(payload.total_gas_used, 42000);
        assert!(!payload.commit_span_executed);
        assert_eq!(payload.state_sync_count, 0);
    }

    #[test]
    fn test_payload_sprint_boundary() {
        let mut config = make_config(16);
        config.pending_state_sync_events = vec![
            (U256::from(1), Bytes::from_static(b"sync1")),
            (U256::from(2), Bytes::from_static(b"sync2")),
        ];

        let txs = vec![make_user_tx(21000)];
        let payload = BorPayloadBuilder::build(&config, txs);

        // 1 user tx + 2 state sync system txs
        assert_eq!(payload.transactions.len(), 3);
        assert_eq!(payload.total_gas_used, 21000); // Only user tx gas
        assert!(!payload.commit_span_executed);
        assert_eq!(payload.state_sync_count, 2);

        // Last 2 are system txs
        assert!(!payload.transactions[0].is_system_tx);
        assert!(payload.transactions[1].is_system_tx);
        assert!(payload.transactions[2].is_system_tx);
    }

    #[test]
    fn test_payload_span_boundary() {
        let mut config = make_config(6400);
        config.has_pending_span = true;
        config.pending_span_id = Some(U256::from(1));
        config.pending_validator_bytes = Some(Bytes::from_static(&[0xbb; 20]));
        config.pending_state_sync_events = vec![
            (U256::from(10), Bytes::from_static(b"event")),
        ];

        let txs = vec![make_user_tx(50000)];
        let payload = BorPayloadBuilder::build(&config, txs);

        // 1 user tx + 1 commitSpan + 1 onStateReceive
        assert_eq!(payload.transactions.len(), 3);
        assert!(payload.commit_span_executed);
        assert_eq!(payload.state_sync_count, 1);

        // commitSpan before state sync
        assert!(!payload.transactions[0].is_system_tx);
        assert!(payload.transactions[1].is_system_tx); // commitSpan
        assert!(payload.transactions[2].is_system_tx); // onStateReceive
    }

    #[test]
    fn test_payload_respects_gas_limit() {
        let mut config = make_config(5);
        config.gas_limit = 50000;

        let txs = vec![
            make_user_tx(30000),
            make_user_tx(30000), // This would exceed gas limit
        ];
        let payload = BorPayloadBuilder::build(&config, txs);

        // Only first tx fits
        assert_eq!(payload.transactions.len(), 1);
        assert_eq!(payload.total_gas_used, 30000);
    }

    #[test]
    fn test_payload_post_madhugiri() {
        // Post-Madhugiri: state sync tx is the last tx in the block
        let mut config = make_config(80_084_816); // Post-Madhugiri sprint boundary (80084816 % 16 == 0)
        config.pending_state_sync_events = vec![
            (U256::from(999), Bytes::from_static(b"post_madhugiri_sync")),
        ];

        let txs = vec![make_user_tx(21000)];
        let payload = BorPayloadBuilder::build(&config, txs);

        assert_eq!(payload.transactions.len(), 2);
        assert_eq!(payload.state_sync_count, 1);
        // State sync tx is last
        assert!(payload.transactions.last().unwrap().is_system_tx);
    }

    #[test]
    fn test_payload_empty_block() {
        let config = make_config(5);
        let payload = BorPayloadBuilder::build(&config, vec![]);

        assert!(payload.transactions.is_empty());
        assert_eq!(payload.total_gas_used, 0);
    }
}
