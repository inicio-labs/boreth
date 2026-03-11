//! Bor consensus snapshot: tracks validator set and recent signers at a block.

use alloy_primitives::{Address, B256};
use bor_primitives::{Validator, ValidatorSet};
use std::collections::BTreeMap;

/// Snapshot of the Bor consensus state at a given block.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BorSnapshot {
    /// Block number this snapshot was taken at.
    pub number: u64,
    /// Block hash this snapshot was taken at.
    pub hash: B256,
    /// The current validator set.
    pub validator_set: ValidatorSet,
    /// Recent block signers: block_number -> signer address.
    pub recents: BTreeMap<u64, Address>,
}

impl BorSnapshot {
    /// Create a new snapshot at the given block.
    pub fn new(number: u64, hash: B256, validator_set: ValidatorSet) -> Self {
        Self {
            number,
            hash,
            validator_set,
            recents: BTreeMap::new(),
        }
    }

    /// Apply a new block to the snapshot: record the signer in recents.
    pub fn apply(&mut self, block_number: u64, signer: Address) {
        self.number = block_number;
        self.recents.insert(block_number, signer);

        // Prune old entries outside the window
        let validator_count = self.validator_set.validators.len();
        if validator_count > 0 {
            let window = (validator_count / 2 + 1) as u64;
            let cutoff = block_number.saturating_sub(window);
            self.recents = self.recents.split_off(&cutoff);
        }
    }

    /// Check if an address is an authorized validator/signer.
    pub fn is_authorized(&self, addr: &Address) -> bool {
        self.validator_set
            .validators
            .iter()
            .any(|v| &v.signer == addr)
    }

    /// Encode snapshot to JSON bytes for storage.
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("snapshot serialization should not fail")
    }

    /// Decode snapshot from JSON bytes.
    pub fn decode(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    fn test_validator(id: u64, addr: &str) -> Validator {
        let a = addr.parse::<Address>().unwrap();
        Validator {
            id,
            address: a,
            voting_power: 100,
            signer: a,
            proposer_priority: 0,
        }
    }

    fn test_validator_set() -> ValidatorSet {
        ValidatorSet {
            validators: vec![
                test_validator(1, "0x0000000000000000000000000000000000000001"),
                test_validator(2, "0x0000000000000000000000000000000000000002"),
                test_validator(3, "0x0000000000000000000000000000000000000003"),
            ],
            proposer: None,
        }
    }

    #[test]
    fn test_snapshot_create() {
        let vs = test_validator_set();
        let snap = BorSnapshot::new(100, B256::ZERO, vs);
        assert_eq!(snap.number, 100);
        assert!(snap.recents.is_empty());
        assert_eq!(snap.validator_set.validators.len(), 3);
    }

    #[test]
    fn test_snapshot_apply() {
        let vs = test_validator_set();
        let mut snap = BorSnapshot::new(100, B256::ZERO, vs);
        let signer = address!("0000000000000000000000000000000000000001");
        snap.apply(101, signer);
        assert_eq!(snap.number, 101);
        assert_eq!(snap.recents.get(&101), Some(&signer));
    }

    #[test]
    fn test_snapshot_is_authorized() {
        let vs = test_validator_set();
        let snap = BorSnapshot::new(100, B256::ZERO, vs);
        let valid = address!("0000000000000000000000000000000000000001");
        let invalid = address!("0000000000000000000000000000000000000099");
        assert!(snap.is_authorized(&valid));
        assert!(!snap.is_authorized(&invalid));
    }

    #[test]
    fn test_snapshot_encode_decode_roundtrip() {
        let vs = test_validator_set();
        let mut snap = BorSnapshot::new(100, B256::ZERO, vs);
        snap.apply(101, address!("0000000000000000000000000000000000000001"));

        let encoded = snap.encode();
        let decoded = BorSnapshot::decode(&encoded).unwrap();
        assert_eq!(decoded.number, snap.number);
        assert_eq!(decoded.validator_set.validators.len(), 3);
        assert_eq!(decoded.recents.len(), snap.recents.len());
    }
}
