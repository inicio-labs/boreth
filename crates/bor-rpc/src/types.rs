//! RPC response types for the `bor_*` namespace.

use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

/// Response type for `bor_getSnapshot` and `bor_getSnapshotAtHash`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorSnapshotResponse {
    /// The block number at which the snapshot was taken.
    pub number: u64,
    /// The block hash at which the snapshot was taken.
    pub hash: B256,
    /// The validator set at this snapshot.
    pub validator_set: Vec<ValidatorInfo>,
}

/// Validator information returned in RPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorInfo {
    /// The validator's address.
    pub address: Address,
    /// The validator's voting power.
    pub voting_power: i64,
    /// The validator's proposer priority.
    pub proposer_priority: i64,
}

/// Response type for `bor_getCurrentValidators`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentValidatorsResponse {
    /// The current set of validators.
    pub validators: Vec<ValidatorInfo>,
}

/// Response type for `bor_getTransactionReceiptsByBlock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorReceiptResponse {
    /// Transaction hash.
    pub tx_hash: B256,
    /// Block number.
    pub block_number: u64,
    /// Block hash.
    pub block_hash: B256,
    /// Cumulative gas used.
    pub cumulative_gas_used: U256,
    /// Gas used by this transaction.
    pub gas_used: U256,
    /// Whether this is a Bor system transaction.
    pub is_bor_tx: bool,
    /// Status (1 = success, 0 = failure).
    pub status: u64,
}
