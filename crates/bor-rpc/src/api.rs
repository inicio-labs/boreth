//! Bor namespace RPC trait definition.

use crate::types::{BorSnapshotResponse, CurrentValidatorsResponse, BorReceiptResponse};
use alloy_primitives::{Address, B256};

/// Bor namespace RPC methods.
pub trait BorApi {
    /// The error type returned by RPC methods.
    type Error;

    /// Returns the snapshot at a given block number.
    fn bor_get_snapshot(&self, block_number: u64) -> Result<BorSnapshotResponse, Self::Error>;

    /// Returns the snapshot at a given block hash.
    fn bor_get_snapshot_at_hash(&self, hash: B256) -> Result<BorSnapshotResponse, Self::Error>;

    /// Returns the current validator set.
    fn bor_get_current_validators(&self) -> Result<CurrentValidatorsResponse, Self::Error>;

    /// Returns the address of the current proposer.
    fn bor_get_current_proposer(&self) -> Result<Address, Self::Error>;

    /// Returns the root hash for the given block range.
    fn bor_get_root_hash(&self, start: u64, end: u64) -> Result<B256, Self::Error>;

    /// Returns the block author (signer) by recovering it from the seal.
    /// Coinbase is always 0x0 in Bor, so this is the only way to get the producer.
    fn bor_get_author(&self, block_number: u64) -> Result<Address, Self::Error>;

    /// Returns transaction receipts for a block, merging Bor receipts appropriately.
    /// Pre-Madhugiri: includes separate Bor receipt.
    /// Post-Madhugiri: returns unified receipt list.
    fn bor_get_transaction_receipts_by_block(
        &self,
        block_number: u64,
    ) -> Result<Vec<BorReceiptResponse>, Self::Error>;
}
