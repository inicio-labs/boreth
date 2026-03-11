//! Cross-verification of Bor receipt key encoding against Go implementation.
//!
//! The Go Bor implementation:
//!   BorReceiptKey(number, hash) = "matic-bor-receipt-" + number_BE_u64 + hash_raw_32
//!   GetDerivedBorTxHash(key) = keccak256(key)
//!
//! These tests verify our Rust implementation produces identical results.

use alloy_primitives::{keccak256, B256};
use bor_storage::receipt_key::{bor_receipt_key, derived_bor_tx_hash};

/// Prefix used by Go Bor for receipt keys.
const BOR_RECEIPT_PREFIX: &[u8] = b"matic-bor-receipt-";

/// Mirror of Go's BorReceiptKey: returns raw bytes (prefix + number_be_u64 + hash_raw_32)
fn go_bor_receipt_key(block_number: u64, block_hash: &B256) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(BOR_RECEIPT_PREFIX);
    data.extend_from_slice(&block_number.to_be_bytes());
    data.extend_from_slice(block_hash.as_slice());
    data
}

/// Mirror of Go's GetDerivedBorTxHash: keccak256(BorReceiptKey)
fn go_derived_bor_tx_hash(block_number: u64, block_hash: &B256) -> B256 {
    let key = go_bor_receipt_key(block_number, block_hash);
    keccak256(key)
}

/// Test known mainnet blocks, verifying Rust DB key matches Go algorithm.
#[test]
fn test_receipt_key_matches_go_for_mainnet_blocks() {
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
        let block_hash = keccak256(block_number.to_be_bytes());

        // DB key comparison
        let rust_key = bor_receipt_key(block_number, &block_hash);
        let go_key = go_bor_receipt_key(block_number, &block_hash);
        assert_eq!(
            rust_key, go_key,
            "DB key mismatch at block {block_number}"
        );

        // Derived tx hash comparison
        let rust_hash = derived_bor_tx_hash(block_number, &block_hash);
        let go_hash = go_derived_bor_tx_hash(block_number, &block_hash);
        assert_eq!(
            rust_hash, go_hash,
            "Derived tx hash mismatch at block {block_number}"
        );

        assert_ne!(rust_hash, B256::ZERO, "Tx hash at block {block_number} should not be zero");
    }
}

/// Verify the specific encoding: Go uses big-endian u64 bytes.
#[test]
fn test_encoding_is_fixed_8_byte_big_endian() {
    let hash = B256::ZERO;

    // Block 0: number bytes = [0,0,0,0,0,0,0,0]
    let key_0 = bor_receipt_key(0, &hash);
    assert_eq!(key_0.len(), 18 + 8 + 32);
    assert_eq!(&key_0[..18], BOR_RECEIPT_PREFIX);
    assert_eq!(&key_0[18..26], &[0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&key_0[26..], hash.as_slice());

    // Block 1
    let key_1 = bor_receipt_key(1, &hash);
    assert_eq!(&key_1[18..26], &[0, 0, 0, 0, 0, 0, 0, 1]);

    // Block 256
    let key_256 = bor_receipt_key(256, &hash);
    assert_eq!(&key_256[18..26], &[0, 0, 0, 0, 0, 0, 1, 0]);

    // Block 2^32
    let key_2_32 = bor_receipt_key(1u64 << 32, &hash);
    assert_eq!(&key_2_32[18..26], &[0, 0, 0, 1, 0, 0, 0, 0]);

    // Block 2^32-1
    let key_2_32m1 = bor_receipt_key((1u64 << 32) - 1, &hash);
    assert_eq!(&key_2_32m1[18..26], &[0, 0, 0, 0, 255, 255, 255, 255]);

    // Block u64::MAX
    let key_max = bor_receipt_key(u64::MAX, &hash);
    assert_eq!(&key_max[18..26], &[255, 255, 255, 255, 255, 255, 255, 255]);
}

/// Verify that little-endian encoding would produce a DIFFERENT key.
#[test]
fn test_big_endian_vs_little_endian_differ() {
    let block_number = 256u64;
    let hash = B256::from([0xab; 32]);

    let be_key = bor_receipt_key(block_number, &hash);

    // Wrong: LE encoding
    let mut le_data = Vec::new();
    le_data.extend_from_slice(BOR_RECEIPT_PREFIX);
    le_data.extend_from_slice(&block_number.to_le_bytes());
    le_data.extend_from_slice(hash.as_slice());

    assert_ne!(be_key, le_data, "BE and LE encodings must produce different keys");
}

/// Verify block hash is raw 32 bytes, NOT hex-encoded.
#[test]
fn test_block_hash_is_raw_bytes_not_hex() {
    let hash = B256::from([0xab; 32]);
    let block_number = 100u64;

    let correct_key = bor_receipt_key(block_number, &hash);

    // Wrong: hex-encoded string
    let hex_str = format!("{hash:x}");
    let mut wrong_data = Vec::new();
    wrong_data.extend_from_slice(BOR_RECEIPT_PREFIX);
    wrong_data.extend_from_slice(&block_number.to_be_bytes());
    wrong_data.extend_from_slice(hex_str.as_bytes());

    assert_ne!(correct_key, wrong_data, "Raw bytes vs hex encoding must differ");
}

/// All unique block numbers should produce unique DB keys (same hash).
#[test]
fn test_no_key_collisions() {
    let hash = B256::from([0xaa; 32]);
    let blocks: Vec<u64> = (0..100).collect();
    let keys: Vec<Vec<u8>> = blocks.iter().map(|&b| bor_receipt_key(b, &hash)).collect();

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

/// Same block number with different hashes → different keys.
#[test]
fn test_different_hashes_different_keys() {
    let block = 1_000_000u64;
    let hash_a = B256::from([0x01; 32]);
    let hash_b = B256::from([0x02; 32]);
    assert_ne!(
        bor_receipt_key(block, &hash_a),
        bor_receipt_key(block, &hash_b),
    );
}

/// Verify keys are deterministic.
#[test]
fn test_deterministic_keys() {
    let hash = B256::from([0xab; 32]);
    for block in [0u64, 1, 1000, 80_084_800, u64::MAX] {
        let key1 = bor_receipt_key(block, &hash);
        let key2 = bor_receipt_key(block, &hash);
        assert_eq!(key1, key2, "Key should be deterministic for block {block}");
    }
}

/// Verify derived tx hashes at hardfork boundaries are all distinct.
#[test]
fn test_hardfork_boundary_derived_hashes() {
    let delhi = 38_189_056u64;
    let rio = 77_414_656u64;
    let madhugiri = 80_084_800u64;
    let lisovo = 83_756_500u64;

    let hash = B256::from([0xcc; 32]);
    let tx_hashes: Vec<B256> = [delhi, rio, madhugiri, lisovo]
        .iter()
        .map(|&b| derived_bor_tx_hash(b, &hash))
        .collect();

    for i in 0..tx_hashes.len() {
        for j in (i + 1)..tx_hashes.len() {
            assert_ne!(tx_hashes[i], tx_hashes[j]);
        }
    }
}

/// Test edge cases: large block numbers.
#[test]
fn test_large_block_numbers() {
    let hash = B256::from([0xdd; 32]);
    let key_2_32 = bor_receipt_key(1u64 << 32, &hash);
    let key_2_32_minus_1 = bor_receipt_key((1u64 << 32) - 1, &hash);
    let key_max = bor_receipt_key(u64::MAX, &hash);

    assert_ne!(key_2_32, key_2_32_minus_1);
    assert_ne!(key_2_32, key_max);
    assert_ne!(key_2_32_minus_1, key_max);

    // Verify against Go computation
    assert_eq!(key_2_32, go_bor_receipt_key(1u64 << 32, &hash));
    assert_eq!(key_max, go_bor_receipt_key(u64::MAX, &hash));
}

/// Test that the key without prefix would be wrong.
#[test]
fn test_prefix_is_required() {
    let block = 100u64;
    let hash = B256::from([0xee; 32]);

    let correct = bor_receipt_key(block, &hash);

    // Without prefix
    let mut no_prefix_data = Vec::new();
    no_prefix_data.extend_from_slice(&block.to_be_bytes());
    no_prefix_data.extend_from_slice(hash.as_slice());

    assert_ne!(correct, no_prefix_data, "Missing prefix must produce different key");
}

/// Verify string encoding "80084800" produces DIFFERENT key than binary encoding.
#[test]
fn test_string_encoding_differs_from_binary() {
    let block_number = 80_084_800u64;
    let hash = B256::from([0xab; 32]);

    let correct = bor_receipt_key(block_number, &hash);

    // Wrong: using string representation of block number
    let mut string_data = Vec::new();
    string_data.extend_from_slice(BOR_RECEIPT_PREFIX);
    string_data.extend_from_slice(format!("{block_number}").as_bytes());
    string_data.extend_from_slice(hash.as_slice());

    assert_ne!(correct, string_data, "String encoding must differ from binary");
}
