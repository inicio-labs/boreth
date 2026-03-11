//! Bor receipt key computation.
//!
//! Bor receipt keys are computed differently than standard Ethereum receipt keys.
//! The key is derived from the block hash (post-Madhugiri) or block number (legacy).

use alloy_primitives::B256;

/// Compute the Bor receipt storage key from a block hash.
/// Pre-Madhugiri: key = keccak256(block_number as bytes)
/// Post-Madhugiri: key = keccak256(block_hash)
pub fn bor_receipt_key(block_hash: &B256) -> B256 {
    // Use keccak256 of the block hash
    alloy_primitives::keccak256(block_hash.as_slice())
}

/// Compute the legacy Bor receipt key from block number
pub fn bor_receipt_key_legacy(block_number: u64) -> B256 {
    alloy_primitives::keccak256(block_number.to_be_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_key_deterministic() {
        let hash = B256::from([0xab; 32]);
        let key1 = bor_receipt_key(&hash);
        let key2 = bor_receipt_key(&hash);
        assert_eq!(key1, key2, "same input must produce same output");
    }

    #[test]
    fn test_receipt_key_different_inputs() {
        let hash_a = B256::from([0x01; 32]);
        let hash_b = B256::from([0x02; 32]);
        let key_a = bor_receipt_key(&hash_a);
        let key_b = bor_receipt_key(&hash_b);
        assert_ne!(key_a, key_b, "different hashes must produce different keys");
    }

    #[test]
    fn test_legacy_key_computation() {
        let block_number: u64 = 12345;
        let key = bor_receipt_key_legacy(block_number);
        let expected = alloy_primitives::keccak256(block_number.to_be_bytes());
        assert_eq!(key, expected);

        // Different block numbers yield different keys
        let key2 = bor_receipt_key_legacy(99999);
        assert_ne!(key, key2);
    }
}
