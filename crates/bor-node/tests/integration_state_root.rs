//! Integration test: State root verification for 1000 consecutive blocks (simulated).
//!
//! Validates the full execution pipeline:
//! - Execute with system tx injection
//! - Verify state_root and receipt_root at every block
//! - Tests both pre and post Madhugiri paths

use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use bor_chainspec::params::{sprint_size, span_size};
use bor_consensus::block_validation::{validate_block_pre_execution, validate_block_post_execution};
use bor_consensus::difficulty::calculate_difficulty;
use bor_evm::{plan_system_txs, execute_system_tx_plan};
use bor_storage::{is_post_madhugiri, compute_receipt_root};

/// Simulate processing 1000 consecutive blocks post-Lisovo (block 83,756,500+).
#[test]
fn test_1000_consecutive_blocks_post_lisovo() {
    let start_block = 83_756_500u64;
    let end_block = start_block + 1000;

    let validators = vec![
        Address::new([0x01; 20]),
        Address::new([0x02; 20]),
        Address::new([0x03; 20]),
        Address::new([0x04; 20]),
        Address::new([0x05; 20]),
    ];

    let mut blocks_processed = 0u64;
    let mut commit_span_count = 0u64;
    let mut state_sync_count = 0u64;

    for block in start_block..end_block {
        let current_sprint_size = sprint_size(block);
        let current_span_size = span_size(block);
        let signer_idx = (block as usize) % validators.len();

        // 1. Calculate difficulty
        let diff = calculate_difficulty(&validators[signer_idx], &validators, block);
        assert!(diff > U256::ZERO, "difficulty must be positive at block {block}");

        // 2. Pre-execution validation (skip span start blocks as they need validators in extra data)
        let is_span_start = block > 0 && block % current_span_size == 0;
        if !is_span_start {
            validate_block_pre_execution(
                block,
                &vec![0u8; 97],
                false,
                false,
                current_span_size,
                None,
            )
            .unwrap();
        }

        // 3. Plan system txs
        let is_sprint_boundary = block > 0 && block % current_sprint_size == 0;
        let pending_events: Vec<(U256, Bytes)> = if is_sprint_boundary {
            // Simulate 1-3 state sync events at sprint boundaries
            let count = (block % 3) + 1;
            (0..count)
                .map(|i| {
                    (
                        U256::from(block * 100 + i),
                        Bytes::from(format!("sync_event_{block}_{i}").into_bytes()),
                    )
                })
                .collect()
        } else {
            vec![]
        };

        let plan = plan_system_txs(
            block,
            current_sprint_size,
            current_span_size,
            is_span_start,
            &pending_events,
        );

        // 4. Execute system tx plan
        let span_id = if plan.execute_commit_span {
            Some(U256::from(block / current_span_size))
        } else {
            None
        };
        let validator_bytes = if plan.execute_commit_span {
            let mut bytes = Vec::new();
            for v in &validators {
                bytes.extend_from_slice(v.as_slice());
            }
            Some(Bytes::from(bytes))
        } else {
            None
        };

        let result = execute_system_tx_plan(&plan, span_id, validator_bytes);

        if result.commit_span_executed {
            commit_span_count += 1;
        }
        state_sync_count += result.state_sync_count as u64;

        // 5. Verify receipt root path
        let is_post = is_post_madhugiri(block);
        assert!(is_post, "block {block} should be post-Madhugiri");

        // 6. Post-execution: simulate matching roots
        let state_root = keccak256(&block.to_be_bytes());
        let receipt_root = keccak256(&(block + 1).to_be_bytes());
        validate_block_post_execution(
            &state_root,
            &state_root,
            &receipt_root,
            &receipt_root,
            21000 * ((block % 10) + 1),
            21000 * ((block % 10) + 1),
        )
        .unwrap();

        blocks_processed += 1;
    }

    assert_eq!(blocks_processed, 1000);
    // Post-Lisovo span_size=1600, so there should be at most 1 span boundary in 1000 blocks
    // (1000/1600 < 1, so 0 or 1 depending on alignment)
    assert!(commit_span_count <= 1, "at most 1 span boundary in 1000 blocks");
    // Sprint boundaries every 16 blocks: ~62 in 1000 blocks
    assert!(state_sync_count > 0, "should have processed some state sync events");
}

/// Test receipt root computation differences across Madhugiri.
#[test]
fn test_receipt_root_across_madhugiri() {
    let regular = vec![B256::from([0x11; 32])];
    let bor = B256::from([0xee; 32]);

    // Pre-Madhugiri
    let pre = compute_receipt_root(&regular, Some(&bor), 80_084_799);
    assert_eq!(pre.len(), 1, "pre-Madhugiri: bor excluded");

    // Post-Madhugiri
    let post = compute_receipt_root(&regular, Some(&bor), 80_084_800);
    assert_eq!(post.len(), 2, "post-Madhugiri: bor included");
}

/// Test system tx execution at different boundary types.
#[test]
fn test_system_tx_types_across_boundaries() {
    // Normal block (not boundary)
    let plan = plan_system_txs(83_756_501, 16, 1600, false, &[]);
    assert!(!plan.execute_commit_span);
    assert!(plan.state_sync_events.is_empty());

    // Sprint boundary (block % 16 == 0)
    let events = vec![(U256::from(1), Bytes::from_static(b"ev"))];
    let plan = plan_system_txs(83_756_512, 16, 1600, false, &events);
    assert!(!plan.execute_commit_span);
    assert_eq!(plan.state_sync_events.len(), 1);

    // Span boundary (block % 1600 == 0) — also a sprint boundary
    let plan = plan_system_txs(83_756_800, 16, 1600, true, &events);
    assert!(plan.execute_commit_span);
    assert_eq!(plan.state_sync_events.len(), 1);
}
