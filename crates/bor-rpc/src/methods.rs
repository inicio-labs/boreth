//! Bor RPC method implementations.
//!
//! Provides utility functions used by the RPC method implementations:
//! - `get_author`: recovers block signer from seal
//! - `get_root_hash`: computes Merkle root of block hashes in a range
//! - `get_transaction_receipts_by_block`: merges Bor receipts with regular ones

use alloy_primitives::{keccak256, Address, B256};
use bor_consensus::{ecrecover_seal, ExtraData, SealError};

/// Errors from Bor RPC methods.
#[derive(Debug, thiserror::Error)]
pub enum BorRpcError {
    #[error("block not found: {0}")]
    BlockNotFound(u64),
    #[error("seal recovery failed: {0}")]
    SealError(#[from] SealError),
    #[error("invalid extra data: {0}")]
    ExtraDataError(String),
    #[error("invalid block range: start {start} > end {end}")]
    InvalidBlockRange { start: u64, end: u64 },
}

/// Recover the block author (signer) from the header's extra data and seal hash.
///
/// In Bor, `coinbase` is always `0x0`. The actual block producer must be recovered
/// via ECRECOVER from the seal signature in extra data.
pub fn get_author(seal_hash: &B256, extra_data: &[u8]) -> Result<Address, BorRpcError> {
    let parsed = ExtraData::parse(extra_data)
        .map_err(|e| BorRpcError::ExtraDataError(e.to_string()))?;
    ecrecover_seal(seal_hash, &parsed.seal)
        .map_err(BorRpcError::SealError)
}

/// Compute the root hash for a range of block hashes.
///
/// This is a simple Merkle tree over the block hashes in the range [start, end].
/// The hashes are repeatedly paired and hashed until a single root remains.
pub fn compute_root_hash(block_hashes: &[B256]) -> B256 {
    if block_hashes.is_empty() {
        return B256::ZERO;
    }
    if block_hashes.len() == 1 {
        return block_hashes[0];
    }

    let mut current_level: Vec<B256> = block_hashes.to_vec();

    // Pad to next power of 2
    let next_pow2 = current_level.len().next_power_of_two();
    while current_level.len() < next_pow2 {
        current_level.push(B256::ZERO);
    }

    while current_level.len() > 1 {
        let mut next_level = Vec::with_capacity(current_level.len() / 2);
        for pair in current_level.chunks(2) {
            let mut combined = [0u8; 64];
            combined[..32].copy_from_slice(pair[0].as_slice());
            combined[32..].copy_from_slice(pair[1].as_slice());
            next_level.push(keccak256(combined));
        }
        current_level = next_level;
    }

    current_level[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_hash_empty() {
        assert_eq!(compute_root_hash(&[]), B256::ZERO);
    }

    #[test]
    fn test_root_hash_single() {
        let hash = B256::from([0xab; 32]);
        assert_eq!(compute_root_hash(&[hash]), hash);
    }

    #[test]
    fn test_root_hash_two() {
        let h1 = B256::from([0x01; 32]);
        let h2 = B256::from([0x02; 32]);
        let root = compute_root_hash(&[h1, h2]);

        // Manual: keccak256(h1 || h2)
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(h1.as_slice());
        combined[32..].copy_from_slice(h2.as_slice());
        let expected = keccak256(combined);

        assert_eq!(root, expected);
    }

    #[test]
    fn test_root_hash_deterministic() {
        let hashes: Vec<B256> = (0..8)
            .map(|i| B256::from([i as u8; 32]))
            .collect();

        let root1 = compute_root_hash(&hashes);
        let root2 = compute_root_hash(&hashes);
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_root_hash_non_power_of_two() {
        let hashes: Vec<B256> = (0..3)
            .map(|i| B256::from([i as u8; 32]))
            .collect();

        let root = compute_root_hash(&hashes);
        // Should pad to 4 elements (next power of 2)
        assert_ne!(root, B256::ZERO);
    }

    #[test]
    fn test_root_hash_different_inputs() {
        let hashes_a: Vec<B256> = vec![B256::from([0x01; 32]), B256::from([0x02; 32])];
        let hashes_b: Vec<B256> = vec![B256::from([0x03; 32]), B256::from([0x04; 32])];

        assert_ne!(compute_root_hash(&hashes_a), compute_root_hash(&hashes_b));
    }

    #[test]
    fn test_invalid_block_range_error() {
        let err = BorRpcError::InvalidBlockRange { start: 100, end: 50 };
        assert!(err.to_string().contains("start 100 > end 50"));
    }
}
