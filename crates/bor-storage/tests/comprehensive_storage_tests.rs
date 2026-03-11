//! Comprehensive storage tests covering receipt storage, gas derivation,
//! persistence, and receipt key edge cases.

use alloy_primitives::B256;
use bor_storage::gas::{derive_bor_receipt_gas, is_bor_system_tx};
use bor_storage::persistence::{InMemorySnapshotStore, InMemorySpanStore, SnapshotStore, SpanStore};
use bor_storage::receipt_key::{bor_receipt_key, derived_bor_tx_hash};
use bor_storage::{compute_receipt_root, is_post_madhugiri, store_block_receipts};

// ===== 2.1/2.2/2.3 Receipt root pre/post Madhugiri =====

#[test]
fn test_pre_madhugiri_bor_excluded_from_receipt_root() {
    let regular = vec![
        B256::from([0x01; 32]),
        B256::from([0x02; 32]),
        B256::from([0x03; 32]),
        B256::from([0x04; 32]),
        B256::from([0x05; 32]),
    ];
    let bor = B256::from([0xff; 32]);

    let root = compute_receipt_root(&regular, Some(&bor), 50_000_000);
    assert_eq!(root.len(), 5, "pre-Madhugiri: bor receipt must be EXCLUDED");
    assert!(!root.contains(&bor));
}

#[test]
fn test_post_madhugiri_bor_included_in_receipt_root() {
    let regular = vec![
        B256::from([0x01; 32]),
        B256::from([0x02; 32]),
        B256::from([0x03; 32]),
        B256::from([0x04; 32]),
        B256::from([0x05; 32]),
    ];
    let bor = B256::from([0xff; 32]);

    let root = compute_receipt_root(&regular, Some(&bor), 81_000_000);
    assert_eq!(root.len(), 6, "post-Madhugiri: bor receipt must be INCLUDED");
    assert!(root.contains(&bor));
    // Bor receipt should be LAST
    assert_eq!(root.last().unwrap(), &bor);
}

#[test]
fn test_madhugiri_boundary_exact_transition() {
    let regular = vec![B256::from([0x01; 32])];
    let bor = B256::from([0xff; 32]);

    let pre = compute_receipt_root(&regular, Some(&bor), 80_084_799);
    assert_eq!(pre.len(), 1, "block 80_084_799: bor excluded");
    assert!(!pre.contains(&bor));

    let post = compute_receipt_root(&regular, Some(&bor), 80_084_800);
    assert_eq!(post.len(), 2, "block 80_084_800: bor included");
    assert!(post.contains(&bor));
}

// ===== 2.4 Empty block receipt handling =====

#[test]
fn test_pre_madhugiri_empty_block_no_receipts() {
    let root = compute_receipt_root(&[], None, 50_000_000);
    assert!(root.is_empty());
}

#[test]
fn test_post_madhugiri_empty_block_no_receipts() {
    let root = compute_receipt_root(&[], None, 81_000_000);
    assert!(root.is_empty());
}

#[test]
fn test_post_madhugiri_sprint_boundary_only_bor() {
    let bor = B256::from([0xff; 32]);
    let root = compute_receipt_root(&[], Some(&bor), 81_000_000);
    assert_eq!(root.len(), 1, "post-Madhugiri: bor receipt alone in root");
    assert_eq!(root[0], bor);
}

#[test]
fn test_pre_madhugiri_sprint_boundary_bor_excluded() {
    let bor = B256::from([0xff; 32]);
    let root = compute_receipt_root(&[], Some(&bor), 50_000_000);
    assert!(root.is_empty(), "pre-Madhugiri: bor receipt excluded even if only receipt");
}

// ===== 2.5 Bor receipt cumulative gas =====

#[test]
fn test_bor_receipt_gas_equals_last_regular() {
    // Regular txs: 100k, 200k, 300k cumulative
    // Bor system txs use 0 gas
    assert_eq!(derive_bor_receipt_gas(300_000, 0), 300_000);
    assert_eq!(derive_bor_receipt_gas(300_000, 1), 300_000);
    assert_eq!(derive_bor_receipt_gas(300_000, 5), 300_000);
}

#[test]
fn test_bor_receipt_gas_zero_regular() {
    assert_eq!(derive_bor_receipt_gas(0, 0), 0);
}

// ===== is_bor_system_tx =====

#[test]
fn test_bor_system_tx_detection() {
    // 10 txs, last 3 are bor
    assert!(!is_bor_system_tx(0, 10, 3));
    assert!(!is_bor_system_tx(6, 10, 3));
    assert!(is_bor_system_tx(7, 10, 3));
    assert!(is_bor_system_tx(8, 10, 3));
    assert!(is_bor_system_tx(9, 10, 3));
}

#[test]
fn test_all_txs_are_bor() {
    assert!(is_bor_system_tx(0, 3, 3));
    assert!(is_bor_system_tx(1, 3, 3));
    assert!(is_bor_system_tx(2, 3, 3));
}

// ===== 1.2 Block number encoding =====

#[test]
fn test_receipt_key_block_number_encodings() {
    let hash = B256::ZERO;

    // Verify big-endian encoding for various block numbers
    let cases: Vec<(u64, [u8; 8])> = vec![
        (0, [0, 0, 0, 0, 0, 0, 0, 0]),
        (1, [0, 0, 0, 0, 0, 0, 0, 1]),
        (256, [0, 0, 0, 0, 0, 0, 1, 0]),
        (1u64 << 32, [0, 0, 0, 1, 0, 0, 0, 0]),
        ((1u64 << 32) - 1, [0, 0, 0, 0, 255, 255, 255, 255]),
        (u64::MAX, [255, 255, 255, 255, 255, 255, 255, 255]),
    ];

    for (block, expected_bytes) in cases {
        let key = bor_receipt_key(block, &hash);
        assert_eq!(
            &key[18..26],
            &expected_bytes,
            "Block {block} encoding mismatch"
        );
    }
}

#[test]
fn test_le_encoding_produces_different_key() {
    let hash = B256::ZERO;
    let block = 256u64;

    let be_key = bor_receipt_key(block, &hash);

    // Manually construct LE key
    let mut le_key = Vec::new();
    le_key.extend_from_slice(b"matic-bor-receipt-");
    le_key.extend_from_slice(&block.to_le_bytes());
    le_key.extend_from_slice(hash.as_slice());

    assert_ne!(be_key, le_key, "LE encoding must differ from BE");
}

// ===== 1.3 Block hash is raw bytes =====

#[test]
fn test_hash_is_raw_bytes_not_hex() {
    let hash = B256::from([0xab; 32]);
    let key = bor_receipt_key(100, &hash);

    // Raw bytes: key[26..58] == hash bytes
    assert_eq!(&key[26..], hash.as_slice());

    // Hex string would be different
    let hex = format!("{hash:x}");
    assert_ne!(&key[26..], hex.as_bytes());
}

// ===== 1.4 Receipt key transition at Madhugiri =====

#[test]
fn test_madhugiri_storage_transition() {
    let hash = B256::from([0xab; 32]);

    let pre = store_block_receipts(80_084_799, &hash);
    let post = store_block_receipts(80_084_800, &hash);

    assert!(pre.separate, "pre-Madhugiri: separate storage");
    assert!(!post.separate, "post-Madhugiri: unified storage");

    // Keys differ because block numbers differ
    assert_ne!(pre.key, post.key);
}

// ===== 1.5 Receipt key collision resistance =====

#[test]
fn test_10000_consecutive_blocks_unique_keys() {
    let hash = B256::from([0xaa; 32]);
    let mut keys: Vec<Vec<u8>> = Vec::new();

    for block in 0..10_000 {
        keys.push(bor_receipt_key(block, &hash));
    }

    // Check no duplicates (using sort + dedup)
    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), keys.len(), "all 10,000 keys must be unique");
}

#[test]
fn test_same_block_different_hashes_different_keys() {
    let block = 1_000_000u64;
    let hash_a = B256::from([0x01; 32]);
    let hash_b = B256::from([0x02; 32]);
    assert_ne!(bor_receipt_key(block, &hash_a), bor_receipt_key(block, &hash_b));
}

#[test]
fn test_different_blocks_same_hash_different_keys() {
    let hash = B256::from([0xab; 32]);
    assert_ne!(bor_receipt_key(100, &hash), bor_receipt_key(101, &hash));
}

// ===== Derived tx hash =====

#[test]
fn test_derived_tx_hash_is_keccak_of_key() {
    let hash = B256::from([0xab; 32]);
    let key = bor_receipt_key(100, &hash);
    let tx_hash = derived_bor_tx_hash(100, &hash);
    assert_eq!(tx_hash, alloy_primitives::keccak256(&key));
}

#[test]
fn test_derived_tx_hash_unique_per_block() {
    let hash = B256::from([0xab; 32]);
    let h1 = derived_bor_tx_hash(100, &hash);
    let h2 = derived_bor_tx_hash(101, &hash);
    assert_ne!(h1, h2);
}

// ===== 9.1 SpanStore latest_span_id with gaps =====

#[test]
fn test_span_store_latest_with_gaps() {
    use bor_primitives::{Span, Validator, ValidatorSet};

    fn make_span(id: u64) -> Span {
        Span {
            id,
            start_block: id * 6400,
            end_block: (id + 1) * 6400 - 1,
            validator_set: ValidatorSet {
                validators: vec![Validator {
                    id: 1,
                    address: alloy_primitives::Address::ZERO,
                    voting_power: 100,
                    signer: alloy_primitives::Address::ZERO,
                    proposer_priority: 0,
                }],
                proposer: None,
            },
            selected_producers: vec![],
            bor_chain_id: "137".to_string(),
        }
    }

    let mut store = InMemorySpanStore::new();

    // Insert with gaps
    store.put_span(make_span(1));
    store.put_span(make_span(3));
    store.put_span(make_span(5));
    assert_eq!(store.latest_span_id(), Some(5));

    // Insert in reverse order
    let mut store2 = InMemorySpanStore::new();
    store2.put_span(make_span(5));
    store2.put_span(make_span(3));
    store2.put_span(make_span(1));
    assert_eq!(store2.latest_span_id(), Some(5));

    // Span 0
    let mut store3 = InMemorySpanStore::new();
    store3.put_span(make_span(0));
    assert_eq!(store3.latest_span_id(), Some(0));

    // Empty
    let store4 = InMemorySpanStore::new();
    assert_eq!(store4.latest_span_id(), None);
}

// ===== 9.2 SnapshotStore overwrite =====

#[test]
fn test_snapshot_store_overwrite_last_write_wins() {
    let mut store = InMemorySnapshotStore::new();
    let hash = [0xffu8; 32];

    store.put_snapshot(hash, vec![1, 2, 3]);
    store.put_snapshot(hash, vec![4, 5, 6]);

    let retrieved = store.get_snapshot(&hash).unwrap();
    assert_eq!(retrieved, vec![4, 5, 6], "last write wins");
}

#[test]
fn test_snapshot_store_get_nonexistent() {
    let store = InMemorySnapshotStore::new();
    assert!(store.get_snapshot(&[0x01; 32]).is_none());
}

// ===== is_post_madhugiri edge cases =====

#[test]
fn test_is_post_madhugiri_boundaries() {
    assert!(!is_post_madhugiri(0));
    assert!(!is_post_madhugiri(80_084_799));
    assert!(is_post_madhugiri(80_084_800));
    assert!(is_post_madhugiri(80_084_801));
    assert!(is_post_madhugiri(u64::MAX));
}
