//! Comprehensive integration tests for the heimdall-client crate.

use alloy_primitives::{Address, Bytes, B256};
use bor_primitives::{Span, Validator, ValidatorSet};
use heimdall_client::{
    Checkpoint, HeimdallClient, HeimdallError, Milestone, MockHeimdallClient, SpanCache,
    StateSyncEvent,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn make_event(id: u64) -> StateSyncEvent {
    StateSyncEvent {
        id,
        contract: Address::new([0xbb; 20]),
        data: Bytes::from(vec![0x01, 0x02, 0x03]),
        tx_hash: B256::ZERO,
        log_index: id,
        bor_chain_id: "137".to_string(),
        time: 1_000_000 + id,
    }
}

// ---------------------------------------------------------------------------
// SpanCache LRU tests
// ---------------------------------------------------------------------------

/// 1. Cache with max_size=1 evicts immediately on second insert.
#[test]
fn cache_size_one_evicts_on_second_insert() {
    let mut cache = SpanCache::new(1);
    cache.insert(make_span(0));
    assert_eq!(cache.len(), 1);
    assert!(cache.contains(0));

    cache.insert(make_span(1));
    assert_eq!(cache.len(), 1);
    assert!(!cache.contains(0), "span 0 should have been evicted");
    assert!(cache.contains(1));
}

/// 2. Cache with max_size=0 handles gracefully (items are inserted but never evicted
///    because the eviction guard requires max_size > 0).
#[test]
fn cache_size_zero_edge_case() {
    let mut cache = SpanCache::new(0);
    assert!(cache.is_empty());

    // Inserts still succeed because the eviction branch is skipped when max_size == 0.
    cache.insert(make_span(0));
    // The implementation allows inserts; verify it does not panic.
    assert!(cache.len() <= 1);
}

/// 3. Insert 100 spans into cache of size 10, verify only last 10 remain.
#[test]
fn cache_overflow_keeps_most_recent() {
    let mut cache = SpanCache::new(10);
    for i in 0..100 {
        cache.insert(make_span(i));
    }
    assert_eq!(cache.len(), 10);
    for i in 0..90 {
        assert!(!cache.contains(i), "span {i} should have been evicted");
    }
    for i in 90..100 {
        assert!(cache.contains(i), "span {i} should still be present");
    }
}

/// 4. Access pattern: insert A, B, C into size-2 cache; get(A) promotes it;
///    inserting D should evict B (the true LRU), not A.
#[test]
fn access_promotes_and_changes_eviction_order() {
    let mut cache = SpanCache::new(2);
    cache.insert(make_span(10)); // A
    cache.insert(make_span(20)); // B — evicts A since size=2

    // At this point cache has [10, 20]. Access 10 to promote it.
    let _ = cache.get(10);
    // Now LRU order: 20 (least recent), 10 (most recent).

    cache.insert(make_span(30)); // D — should evict 20 (LRU), not 10.
    assert!(cache.contains(10), "span 10 was promoted and should survive");
    assert!(!cache.contains(20), "span 20 was LRU and should be evicted");
    assert!(cache.contains(30));
}

/// 5. Duplicate insert updates in place without growing size.
#[test]
fn duplicate_insert_no_growth() {
    let mut cache = SpanCache::new(4);
    cache.insert(make_span(1));
    cache.insert(make_span(2));
    assert_eq!(cache.len(), 2);

    // Re-insert span 1 with same id.
    cache.insert(make_span(1));
    assert_eq!(cache.len(), 2, "size must not grow on duplicate insert");
    assert!(cache.contains(1));
    assert!(cache.contains(2));
}

/// 6. Get on missing key does not affect eviction order.
#[test]
fn get_missing_key_does_not_affect_order() {
    let mut cache = SpanCache::new(2);
    cache.insert(make_span(1)); // LRU
    cache.insert(make_span(2)); // MRU

    // Miss — should be a no-op.
    assert!(cache.get(999).is_none());

    // Insert a third span; span 1 should still be the LRU and get evicted.
    cache.insert(make_span(3));
    assert!(!cache.contains(1), "span 1 should be evicted as LRU");
    assert!(cache.contains(2));
    assert!(cache.contains(3));
}

// ---------------------------------------------------------------------------
// MockHeimdallClient tests
// ---------------------------------------------------------------------------

/// 7. fetch_state_sync_events filters by from_id correctly.
#[tokio::test]
async fn mock_events_filter_by_from_id() {
    let events: Vec<StateSyncEvent> = (1..=5).map(make_event).collect();
    let client = MockHeimdallClient::new().with_events(events);

    let result = client
        .fetch_state_sync_events(3, u64::MAX, 100)
        .await
        .unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 3);
    assert_eq!(result[1].id, 4);
    assert_eq!(result[2].id, 5);
}

/// 8. fetch_state_sync_events with limit caps results.
#[tokio::test]
async fn mock_events_limit_caps_results() {
    let events: Vec<StateSyncEvent> = (1..=10).map(make_event).collect();
    let client = MockHeimdallClient::new().with_events(events);

    let result = client
        .fetch_state_sync_events(1, u64::MAX, 3)
        .await
        .unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].id, 1);
    assert_eq!(result[2].id, 3);
}

/// 9. fetch_state_sync_events returns empty for no matching events.
#[tokio::test]
async fn mock_events_no_match_returns_empty() {
    let events = vec![make_event(1), make_event(2)];
    let client = MockHeimdallClient::new().with_events(events);

    let result = client
        .fetch_state_sync_events(100, u64::MAX, 10)
        .await
        .unwrap();
    assert!(result.is_empty());
}

/// 10. Multiple spans registered, fetch each by ID.
#[tokio::test]
async fn mock_multiple_spans_fetch_each() {
    let client = MockHeimdallClient::new()
        .with_span(0, make_span(0))
        .with_span(1, make_span(1))
        .with_span(2, make_span(2))
        .with_span(5, make_span(5));

    for id in [0, 1, 2, 5] {
        let span = client.fetch_span(id).await.unwrap();
        assert_eq!(span.id, id);
        assert_eq!(span.start_block, id * 6400);
    }
}

/// 11. NotFound error for unregistered span, checkpoint, and milestone.
#[tokio::test]
async fn mock_not_found_for_unregistered() {
    let client = MockHeimdallClient::new();

    let span_err = client.fetch_span(42).await.unwrap_err();
    assert!(
        matches!(span_err, HeimdallError::NotFound),
        "expected NotFound for missing span"
    );

    let cp_err = client.fetch_checkpoint(99).await.unwrap_err();
    assert!(
        matches!(cp_err, HeimdallError::NotFound),
        "expected NotFound for missing checkpoint"
    );

    let ms_err = client.fetch_milestone_latest().await.unwrap_err();
    assert!(
        matches!(ms_err, HeimdallError::NotFound),
        "expected NotFound for missing milestone"
    );
}

/// 12. Builder chaining - all methods compose together.
#[tokio::test]
async fn mock_builder_chaining_all_methods() {
    let checkpoint = Checkpoint {
        start_block: 0,
        end_block: 999,
        root_hash: B256::ZERO,
        proposer: Address::new([0xcc; 20]),
    };
    let milestone = Milestone {
        start_block: 0,
        end_block: 255,
        hash: B256::ZERO,
        proposer: Address::new([0xdd; 20]),
    };

    let client = MockHeimdallClient::new()
        .with_span(1, make_span(1))
        .with_span(2, make_span(2))
        .with_latest_span(make_span(99))
        .with_events(vec![make_event(1), make_event(2)])
        .with_checkpoint(1, checkpoint)
        .with_latest_milestone(milestone);

    assert_eq!(client.fetch_span(1).await.unwrap().id, 1);
    assert_eq!(client.fetch_span(2).await.unwrap().id, 2);
    assert_eq!(client.fetch_latest_span().await.unwrap().id, 99);
    assert_eq!(
        client
            .fetch_state_sync_events(1, u64::MAX, 100)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(client.fetch_checkpoint(1).await.unwrap().start_block, 0);
    assert_eq!(
        client.fetch_milestone_latest().await.unwrap().end_block,
        255
    );
}

/// 13. Empty mock returns NotFound for everything.
#[tokio::test]
async fn empty_mock_returns_not_found() {
    let client = MockHeimdallClient::new();

    assert!(client.fetch_span(0).await.is_err());
    assert!(client.fetch_latest_span().await.is_err());
    assert!(client.fetch_checkpoint(0).await.is_err());
    assert!(client.fetch_milestone_latest().await.is_err());

    // Events return Ok(empty vec), not an error.
    let events = client
        .fetch_state_sync_events(0, u64::MAX, 100)
        .await
        .unwrap();
    assert!(events.is_empty());
}

// ---------------------------------------------------------------------------
// Serde roundtrip tests
// ---------------------------------------------------------------------------

/// 14. StateSyncEvent JSON roundtrip.
#[test]
fn serde_roundtrip_state_sync_event() {
    let event = make_event(42);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: StateSyncEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, event.id);
    assert_eq!(deserialized.contract, event.contract);
    assert_eq!(deserialized.data, event.data);
    assert_eq!(deserialized.tx_hash, event.tx_hash);
    assert_eq!(deserialized.log_index, event.log_index);
    assert_eq!(deserialized.bor_chain_id, event.bor_chain_id);
    assert_eq!(deserialized.time, event.time);
}

/// 15. Checkpoint JSON roundtrip.
#[test]
fn serde_roundtrip_checkpoint() {
    let checkpoint = Checkpoint {
        start_block: 1000,
        end_block: 1999,
        root_hash: B256::new([0xab; 32]),
        proposer: Address::new([0xcc; 20]),
    };
    let json = serde_json::to_string(&checkpoint).unwrap();
    let deserialized: Checkpoint = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.start_block, checkpoint.start_block);
    assert_eq!(deserialized.end_block, checkpoint.end_block);
    assert_eq!(deserialized.root_hash, checkpoint.root_hash);
    assert_eq!(deserialized.proposer, checkpoint.proposer);
}

/// 16. Milestone JSON roundtrip.
#[test]
fn serde_roundtrip_milestone() {
    let milestone = Milestone {
        start_block: 500,
        end_block: 750,
        hash: B256::new([0xfe; 32]),
        proposer: Address::new([0xdd; 20]),
    };
    let json = serde_json::to_string(&milestone).unwrap();
    let deserialized: Milestone = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.start_block, milestone.start_block);
    assert_eq!(deserialized.end_block, milestone.end_block);
    assert_eq!(deserialized.hash, milestone.hash);
    assert_eq!(deserialized.proposer, milestone.proposer);
}
