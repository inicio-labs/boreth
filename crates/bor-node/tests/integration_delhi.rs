//! Integration test: Sync across Delhi hardfork boundary (mainnet).
//!
//! Verifies sprint size transition from 64 to 16 at block 38,189,056.

use bor_chainspec::params::sprint_size;
use bor_chainspec::BorHardfork;
use bor_consensus::block_validation::validate_block_pre_execution;
use bor_evm::plan_system_txs;
use bor_node::{BorNode, BorNodeConfig};

const DELHI_BLOCK: u64 = 38_189_056;

#[test]
fn test_delhi_sprint_size_transition() {
    // Pre-Delhi: sprint size is 64
    assert_eq!(sprint_size(DELHI_BLOCK - 1), 64);
    // At Delhi: sprint size is 16
    assert_eq!(sprint_size(DELHI_BLOCK), 16);
    // Post-Delhi: sprint size is 16
    assert_eq!(sprint_size(DELHI_BLOCK + 100), 16);
}

#[test]
fn test_delhi_boundary_blocks() {
    let node = BorNode::new(BorNodeConfig::mainnet()).unwrap();

    // Simulate blocks around Delhi boundary
    for block in (DELHI_BLOCK - 16)..=(DELHI_BLOCK + 16) {
        let current_sprint_size = sprint_size(block);

        // Pre-execution validation should pass for normal blocks
        validate_block_pre_execution(
            block,
            &vec![0u8; 97],
            false,
            false,
            6400, // span_size doesn't change at Delhi
            None,
        )
        .unwrap();

        // Plan system txs with correct sprint size
        let plan = plan_system_txs(block, current_sprint_size, 6400, false, &[]);

        // Sprint boundaries should be based on current sprint size
        if block > 0 && block % current_sprint_size == 0 {
            // This is a sprint boundary
            assert!(plan.state_sync_events.is_empty()); // no events in test
        }
    }
}

#[test]
fn test_delhi_fork_active() {
    let node = BorNode::new(BorNodeConfig::mainnet()).unwrap();
    assert!(node.chain_spec.is_bor_fork_active_at_block(BorHardfork::Delhi, DELHI_BLOCK));
    assert!(!node.chain_spec.is_bor_fork_active_at_block(BorHardfork::Delhi, DELHI_BLOCK - 1));
}
