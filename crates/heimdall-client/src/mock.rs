//! Mock Heimdall client for testing.

use crate::{Checkpoint, HeimdallClient, HeimdallError, Milestone, StateSyncEvent};
use bor_primitives::Span;
use std::collections::HashMap;
use std::sync::Arc;

/// A mock Heimdall client that returns pre-configured responses.
///
/// Build one using the builder pattern:
/// ```ignore
/// let client = MockHeimdallClient::new()
///     .with_span(1, span)
///     .with_events(events);
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockHeimdallClient {
    spans: Arc<HashMap<u64, Span>>,
    latest_span: Option<Arc<Span>>,
    events: Arc<Vec<StateSyncEvent>>,
    checkpoints: Arc<HashMap<u64, Checkpoint>>,
    latest_milestone: Option<Arc<Milestone>>,
}

impl MockHeimdallClient {
    /// Create a new empty mock client.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a span for a given ID.
    pub fn with_span(mut self, id: u64, span: Span) -> Self {
        Arc::make_mut(&mut self.spans).insert(id, span);
        self
    }

    /// Set the latest span.
    pub fn with_latest_span(mut self, span: Span) -> Self {
        self.latest_span = Some(Arc::new(span));
        self
    }

    /// Set state-sync events to be returned.
    pub fn with_events(mut self, events: Vec<StateSyncEvent>) -> Self {
        self.events = Arc::new(events);
        self
    }

    /// Register a checkpoint for a given number.
    pub fn with_checkpoint(mut self, number: u64, checkpoint: Checkpoint) -> Self {
        Arc::make_mut(&mut self.checkpoints).insert(number, checkpoint);
        self
    }

    /// Set the latest milestone.
    pub fn with_latest_milestone(mut self, milestone: Milestone) -> Self {
        self.latest_milestone = Some(Arc::new(milestone));
        self
    }
}

impl HeimdallClient for MockHeimdallClient {
    async fn fetch_span(&self, span_id: u64) -> Result<Span, HeimdallError> {
        self.spans
            .get(&span_id)
            .cloned()
            .ok_or(HeimdallError::NotFound)
    }

    async fn fetch_latest_span(&self) -> Result<Span, HeimdallError> {
        self.latest_span
            .as_ref()
            .map(|s| s.as_ref().clone())
            .ok_or(HeimdallError::NotFound)
    }

    async fn fetch_state_sync_events(
        &self,
        from_id: u64,
        _to_time: u64,
        limit: usize,
    ) -> Result<Vec<StateSyncEvent>, HeimdallError> {
        let filtered: Vec<StateSyncEvent> = self
            .events
            .iter()
            .filter(|e| e.id >= from_id)
            .take(limit)
            .cloned()
            .collect();
        Ok(filtered)
    }

    async fn fetch_checkpoint(&self, number: u64) -> Result<Checkpoint, HeimdallError> {
        self.checkpoints
            .get(&number)
            .cloned()
            .ok_or(HeimdallError::NotFound)
    }

    async fn fetch_milestone_latest(&self) -> Result<Milestone, HeimdallError> {
        self.latest_milestone
            .as_ref()
            .map(|m| m.as_ref().clone())
            .ok_or(HeimdallError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, Bytes, B256};
    use bor_primitives::{Validator, ValidatorSet};

    fn sample_validator() -> Validator {
        Validator {
            id: 1,
            address: Address::new([0xaa; 20]),
            voting_power: 100,
            signer: Address::new([0xaa; 20]),
            proposer_priority: 0,
        }
    }

    fn sample_span(id: u64) -> Span {
        Span {
            id,
            start_block: id * 6400,
            end_block: (id + 1) * 6400 - 1,
            validator_set: ValidatorSet {
                validators: vec![sample_validator()],
                proposer: Some(sample_validator()),
            },
            selected_producers: vec![sample_validator()],
            bor_chain_id: "137".to_string(),
        }
    }

    fn sample_event(id: u64) -> StateSyncEvent {
        StateSyncEvent {
            id,
            contract: Address::new([0xbb; 20]),
            data: Bytes::from(vec![0x01, 0x02]),
            tx_hash: B256::ZERO,
            log_index: 0,
            bor_chain_id: "137".to_string(),
            time: 1_000_000 + id,
        }
    }

    fn sample_checkpoint(number: u64) -> Checkpoint {
        Checkpoint {
            start_block: number * 1000,
            end_block: (number + 1) * 1000 - 1,
            root_hash: B256::ZERO,
            proposer: Address::new([0xcc; 20]),
        }
    }

    fn sample_milestone() -> Milestone {
        Milestone {
            start_block: 0,
            end_block: 255,
            hash: B256::ZERO,
            proposer: Address::new([0xdd; 20]),
        }
    }

    #[tokio::test]
    async fn test_mock_fetch_span() {
        let client = MockHeimdallClient::new().with_span(1, sample_span(1));

        let span = client.fetch_span(1).await.unwrap();
        assert_eq!(span.id, 1);
        assert_eq!(span.start_block, 6400);

        let err = client.fetch_span(99).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_mock_fetch_latest_span() {
        let client = MockHeimdallClient::new().with_latest_span(sample_span(5));

        let span = client.fetch_latest_span().await.unwrap();
        assert_eq!(span.id, 5);

        let empty = MockHeimdallClient::new();
        assert!(empty.fetch_latest_span().await.is_err());
    }

    #[tokio::test]
    async fn test_mock_fetch_state_sync_events() {
        let events = vec![sample_event(1), sample_event(2), sample_event(3)];
        let client = MockHeimdallClient::new().with_events(events);

        let result = client.fetch_state_sync_events(2, u64::MAX, 10).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, 2);
        assert_eq!(result[1].id, 3);

        let limited = client.fetch_state_sync_events(1, u64::MAX, 2).await.unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_fetch_checkpoint() {
        let client = MockHeimdallClient::new().with_checkpoint(1, sample_checkpoint(1));

        let cp = client.fetch_checkpoint(1).await.unwrap();
        assert_eq!(cp.start_block, 1000);

        assert!(client.fetch_checkpoint(99).await.is_err());
    }

    #[tokio::test]
    async fn test_mock_fetch_milestone_latest() {
        let client = MockHeimdallClient::new().with_latest_milestone(sample_milestone());

        let ms = client.fetch_milestone_latest().await.unwrap();
        assert_eq!(ms.end_block, 255);

        let empty = MockHeimdallClient::new();
        assert!(empty.fetch_milestone_latest().await.is_err());
    }

    #[tokio::test]
    async fn test_builder_chaining() {
        let client = MockHeimdallClient::new()
            .with_span(1, sample_span(1))
            .with_span(2, sample_span(2))
            .with_latest_span(sample_span(2))
            .with_events(vec![sample_event(1)])
            .with_checkpoint(1, sample_checkpoint(1))
            .with_latest_milestone(sample_milestone());

        assert!(client.fetch_span(1).await.is_ok());
        assert!(client.fetch_span(2).await.is_ok());
        assert!(client.fetch_latest_span().await.is_ok());
        assert_eq!(client.fetch_state_sync_events(1, u64::MAX, 10).await.unwrap().len(), 1);
        assert!(client.fetch_checkpoint(1).await.is_ok());
        assert!(client.fetch_milestone_latest().await.is_ok());
    }
}
