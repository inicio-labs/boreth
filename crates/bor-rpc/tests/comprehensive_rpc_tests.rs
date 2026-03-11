use alloy_primitives::{keccak256, B256};
use bor_rpc::{compute_root_hash, BorRpcError};

// ---------------------------------------------------------------------------
// 1. Root hash of empty slice is B256::ZERO
// ---------------------------------------------------------------------------
#[test]
fn root_hash_empty_slice_returns_zero() {
    assert_eq!(compute_root_hash(&[]), B256::ZERO);
}

// ---------------------------------------------------------------------------
// 2. Root hash of single hash is the hash itself
// ---------------------------------------------------------------------------
#[test]
fn root_hash_single_element_returns_itself() {
    let h = B256::from([0xab; 32]);
    assert_eq!(compute_root_hash(&[h]), h);
}

// ---------------------------------------------------------------------------
// 3. Root hash of two hashes = keccak256(h1 || h2)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_two_elements_equals_keccak_of_concat() {
    let h1 = B256::from([0x01; 32]);
    let h2 = B256::from([0x02; 32]);

    let root = compute_root_hash(&[h1, h2]);

    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(h1.as_slice());
    combined[32..].copy_from_slice(h2.as_slice());
    let expected = keccak256(combined);

    assert_eq!(root, expected);
}

// ---------------------------------------------------------------------------
// 4. Root hash is deterministic (same input, same output)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_is_deterministic() {
    let hashes: Vec<B256> = (0..8).map(|i| B256::from([i as u8; 32])).collect();

    let root1 = compute_root_hash(&hashes);
    let root2 = compute_root_hash(&hashes);
    assert_eq!(root1, root2);
}

// ---------------------------------------------------------------------------
// 5. Root hash of 3 hashes (padded to 4 with B256::ZERO)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_three_elements_padded_to_four() {
    let h1 = B256::from([0x11; 32]);
    let h2 = B256::from([0x22; 32]);
    let h3 = B256::from([0x33; 32]);

    let root_3 = compute_root_hash(&[h1, h2, h3]);

    // Manually compute with explicit padding to 4
    let root_4 = compute_root_hash(&[h1, h2, h3, B256::ZERO]);

    assert_eq!(root_3, root_4);
    assert_ne!(root_3, B256::ZERO);
}

// ---------------------------------------------------------------------------
// 6. Root hash of 4 hashes (exact power of 2)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_four_elements_exact_power_of_two() {
    let h1 = B256::from([0x01; 32]);
    let h2 = B256::from([0x02; 32]);
    let h3 = B256::from([0x03; 32]);
    let h4 = B256::from([0x04; 32]);

    let root = compute_root_hash(&[h1, h2, h3, h4]);

    // Level 1: pair (h1,h2) and pair (h3,h4)
    let mut c12 = [0u8; 64];
    c12[..32].copy_from_slice(h1.as_slice());
    c12[32..].copy_from_slice(h2.as_slice());
    let left = keccak256(c12);

    let mut c34 = [0u8; 64];
    c34[..32].copy_from_slice(h3.as_slice());
    c34[32..].copy_from_slice(h4.as_slice());
    let right = keccak256(c34);

    // Level 2: pair (left, right)
    let mut c_root = [0u8; 64];
    c_root[..32].copy_from_slice(left.as_slice());
    c_root[32..].copy_from_slice(right.as_slice());
    let expected = keccak256(c_root);

    assert_eq!(root, expected);
}

// ---------------------------------------------------------------------------
// 7. Root hash of 8 hashes
// ---------------------------------------------------------------------------
#[test]
fn root_hash_eight_elements() {
    let hashes: Vec<B256> = (0..8).map(|i| B256::from([i as u8; 32])).collect();

    let root = compute_root_hash(&hashes);

    // 8 is a power of 2, no padding needed.
    // Verify via manual bottom-up Merkle computation.
    let pair_hash = |a: &B256, b: &B256| -> B256 {
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(a.as_slice());
        combined[32..].copy_from_slice(b.as_slice());
        keccak256(combined)
    };

    // Level 1 (4 nodes)
    let l1_0 = pair_hash(&hashes[0], &hashes[1]);
    let l1_1 = pair_hash(&hashes[2], &hashes[3]);
    let l1_2 = pair_hash(&hashes[4], &hashes[5]);
    let l1_3 = pair_hash(&hashes[6], &hashes[7]);

    // Level 2 (2 nodes)
    let l2_0 = pair_hash(&l1_0, &l1_1);
    let l2_1 = pair_hash(&l1_2, &l1_3);

    // Level 3 (root)
    let expected = pair_hash(&l2_0, &l2_1);

    assert_eq!(root, expected);
}

// ---------------------------------------------------------------------------
// 8. Root hash of 5 hashes (padded to 8)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_five_elements_padded_to_eight() {
    let hashes: Vec<B256> = (0..5).map(|i| B256::from([(i + 1) as u8; 32])).collect();

    let root_5 = compute_root_hash(&hashes);

    // Manually pad to 8 with B256::ZERO
    let mut padded = hashes.clone();
    while padded.len() < 8 {
        padded.push(B256::ZERO);
    }
    let root_8 = compute_root_hash(&padded);

    assert_eq!(root_5, root_8);
    assert_ne!(root_5, B256::ZERO);
}

// ---------------------------------------------------------------------------
// 9. Different inputs produce different root hashes
// ---------------------------------------------------------------------------
#[test]
fn different_inputs_produce_different_roots() {
    let hashes_a = vec![B256::from([0x01; 32]), B256::from([0x02; 32])];
    let hashes_b = vec![B256::from([0x03; 32]), B256::from([0x04; 32])];

    assert_ne!(compute_root_hash(&hashes_a), compute_root_hash(&hashes_b));
}

// ---------------------------------------------------------------------------
// 10. Order matters - swapping two hashes changes root
// ---------------------------------------------------------------------------
#[test]
fn swapping_hashes_changes_root() {
    let h1 = B256::from([0x01; 32]);
    let h2 = B256::from([0x02; 32]);

    let root_original = compute_root_hash(&[h1, h2]);
    let root_swapped = compute_root_hash(&[h2, h1]);

    assert_ne!(root_original, root_swapped);
}

// ---------------------------------------------------------------------------
// 11. Root hash with all identical hashes
// ---------------------------------------------------------------------------
#[test]
fn root_hash_all_identical_hashes() {
    let h = B256::from([0xff; 32]);
    let hashes = vec![h; 4];

    let root = compute_root_hash(&hashes);

    // keccak256(h || h) for each pair, then keccak256(pair || pair)
    let mut c = [0u8; 64];
    c[..32].copy_from_slice(h.as_slice());
    c[32..].copy_from_slice(h.as_slice());
    let pair = keccak256(c);

    let mut c2 = [0u8; 64];
    c2[..32].copy_from_slice(pair.as_slice());
    c2[32..].copy_from_slice(pair.as_slice());
    let expected = keccak256(c2);

    assert_eq!(root, expected);
}

// ---------------------------------------------------------------------------
// 12. Root hash with all-zero hashes
// ---------------------------------------------------------------------------
#[test]
fn root_hash_all_zero_hashes() {
    let hashes = vec![B256::ZERO; 4];

    let root = compute_root_hash(&hashes);

    // keccak256(0x00..00 || 0x00..00) for each pair, then hash the pair results
    let c = [0u8; 64]; // all zeros
    let pair = keccak256(c);

    let mut c2 = [0u8; 64];
    c2[..32].copy_from_slice(pair.as_slice());
    c2[32..].copy_from_slice(pair.as_slice());
    let expected = keccak256(c2);

    assert_eq!(root, expected);
    // The root of all-zero hashes should NOT be zero itself (keccak produces non-zero output)
    assert_ne!(root, B256::ZERO);
}

// ---------------------------------------------------------------------------
// 13. Error display strings for all BorRpcError variants
// ---------------------------------------------------------------------------
#[test]
fn error_display_block_not_found() {
    let err = BorRpcError::BlockNotFound(42);
    let msg = err.to_string();
    assert!(msg.contains("block not found"), "got: {msg}");
    assert!(msg.contains("42"), "got: {msg}");
}

#[test]
fn error_display_extra_data_error() {
    let err = BorRpcError::ExtraDataError("bad data".to_string());
    let msg = err.to_string();
    assert!(msg.contains("invalid extra data"), "got: {msg}");
    assert!(msg.contains("bad data"), "got: {msg}");
}

#[test]
fn error_display_invalid_block_range() {
    let err = BorRpcError::InvalidBlockRange {
        start: 100,
        end: 50,
    };
    let msg = err.to_string();
    assert!(msg.contains("start 100"), "got: {msg}");
    assert!(msg.contains("end 50"), "got: {msg}");
}
