//! Dual-path receipt storage for Bor.
//!
//! Pre-Madhugiri (block < 80,084,800): Bor receipts are stored separately and
//! NOT included in the receipt root.
//!
//! Post-Madhugiri (block >= 80,084,800): Bor receipts (state sync tx) are
//! unified with regular receipts and included in the receipt root.

use alloy_primitives::B256;
use crate::receipt_key::bor_receipt_key;

/// Madhugiri hardfork activation block on mainnet.
const MADHUGIRI_BLOCK: u64 = 80_084_800;

/// Returns `true` if the block uses unified (post-Madhugiri) receipt storage.
pub fn is_post_madhugiri(block_number: u64) -> bool {
    block_number >= MADHUGIRI_BLOCK
}

/// Compute the receipt root, optionally including the Bor receipt.
///
/// Pre-Madhugiri: receipt root is computed from only regular transaction receipts.
/// Post-Madhugiri: receipt root includes the state sync transaction receipt.
///
/// `receipt_hashes` are the RLP-encoded receipt hashes for regular transactions.
/// `bor_receipt_hash` is the optional hash of the Bor system transaction receipt.
pub fn compute_receipt_root(
    receipt_hashes: &[B256],
    bor_receipt_hash: Option<&B256>,
    block_number: u64,
) -> Vec<B256> {
    let mut all_hashes: Vec<B256> = receipt_hashes.to_vec();

    if is_post_madhugiri(block_number) {
        if let Some(bor_hash) = bor_receipt_hash {
            all_hashes.push(*bor_hash);
        }
    }

    all_hashes
}

/// Store block receipts with the correct path based on block number.
///
/// Returns the storage key for the Bor receipt (if any) and whether it should
/// be stored separately.
pub fn store_block_receipts(
    block_number: u64,
    block_hash: &B256,
) -> BorReceiptStorage {
    let key = bor_receipt_key(block_number, block_hash);
    BorReceiptStorage {
        key,
        separate: !is_post_madhugiri(block_number),
    }
}

/// Describes how a Bor receipt should be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorReceiptStorage {
    /// The raw database key for the Bor receipt.
    pub key: Vec<u8>,
    /// If `true`, the receipt is stored separately (pre-Madhugiri).
    /// If `false`, it's included in the regular receipt trie (post-Madhugiri).
    pub separate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_madhugiri_separate_storage() {
        let storage = store_block_receipts(1_000_000, &B256::ZERO);
        assert!(storage.separate, "pre-Madhugiri receipts should be stored separately");
    }

    #[test]
    fn test_post_madhugiri_standard_storage() {
        let storage = store_block_receipts(80_084_800, &B256::ZERO);
        assert!(!storage.separate, "post-Madhugiri receipts should be unified");
    }

    #[test]
    fn test_receipt_root_excludes_bor_pre_madhugiri() {
        let regular = vec![B256::from([0x01; 32]), B256::from([0x02; 32])];
        let bor = B256::from([0xff; 32]);

        let root_hashes = compute_receipt_root(&regular, Some(&bor), 1_000_000);
        // Pre-Madhugiri: bor receipt NOT included
        assert_eq!(root_hashes.len(), 2);
        assert!(!root_hashes.contains(&bor));
    }

    #[test]
    fn test_receipt_root_includes_statesynctx_post_madhugiri() {
        let regular = vec![B256::from([0x01; 32]), B256::from([0x02; 32])];
        let bor = B256::from([0xff; 32]);

        let root_hashes = compute_receipt_root(&regular, Some(&bor), 80_084_800);
        // Post-Madhugiri: bor receipt IS included
        assert_eq!(root_hashes.len(), 3);
        assert!(root_hashes.contains(&bor));
    }

    #[test]
    fn test_boundary_block_80084800() {
        // Block 80,084,800 is the first post-Madhugiri block
        assert!(is_post_madhugiri(80_084_800));
        assert!(!is_post_madhugiri(80_084_799));

        let block_hash = B256::from([0xab; 32]);
        let storage_pre = store_block_receipts(80_084_799, &block_hash);
        let storage_post = store_block_receipts(80_084_800, &block_hash);

        assert!(storage_pre.separate);
        assert!(!storage_post.separate);

        // Keys should differ because block numbers differ
        assert_ne!(storage_pre.key, storage_post.key);
    }

    #[test]
    fn test_no_bor_receipt() {
        let regular = vec![B256::from([0x01; 32])];
        let root_hashes = compute_receipt_root(&regular, None, 80_084_800);
        assert_eq!(root_hashes.len(), 1);
    }

    #[test]
    fn test_key_contains_prefix_and_data() {
        let block_number = 50_000_000u64;
        let block_hash = B256::from([0xab; 32]);
        let storage = store_block_receipts(block_number, &block_hash);

        // Key should be: "matic-bor-receipt-" + number_BE + hash_raw
        assert_eq!(storage.key.len(), 18 + 8 + 32);
        assert_eq!(&storage.key[..18], b"matic-bor-receipt-");
    }
}
