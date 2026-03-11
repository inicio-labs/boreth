//! Span and snapshot DB persistence traits and in-memory implementations.

use bor_primitives::Span;
use std::collections::HashMap;

/// Trait for persisting Bor spans.
pub trait SpanStore: Send + Sync {
    /// Retrieve a span by its ID.
    fn get_span(&self, span_id: u64) -> Option<Span>;
    /// Store a span, keyed by its `id` field.
    fn put_span(&mut self, span: Span);
    /// Return the highest span ID currently stored, if any.
    fn latest_span_id(&self) -> Option<u64>;
}

/// Trait for persisting Bor snapshots.
pub trait SnapshotStore: Send + Sync {
    /// Retrieve snapshot data by block hash.
    fn get_snapshot(&self, block_hash: &[u8; 32]) -> Option<Vec<u8>>;
    /// Store snapshot data keyed by block hash.
    fn put_snapshot(&mut self, block_hash: [u8; 32], data: Vec<u8>);
}

/// In-memory [`SpanStore`] implementation for testing.
#[derive(Debug, Default)]
pub struct InMemorySpanStore {
    spans: HashMap<u64, Span>,
}

impl InMemorySpanStore {
    /// Create a new, empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SpanStore for InMemorySpanStore {
    fn get_span(&self, span_id: u64) -> Option<Span> {
        self.spans.get(&span_id).cloned()
    }

    fn put_span(&mut self, span: Span) {
        self.spans.insert(span.id, span);
    }

    fn latest_span_id(&self) -> Option<u64> {
        self.spans.keys().max().copied()
    }
}

/// In-memory [`SnapshotStore`] implementation for testing.
#[derive(Debug, Default)]
pub struct InMemorySnapshotStore {
    snapshots: HashMap<[u8; 32], Vec<u8>>,
}

impl InMemorySnapshotStore {
    /// Create a new, empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn get_snapshot(&self, block_hash: &[u8; 32]) -> Option<Vec<u8>> {
        self.snapshots.get(block_hash).cloned()
    }

    fn put_snapshot(&mut self, block_hash: [u8; 32], data: Vec<u8>) {
        self.snapshots.insert(block_hash, data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bor_primitives::{Validator, ValidatorSet};

    fn sample_span(id: u64) -> Span {
        let validator = Validator {
            id: 1,
            address: alloy_primitives::Address::new([0xaa; 20]),
            voting_power: 100,
            signer: alloy_primitives::Address::new([0xaa; 20]),
            proposer_priority: 0,
        };
        Span {
            id,
            start_block: id * 6400,
            end_block: (id + 1) * 6400 - 1,
            validator_set: ValidatorSet {
                validators: vec![validator.clone()],
                proposer: Some(validator.clone()),
            },
            selected_producers: vec![validator],
            bor_chain_id: "137".to_string(),
        }
    }

    #[test]
    fn span_store_put_get_roundtrip() {
        let mut store = InMemorySpanStore::new();
        assert!(store.get_span(1).is_none());
        assert!(store.latest_span_id().is_none());

        let span = sample_span(1);
        store.put_span(span.clone());

        let retrieved = store.get_span(1).expect("span should exist");
        assert_eq!(retrieved.id, 1);
        assert_eq!(retrieved.start_block, 6400);
        assert_eq!(retrieved.end_block, 12799);
        assert_eq!(store.latest_span_id(), Some(1));
    }

    #[test]
    fn span_store_latest_tracks_max() {
        let mut store = InMemorySpanStore::new();
        store.put_span(sample_span(3));
        store.put_span(sample_span(1));
        store.put_span(sample_span(5));
        assert_eq!(store.latest_span_id(), Some(5));
    }

    #[test]
    fn snapshot_store_put_get_roundtrip() {
        let mut store = InMemorySnapshotStore::new();
        let hash = [0xffu8; 32];
        let data = vec![1, 2, 3, 4, 5];

        assert!(store.get_snapshot(&hash).is_none());

        store.put_snapshot(hash, data.clone());

        let retrieved = store.get_snapshot(&hash).expect("snapshot should exist");
        assert_eq!(retrieved, data);
    }

    #[test]
    fn snapshot_store_overwrite() {
        let mut store = InMemorySnapshotStore::new();
        let hash = [0x01u8; 32];

        store.put_snapshot(hash, vec![1, 2, 3]);
        store.put_snapshot(hash, vec![4, 5, 6]);

        let retrieved = store.get_snapshot(&hash).expect("snapshot should exist");
        assert_eq!(retrieved, vec![4, 5, 6]);
    }
}
