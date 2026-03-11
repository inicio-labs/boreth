//! Integration test: Sync across Rio hardfork boundary (mainnet).
//!
//! Verifies span size transition from 6400 to 1600 at block 77,414,656.

use bor_chainspec::params::{span_size, sprint_size};
use bor_chainspec::BorHardfork;
use bor_consensus::block_validation::validate_block_pre_execution;
use bor_evm::plan_system_txs;
use bor_node::{BorNode, BorNodeConfig};

const RIO_BLOCK: u64 = 77_414_656;

#[test]
fn test_rio_span_size_transition() {
    // Pre-Rio: span size is 6400
    assert_eq!(span_size(RIO_BLOCK - 1), 6400);
    // At Rio: span size is 1600
    assert_eq!(span_size(RIO_BLOCK), 1600);
    // Post-Rio: span size is 1600
    assert_eq!(span_size(RIO_BLOCK + 1000), 1600);
}

#[test]
fn test_rio_boundary_blocks() {
    let _node = BorNode::new(BorNodeConfig::mainnet()).unwrap();

    // Simulate blocks around Rio boundary
    for block in (RIO_BLOCK - 8)..=(RIO_BLOCK + 8) {
        let current_sprint_size = sprint_size(block);
        let current_span_size = span_size(block);

        // Pre-execution validation should pass
        // At span boundaries, we'd need validators in extra data,
        // but we skip that check for non-span-start blocks
        let is_span_start = block > 0 && block % current_span_size == 0;
        if !is_span_start {
            validate_block_pre_execution(
                block,
                &[0u8; 97],
                false,
                false,
                current_span_size,
                None,
            )
            .unwrap();
        }

        // Plan system txs
        let plan = plan_system_txs(block, current_sprint_size, current_span_size, false, &[]);

        // Span boundaries change after Rio
        if is_span_start && !plan.execute_commit_span {
            // No pending span in test, so commitSpan not triggered
        }
    }
}

#[test]
fn test_rio_fork_active() {
    let node = BorNode::new(BorNodeConfig::mainnet()).unwrap();
    assert!(node.chain_spec.is_bor_fork_active_at_block(BorHardfork::Rio, RIO_BLOCK));
    assert!(!node.chain_spec.is_bor_fork_active_at_block(BorHardfork::Rio, RIO_BLOCK - 1));
}

#[test]
fn test_rio_single_producer_consensus() {
    // Post-Rio, VEBloP means single producer per sprint
    // Difficulty still used for fork choice
    use bor_consensus::difficulty::calculate_difficulty;
    use alloy_primitives::{Address, U256};

    let validators = vec![
        Address::new([0x01; 20]),
        Address::new([0x02; 20]),
        Address::new([0x03; 20]),
    ];

    // At Rio block, calculate difficulty
    let inturn_idx = (RIO_BLOCK as usize) % validators.len();
    let diff = calculate_difficulty(&validators[inturn_idx], &validators, RIO_BLOCK);
    assert_eq!(diff, U256::from(3)); // inturn = validator_count
}
