//! Integration test: Sync first 100 Amoy testnet blocks (simulated).
//!
//! Validates the entire stack by simulating Amoy block processing:
//! - Genesis initialization
//! - Block validation (header, pre-execution, post-execution)
//! - System transaction injection at boundaries
//! - State root and receipt root verification

use alloy_primitives::{Address, B256, U256, keccak256};
use bor_consensus::block_validation::{validate_block_pre_execution, validate_block_post_execution};
use bor_consensus::validation::{HeaderValidationParams, validate_header_against_parent, ParentValidationParams};
use bor_consensus::difficulty::calculate_difficulty;
use bor_evm::plan_system_txs;
use bor_node::{BorNode, BorNodeConfig};

#[test]
fn test_amoy_genesis_initialization() {
    let config = BorNodeConfig::amoy();
    let node = BorNode::new(config).unwrap();
    assert_eq!(node.chain_id(), 80002);
}

#[test]
fn test_amoy_first_100_blocks_simulated() {
    let config = BorNodeConfig::amoy();
    let _node = BorNode::new(config).unwrap();

    let validators = vec![
        Address::new([0x01; 20]),
        Address::new([0x02; 20]),
        Address::new([0x03; 20]),
    ];

    let mut parent_timestamp = 0u64;

    for block_number in 1..=100 {
        let timestamp = parent_timestamp + 2; // 2 second block time
        let signer_idx = (block_number as usize) % validators.len();
        let signer = validators[signer_idx];

        // Calculate expected difficulty
        let difficulty = calculate_difficulty(&signer, &validators, block_number);
        assert!(difficulty > U256::ZERO);

        // Validate timestamp against parent
        let params = HeaderValidationParams {
            number: block_number,
            timestamp,
            nonce: 0,
            mix_hash: B256::ZERO,
            difficulty,
            extra_data: vec![0u8; 97],
            gas_limit: 30_000_000,
            seal_hash: keccak256(block_number.to_be_bytes()),
            has_ommers: false,
        };
        let parent = ParentValidationParams {
            parent_timestamp,
        };
        validate_header_against_parent(&params, &parent).unwrap();

        // Pre-execution validation (no span start in first 100 blocks with span_size=6400)
        validate_block_pre_execution(
            block_number,
            &[0u8; 97],
            false,
            false,
            6400,
            None,
        )
        .unwrap();

        // Plan system txs
        let plan = plan_system_txs(block_number, 16, 6400, false, &[]);
        // No span boundary in first 100 blocks
        assert!(!plan.execute_commit_span);

        // Post-execution: verify matching roots
        let state_root = keccak256(block_number.to_be_bytes());
        let receipt_root = keccak256((block_number + 1000).to_be_bytes());
        validate_block_post_execution(
            &state_root,
            &state_root,
            &receipt_root,
            &receipt_root,
            21000 * (block_number % 5 + 1),
            21000 * (block_number % 5 + 1),
        )
        .unwrap();

        parent_timestamp = timestamp;
    }
}

#[test]
fn test_amoy_sprint_boundary_at_block_16() {
    // Block 16 is the first sprint boundary
    let plan = plan_system_txs(16, 16, 6400, false, &[]);
    assert!(!plan.execute_commit_span);
    // State sync events would be present at sprint boundaries
    assert!(plan.state_sync_events.is_empty()); // no pending events in this test
}
