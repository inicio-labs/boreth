//! LRU-like span cache backed by a `HashMap` and a `Vec` for access-order tracking.

use bor_primitives::Span;
use std::collections::HashMap;

/// A simple LRU span cache.
///
/// Stores spans keyed by their ID and evicts the least-recently-used entry
/// when the cache exceeds `max_size`.
pub struct SpanCache {
    spans: HashMap<u64, Span>,
    max_size: usize,
    /// Tracks access order — the *back* of the vec is the most-recently-used.
    access_order: Vec<u64>,
}

impl SpanCache {
    /// Creates a new `SpanCache` with the given maximum capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            spans: HashMap::with_capacity(max_size),
            max_size,
            access_order: Vec::with_capacity(max_size),
        }
    }

    /// Returns a reference to the span with the given ID, promoting it to
    /// most-recently-used. Returns `None` if the span is not cached.
    pub fn get(&mut self, span_id: u64) -> Option<&Span> {
        if self.spans.contains_key(&span_id) {
            self.touch(span_id);
            self.spans.get(&span_id)
        } else {
            None
        }
    }

    /// Inserts a span into the cache. If the cache is full, the
    /// least-recently-used entry is evicted first.
    pub fn insert(&mut self, span: Span) {
        let span_id = span.id;

        // If already present, update in place and promote.
        if self.spans.contains_key(&span_id) {
            self.spans.insert(span_id, span);
            self.touch(span_id);
            return;
        }

        // Evict if at capacity.
        if self.spans.len() >= self.max_size && self.max_size > 0 {
            if let Some(lru_id) = self.access_order.first().copied() {
                self.access_order.remove(0);
                self.spans.remove(&lru_id);
            }
        }

        self.spans.insert(span_id, span);
        self.access_order.push(span_id);
    }

    /// Returns `true` if the cache contains a span with the given ID.
    pub fn contains(&self, span_id: u64) -> bool {
        self.spans.contains_key(&span_id)
    }

    /// Returns the number of spans currently in the cache.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Promote `span_id` to most-recently-used by moving it to the back of
    /// the access-order vector.
    fn touch(&mut self, span_id: u64) {
        if let Some(pos) = self.access_order.iter().position(|&id| id == span_id) {
            self.access_order.remove(pos);
        }
        self.access_order.push(span_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use bor_primitives::{Validator, ValidatorSet};

    fn make_span(id: u64) -> Span {
        Span {
            id,
            start_block: id * 6400,
            end_block: (id + 1) * 6400 - 1,
            validator_set: ValidatorSet {
                validators: vec![Validator {
                    id: 1,
                    address: Address::ZERO,
                    voting_power: 100,
                    signer: Address::ZERO,
                    proposer_priority: 0,
                }],
                proposer: None,
            },
            selected_producers: vec![],
            bor_chain_id: "137".to_string(),
        }
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache = SpanCache::new(4);
        let span = make_span(1);
        cache.insert(span);

        assert!(cache.contains(1));
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get(1).unwrap();
        assert_eq!(retrieved.id, 1);
        assert_eq!(retrieved.start_block, 6400);
    }

    #[test]
    fn test_get_missing_returns_none() {
        let mut cache = SpanCache::new(4);
        assert!(cache.get(42).is_none());
    }

    #[test]
    fn test_eviction_when_over_max_size() {
        let mut cache = SpanCache::new(2);

        cache.insert(make_span(1));
        cache.insert(make_span(2));
        assert_eq!(cache.len(), 2);

        // Inserting a third should evict span 1 (least recently used).
        cache.insert(make_span(3));
        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(1));
        assert!(cache.contains(2));
        assert!(cache.contains(3));
    }

    #[test]
    fn test_access_promotes_entry() {
        let mut cache = SpanCache::new(2);

        cache.insert(make_span(1));
        cache.insert(make_span(2));

        // Access span 1 so span 2 becomes the LRU.
        cache.get(1);

        // Inserting span 3 should now evict span 2, not span 1.
        cache.insert(make_span(3));
        assert!(cache.contains(1));
        assert!(!cache.contains(2));
        assert!(cache.contains(3));
    }

    #[test]
    fn test_insert_duplicate_updates_in_place() {
        let mut cache = SpanCache::new(4);
        cache.insert(make_span(1));
        cache.insert(make_span(1));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_is_empty() {
        let cache = SpanCache::new(4);
        assert!(cache.is_empty());
    }
}
