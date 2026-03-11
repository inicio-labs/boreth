// These tests port behaviors from Go Bor's api_test.go. Many Go tests require
// full blockchain setup which we don't replicate; we focus on the pure function
// compute_root_hash.

use alloy_primitives::{keccak256, B256};
use bor_rpc::{BorRpcError, compute_root_hash};

/// Helper: generate `n` distinct hashes by keccak-hashing each index.
fn sequential_hashes(n: usize) -> Vec<B256> {
    (0..n)
        .map(|i| keccak256((i as u64).to_be_bytes()))
        .collect()
}

// ---------------------------------------------------------------------------
// 1. compute_root_hash with exactly 1024 hashes (large real-world checkpoint range)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_1024_hashes() {
    let hashes = sequential_hashes(1024);
    let root = compute_root_hash(&hashes);
    // 1024 is a power of 2, so no padding needed. Result must be non-zero.
    assert_ne!(root, B256::ZERO);
}

// ---------------------------------------------------------------------------
// 2. compute_root_hash deterministic across multiple calls
//    (Go tests verify GetRootHash is stable when tip advances past range end)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_deterministic_across_calls() {
    let hashes = sequential_hashes(100);
    let root1 = compute_root_hash(&hashes);
    let root2 = compute_root_hash(&hashes);
    let root3 = compute_root_hash(&hashes);
    assert_eq!(root1, root2);
    assert_eq!(root2, root3);
}

// ---------------------------------------------------------------------------
// 3. Swapping two adjacent hashes changes the root
// ---------------------------------------------------------------------------
#[test]
fn root_hash_swap_adjacent_changes_result() {
    let mut hashes = sequential_hashes(16);
    let root_before = compute_root_hash(&hashes);

    // Swap positions 4 and 5
    hashes.swap(4, 5);
    let root_after = compute_root_hash(&hashes);

    assert_ne!(root_before, root_after, "swapping adjacent hashes must change the root");
}

// ---------------------------------------------------------------------------
// 4. Prepending vs appending a hash produces different roots
// ---------------------------------------------------------------------------
#[test]
fn root_hash_prepend_vs_append_differ() {
    let base = sequential_hashes(8);
    let extra = keccak256(b"extra");

    let mut prepended = vec![extra];
    prepended.extend_from_slice(&base);

    let mut appended = base.clone();
    appended.push(extra);

    let root_prepend = compute_root_hash(&prepended);
    let root_append = compute_root_hash(&appended);

    assert_ne!(
        root_prepend, root_append,
        "prepending vs appending should produce different roots"
    );
}

// ---------------------------------------------------------------------------
// 5. 256 sequential hashes (typical checkpoint)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_256_hashes() {
    let hashes = sequential_hashes(256);
    let root = compute_root_hash(&hashes);
    assert_ne!(root, B256::ZERO);

    // Determinism
    assert_eq!(root, compute_root_hash(&hashes));
}

// ---------------------------------------------------------------------------
// 6. Power-of-2 sizes (2, 4, 8, 16, 32, 64, 128)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_power_of_two_sizes() {
    let sizes = [2, 4, 8, 16, 32, 64, 128];
    let mut previous_root = B256::ZERO;

    for &size in &sizes {
        let hashes = sequential_hashes(size);
        let root = compute_root_hash(&hashes);
        assert_ne!(root, B256::ZERO, "root for size {size} must be non-zero");
        // Each size should produce a unique root (since inputs are sequential_hashes(0..size))
        // size 2 is a prefix of size 4, but padding differs, so roots differ.
        assert_ne!(root, previous_root, "root for size {size} should differ from previous");
        previous_root = root;
    }
}

// ---------------------------------------------------------------------------
// 7. Non-power-of-2 sizes (3, 5, 7, 9, 15, 17, 31, 33)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_non_power_of_two_sizes() {
    let sizes = [3, 5, 7, 9, 15, 17, 31, 33];
    let mut roots = Vec::new();

    for &size in &sizes {
        let hashes = sequential_hashes(size);
        let root = compute_root_hash(&hashes);
        assert_ne!(root, B256::ZERO, "root for size {size} must be non-zero");
        // Should be deterministic
        assert_eq!(root, compute_root_hash(&hashes), "root for size {size} must be deterministic");
        roots.push(root);
    }

    // All roots should be unique
    for i in 0..roots.len() {
        for j in (i + 1)..roots.len() {
            assert_ne!(
                roots[i], roots[j],
                "roots for sizes {} and {} must differ",
                sizes[i], sizes[j]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8. BorRpcError display formatting for all variants
// ---------------------------------------------------------------------------
#[test]
fn bor_rpc_error_display_formatting() {
    let err_block = BorRpcError::BlockNotFound(42);
    assert_eq!(err_block.to_string(), "block not found: 42");

    let err_extra = BorRpcError::ExtraDataError("bad data".to_string());
    assert_eq!(err_extra.to_string(), "invalid extra data: bad data");

    let err_range = BorRpcError::InvalidBlockRange { start: 200, end: 100 };
    let msg = err_range.to_string();
    assert!(msg.contains("start 200"));
    assert!(msg.contains("end 100"));
    assert_eq!(msg, "invalid block range: start 200 > end 100");
}

// ---------------------------------------------------------------------------
// 9. All-same hashes produces deterministic root
// ---------------------------------------------------------------------------
#[test]
fn root_hash_all_same_hashes_deterministic() {
    let same_hash = B256::from([0xab; 32]);
    let hashes: Vec<B256> = vec![same_hash; 16];

    let root1 = compute_root_hash(&hashes);
    let root2 = compute_root_hash(&hashes);
    assert_eq!(root1, root2, "all-same hashes must produce deterministic root");
    assert_ne!(root1, B256::ZERO, "root must be non-zero for non-empty input");

    // All-same with different count should produce a different root
    let hashes_small: Vec<B256> = vec![same_hash; 8];
    let root_small = compute_root_hash(&hashes_small);
    assert_ne!(root1, root_small, "different counts of same hash should differ");
}

// ---------------------------------------------------------------------------
// 10. Commutative property does NOT hold (order matters)
// ---------------------------------------------------------------------------
#[test]
fn root_hash_order_matters_not_commutative() {
    let hashes = sequential_hashes(8);

    // Reverse the order
    let mut reversed = hashes.clone();
    reversed.reverse();

    let root_original = compute_root_hash(&hashes);
    let root_reversed = compute_root_hash(&reversed);

    assert_ne!(
        root_original, root_reversed,
        "reversed input must produce different root (order matters)"
    );

    // Also verify with a simple 2-element case
    let h1 = B256::from([0x01; 32]);
    let h2 = B256::from([0x02; 32]);
    let root_ab = compute_root_hash(&[h1, h2]);
    let root_ba = compute_root_hash(&[h2, h1]);
    assert_ne!(root_ab, root_ba, "hash(a||b) != hash(b||a)");
}
