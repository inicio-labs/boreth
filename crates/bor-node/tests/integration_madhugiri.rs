//! Integration test: Sync across Madhugiri hardfork boundary (mainnet).
//!
//! Verifies receipt storage path changes at block 80,084,800:
//! - Pre-Madhugiri: Bor receipts stored separately, NOT in receipt root
//! - Post-Madhugiri: Bor receipts unified with regular receipts

use alloy_primitives::{B256, keccak256};
use bor_chainspec::BorHardfork;
use bor_consensus::block_validation::{validate_block_pre_execution, validate_block_post_execution};
use bor_storage::{is_post_madhugiri, store_block_receipts, compute_receipt_root};
use bor_node::{BorNode, BorNodeConfig};

const MADHUGIRI_BLOCK: u64 = 80_084_800;

#[test]
fn test_madhugiri_receipt_storage_transition() {
    assert!(!is_post_madhugiri(MADHUGIRI_BLOCK - 1));
    assert!(is_post_madhugiri(MADHUGIRI_BLOCK));
}

#[test]
fn test_pre_madhugiri_separate_receipt_storage() {
    let block_hash = B256::from([0xab; 32]);
    let storage = store_block_receipts(MADHUGIRI_BLOCK - 1, &block_hash);
    assert!(storage.separate, "pre-Madhugiri receipts must be stored separately");
}

#[test]
fn test_post_madhugiri_unified_receipt_storage() {
    let block_hash = B256::from([0xab; 32]);
    let storage = store_block_receipts(MADHUGIRI_BLOCK, &block_hash);
    assert!(!storage.separate, "post-Madhugiri receipts must be unified");
}

#[test]
fn test_madhugiri_boundary_blocks() {
    let node = BorNode::new(BorNodeConfig::mainnet()).unwrap();

    // Pre-Madhugiri block: Bor receipt excluded from receipt root
    let regular_hashes = vec![B256::from([0x01; 32]), B256::from([0x02; 32])];
    let bor_hash = B256::from([0xff; 32]);

    let pre_root_hashes = compute_receipt_root(&regular_hashes, Some(&bor_hash), MADHUGIRI_BLOCK - 1);
    assert_eq!(pre_root_hashes.len(), 2, "pre-Madhugiri: bor receipt excluded");

    // Post-Madhugiri block: Bor receipt included in receipt root
    let post_root_hashes = compute_receipt_root(&regular_hashes, Some(&bor_hash), MADHUGIRI_BLOCK);
    assert_eq!(post_root_hashes.len(), 3, "post-Madhugiri: bor receipt included");
    assert!(post_root_hashes.contains(&bor_hash));
}

#[test]
fn test_madhugiri_fork_active() {
    let node = BorNode::new(BorNodeConfig::mainnet()).unwrap();
    assert!(node.chain_spec.is_bor_fork_active_at_block(BorHardfork::Madhugiri, MADHUGIRI_BLOCK));
    assert!(!node.chain_spec.is_bor_fork_active_at_block(BorHardfork::Madhugiri, MADHUGIRI_BLOCK - 1));
}

#[test]
fn test_madhugiri_boundary_post_execution() {
    // Simulate post-execution at the exact boundary
    let state_root = keccak256(&MADHUGIRI_BLOCK.to_be_bytes());
    let receipt_root = keccak256(&(MADHUGIRI_BLOCK + 1).to_be_bytes());

    // Both state root and receipt root must match
    validate_block_post_execution(
        &state_root,
        &state_root,
        &receipt_root,
        &receipt_root,
        1_000_000,
        1_000_000,
    )
    .unwrap();
}
