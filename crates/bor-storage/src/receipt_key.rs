//! Bor receipt key computation.
//!
//! Go Bor uses two related functions:
//!
//! 1. `BorReceiptKey(number, hash)` → raw DB key:
//!    `"matic-bor-receipt-" + block_number_BE_u64 + block_hash_raw_32`
//!    This is the key used to store/retrieve Bor receipts in the database.
//!
//! 2. `GetDerivedBorTxHash(receiptKey)` → synthetic tx hash:
//!    `keccak256(BorReceiptKey(number, hash))`
//!    This derives a deterministic tx hash for the Bor state sync transaction.

use alloy_primitives::B256;

/// The prefix used for all Bor receipt keys, matching Go Bor's `borReceiptPrefix`.
const BOR_RECEIPT_PREFIX: &[u8] = b"matic-bor-receipt-";

/// Compute the Bor receipt DB key from a block number and block hash.
///
/// Format: `"matic-bor-receipt-" + block_number_BE_u64 + block_hash_raw_32`
///
/// This matches Go Bor's `BorReceiptKey` function exactly.
/// The result is the raw database key (NOT hashed).
pub fn bor_receipt_key(block_number: u64, block_hash: &B256) -> Vec<u8> {
    let mut data = Vec::with_capacity(BOR_RECEIPT_PREFIX.len() + 8 + 32);
    data.extend_from_slice(BOR_RECEIPT_PREFIX);
    data.extend_from_slice(&block_number.to_be_bytes());
    data.extend_from_slice(block_hash.as_slice());
    data
}

/// Derive the synthetic Bor transaction hash from a receipt key.
///
/// Format: `keccak256("matic-bor-receipt-" + block_number_BE_u64 + block_hash_raw_32)`
///
/// This matches Go Bor's `GetDerivedBorTxHash` function.
pub fn derived_bor_tx_hash(block_number: u64, block_hash: &B256) -> B256 {
    let key = bor_receipt_key(block_number, block_hash);
    alloy_primitives::keccak256(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_key_format() {
        let hash = B256::from([0xab; 32]);
        let key = bor_receipt_key(100, &hash);

        // Key should be: prefix (18 bytes) + number (8 bytes) + hash (32 bytes) = 58 bytes
        assert_eq!(key.len(), 18 + 8 + 32);
        assert_eq!(&key[..18], BOR_RECEIPT_PREFIX);
        assert_eq!(&key[18..26], &100u64.to_be_bytes());
        assert_eq!(&key[26..], hash.as_slice());
    }

    #[test]
    fn test_receipt_key_deterministic() {
        let hash = B256::from([0xab; 32]);
        let key1 = bor_receipt_key(100, &hash);
        let key2 = bor_receipt_key(100, &hash);
        assert_eq!(key1, key2, "same input must produce same output");
    }

    #[test]
    fn test_receipt_key_different_hashes() {
        let hash_a = B256::from([0x01; 32]);
        let hash_b = B256::from([0x02; 32]);
        let key_a = bor_receipt_key(100, &hash_a);
        let key_b = bor_receipt_key(100, &hash_b);
        assert_ne!(key_a, key_b, "different hashes must produce different keys");
    }

    #[test]
    fn test_receipt_key_different_block_numbers() {
        let hash = B256::from([0xab; 32]);
        let key1 = bor_receipt_key(100, &hash);
        let key2 = bor_receipt_key(200, &hash);
        assert_ne!(key1, key2, "different block numbers must produce different keys");
    }

    #[test]
    fn test_derived_tx_hash_is_keccak_of_key() {
        let hash = B256::from([0xab; 32]);
        let key = bor_receipt_key(100, &hash);
        let tx_hash = derived_bor_tx_hash(100, &hash);
        assert_eq!(tx_hash, alloy_primitives::keccak256(&key));
    }

    #[test]
    fn test_block_number_is_big_endian_8_byte() {
        let hash = B256::ZERO;

        // Block 0: number bytes = [0,0,0,0,0,0,0,0]
        let key_0 = bor_receipt_key(0, &hash);
        assert_eq!(&key_0[18..26], &[0, 0, 0, 0, 0, 0, 0, 0]);

        // Block 1: number bytes = [0,0,0,0,0,0,0,1]
        let key_1 = bor_receipt_key(1, &hash);
        assert_eq!(&key_1[18..26], &[0, 0, 0, 0, 0, 0, 0, 1]);

        // Block 256: number bytes = [0,0,0,0,0,0,1,0]
        let key_256 = bor_receipt_key(256, &hash);
        assert_eq!(&key_256[18..26], &[0, 0, 0, 0, 0, 0, 1, 0]);
    }

    #[test]
    fn test_derived_tx_hash_deterministic() {
        let hash = B256::from([0xab; 32]);
        let h1 = derived_bor_tx_hash(100, &hash);
        let h2 = derived_bor_tx_hash(100, &hash);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_derived_tx_hash_different_inputs() {
        let hash = B256::from([0xab; 32]);
        let h1 = derived_bor_tx_hash(100, &hash);
        let h2 = derived_bor_tx_hash(101, &hash);
        assert_ne!(h1, h2);
    }
}
