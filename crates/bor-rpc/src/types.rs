//! RPC response types for the `bor_*` namespace.

use alloy_primitives::{Address, B256};
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
