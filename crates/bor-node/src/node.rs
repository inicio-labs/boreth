//! BorNode: wires all components together.

use alloy_primitives::{Address, B256};
use bor_chainspec::BorChainSpec;
use bor_consensus::BorSnapshot;
use bor_storage::persistence::{InMemorySpanStore, InMemorySnapshotStore, SpanStore, SnapshotStore};
use crate::config::{BorNodeConfig, BorNetwork};
use std::sync::{Arc, RwLock};

/// The assembled Bor node with all components wired together.
pub struct BorNode {
    /// Node configuration.
    pub config: BorNodeConfig,
    /// Chain specification.
    pub chain_spec: Arc<BorChainSpec>,
    /// Span store.
    pub span_store: Arc<RwLock<InMemorySpanStore>>,
    /// Snapshot store.
    pub snapshot_store: Arc<RwLock<InMemorySnapshotStore>>,
}

impl BorNode {
    /// Create a new BorNode from configuration.
    pub fn new(config: BorNodeConfig) -> eyre::Result<Self> {
        let chain_spec = match config.network {
            BorNetwork::Mainnet => Arc::new(bor_chainspec::bor_mainnet_genesis()),
            BorNetwork::Amoy => Arc::new(bor_chainspec::bor_amoy_genesis()),
        };

        let span_store = Arc::new(RwLock::new(InMemorySpanStore::new()));
        let snapshot_store = Arc::new(RwLock::new(InMemorySnapshotStore::new()));

        Ok(Self {
            config,
            chain_spec,
            span_store,
            snapshot_store,
        })
    }

    /// Get the chain ID.
    pub fn chain_id(&self) -> u64 {
        self.config.chain_id()
    }

    /// Store a snapshot.
    pub fn put_snapshot(&self, block_hash: B256, snapshot: &BorSnapshot) -> eyre::Result<()> {
        let data = snapshot.encode();
        let mut store = self.snapshot_store.write().map_err(|e| eyre::eyre!("{e}"))?;
        store.put_snapshot(block_hash.0, data);
        Ok(())
    }

    /// Get a snapshot by block hash.
    pub fn get_snapshot(&self, block_hash: &B256) -> eyre::Result<Option<BorSnapshot>> {
        let store = self.snapshot_store.read().map_err(|e| eyre::eyre!("{e}"))?;
        match store.get_snapshot(&block_hash.0) {
            Some(data) => {
                let snap = BorSnapshot::decode(&data)?;
                Ok(Some(snap))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bor_primitives::{Validator, ValidatorSet};

    fn test_validator_set() -> ValidatorSet {
        ValidatorSet {
            validators: vec![Validator {
                id: 1,
                address: Address::new([0xaa; 20]),
                voting_power: 100,
                signer: Address::new([0xaa; 20]),
                proposer_priority: 0,
            }],
            proposer: None,
        }
    }

    #[test]
    fn test_node_builder_compiles() {
        let config = BorNodeConfig::amoy();
        let node = BorNode::new(config).unwrap();
        assert_eq!(node.chain_id(), 80002);
    }

    #[test]
    fn test_node_mainnet() {
        let config = BorNodeConfig::mainnet();
        let node = BorNode::new(config).unwrap();
        assert_eq!(node.chain_id(), 137);
    }

    #[test]
    fn test_snapshot_roundtrip() {
        let config = BorNodeConfig::amoy();
        let node = BorNode::new(config).unwrap();

        let hash = B256::from([0xab; 32]);
        let snapshot = BorSnapshot::new(100, hash, test_validator_set());

        node.put_snapshot(hash, &snapshot).unwrap();

        let retrieved = node.get_snapshot(&hash).unwrap().unwrap();
        assert_eq!(retrieved.number, 100);
        assert_eq!(retrieved.validator_set.validators.len(), 1);
    }

    #[test]
    fn test_snapshot_not_found() {
        let config = BorNodeConfig::amoy();
        let node = BorNode::new(config).unwrap();
        let result = node.get_snapshot(&B256::ZERO).unwrap();
        assert!(result.is_none());
    }
}
