//! Cross-verification of Bor receipt key encoding against Go implementation.
//!
//! The Go Bor implementation computes receipt keys as:
//! - Legacy (pre-Madhugiri): keccak256(big-endian encoded block number as 8 bytes)
//! - New (post-Madhugiri): keccak256(block_hash)
//!
//! These tests verify our Rust implementation produces identical results.

use alloy_primitives::{keccak256, B256};
use bor_storage::receipt_key::{bor_receipt_key, bor_receipt_key_legacy};

/// Go's borReceiptKey computes keccak256 of the big-endian u64 block number.
/// This mirrors that computation exactly.
fn go_bor_receipt_key_legacy(block_number: u64) -> B256 {
    keccak256(block_number.to_be_bytes())
}

/// Go's new receipt key computes keccak256 of the block hash bytes.
fn go_bor_receipt_key(block_hash: &B256) -> B256 {
    keccak256(block_hash.as_slice())
}

/// Test 10 known mainnet blocks, verifying Rust legacy key matches Go algorithm.
#[test]
fn test_legacy_key_10_mainnet_blocks() {
    let blocks: [u64; 10] = [
        0,
        1,
        100,
        1_000_000,
        10_000_000,
        38_189_056,  // Delhi fork
        50_000_000,
        77_414_656,  // Rio fork
        80_084_799,  // Last pre-Madhugiri block
        80_084_800,  // Madhugiri fork
    ];

    for block_number in blocks {
        let rust_key = bor_receipt_key_legacy(block_number);
        let go_key = go_bor_receipt_key_legacy(block_number);

        assert_eq!(
            rust_key, go_key,
            "Legacy key mismatch at block {block_number}: rust={rust_key}, go={go_key}"
        );

        // Verify non-zero
        assert_ne!(rust_key, B256::ZERO, "Key at block {block_number} should not be zero");
    }
}

/// Verify hash-based key (post-Madhugiri) matches Go computation for various hashes.
#[test]
fn test_hash_key_matches_go() {
    let test_hashes = [
        B256::ZERO,
        B256::from([0xff; 32]),
        B256::from([0xab; 32]),
        keccak256(b"polygon_bor_block_80084800"),
        keccak256(b"polygon_bor_block_80084801"),
        keccak256(b"polygon_bor_block_99999999"),
        // Simulate typical block hashes
        keccak256([1u8; 32]),
        keccak256([2u8; 32]),
        keccak256(100u64.to_be_bytes()),
        keccak256(80_084_800u64.to_be_bytes()),
    ];

    for hash in &test_hashes {
        let rust_key = bor_receipt_key(hash);
        let go_key = go_bor_receipt_key(hash);
        assert_eq!(
            rust_key, go_key,
            "Hash key mismatch for hash {hash}"
        );
    }
}

/// Verify the specific encoding: Go uses big-endian u64 bytes, not variable-length.
#[test]
fn test_encoding_is_fixed_8_byte_big_endian() {
    // Block 0 should hash [0,0,0,0,0,0,0,0] (8 zero bytes)
    let key_0 = bor_receipt_key_legacy(0);
    let expected = keccak256([0u8; 8]);
    assert_eq!(key_0, expected, "Block 0 key should hash 8 zero bytes");

    // Block 1 should hash [0,0,0,0,0,0,0,1]
    let key_1 = bor_receipt_key_legacy(1);
    let expected = keccak256([0, 0, 0, 0, 0, 0, 0, 1u8]);
    assert_eq!(key_1, expected, "Block 1 key should hash [0,0,0,0,0,0,0,1]");

    // Block 256 should hash [0,0,0,0,0,0,1,0]
    let key_256 = bor_receipt_key_legacy(256);
    let expected = keccak256([0, 0, 0, 0, 0, 0, 1, 0u8]);
    assert_eq!(key_256, expected, "Block 256 key encoding");
}

/// Legacy and hash keys should produce different results for related inputs.
#[test]
fn test_legacy_vs_hash_key_different() {
    let block_number = 80_084_800u64;
    let block_hash = keccak256(block_number.to_be_bytes());

    let legacy_key = bor_receipt_key_legacy(block_number);
    let hash_key = bor_receipt_key(&block_hash);

    // legacy = keccak256(block_number_bytes)
    // hash = keccak256(keccak256(block_number_bytes))
    assert_ne!(legacy_key, hash_key, "Legacy and hash keys must differ");
}

/// All unique block numbers should produce unique keys.
#[test]
fn test_no_key_collisions() {
    let blocks: Vec<u64> = (0..100).collect();
    let keys: Vec<B256> = blocks.iter().map(|&b| bor_receipt_key_legacy(b)).collect();

    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            assert_ne!(
                keys[i], keys[j],
                "Collision between block {} and {}",
                blocks[i], blocks[j]
            );
        }
    }
}

/// Verify keys are deterministic.
#[test]
fn test_deterministic_keys() {
    for block in [0u64, 1, 1000, 80_084_800, u64::MAX] {
        let key1 = bor_receipt_key_legacy(block);
        let key2 = bor_receipt_key_legacy(block);
        assert_eq!(key1, key2, "Legacy key should be deterministic for block {block}");
    }

    let hash = B256::from([0xab; 32]);
    assert_eq!(bor_receipt_key(&hash), bor_receipt_key(&hash));
}

/// Verify keys at Bor mainnet hardfork boundaries are all distinct.
#[test]
fn test_hardfork_boundary_keys() {
    let delhi = 38_189_056u64;
    let rio = 77_414_656u64;
    let madhugiri = 80_084_800u64;
    let lisovo = 83_756_500u64;

    let keys: Vec<B256> = [delhi, rio, madhugiri, lisovo]
        .iter()
        .map(|&b| bor_receipt_key_legacy(b))
        .collect();

    // All distinct
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            assert_ne!(keys[i], keys[j]);
        }
    }
}
