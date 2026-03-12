//! Background span prefetcher for eagerly populating the span cache.
//!
//! Runs as a background task that monitors the chain tip and pre-fetches
//! Heimdall spans before block validation needs them.

use crate::BorConsensus;
use heimdall_client::HeimdallClient;
use reth_chainspec::{EthChainSpec, EthereumHardforks};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Default span size (pre-Rio). TODO: make configurable per chain spec.
const DEFAULT_SPAN_SIZE: u64 = 6400;

/// How many spans ahead of the current block to pre-fetch.
const PREFETCH_AHEAD: u64 = 2;

/// Poll interval when waiting for chain tip updates.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Background span prefetcher that fetches Heimdall spans ahead of block validation.
///
/// This component monitors a block number source and ensures the span cache
/// in `BorConsensus` is populated with the current span and upcoming spans.
pub struct SpanPrefetcher<C, ChainSpec> {
    /// The Heimdall client to fetch spans from.
    client: C,
    /// Shared reference to the BorConsensus engine (owns the span cache).
    consensus: Arc<BorConsensus<ChainSpec>>,
    /// The highest span ID that has been successfully fetched and cached.
    last_fetched_span: Option<u64>,
    /// Span size in blocks (depends on chain fork).
    span_size: u64,
}

impl<C, ChainSpec> SpanPrefetcher<C, ChainSpec>
where
    C: HeimdallClient,
    ChainSpec: EthChainSpec + EthereumHardforks + Debug + Send + Sync,
{
    /// Create a new span prefetcher.
    pub fn new(
        client: C,
        consensus: Arc<BorConsensus<ChainSpec>>,
    ) -> Self {
        Self {
            client,
            consensus,
            last_fetched_span: None,
            span_size: DEFAULT_SPAN_SIZE,
        }
    }

    /// Create a new span prefetcher with a custom span size.
    pub fn with_span_size(mut self, span_size: u64) -> Self {
        self.span_size = span_size;
        self
    }

    /// Fetch a single span by ID and insert it into the consensus span cache.
    /// Returns `true` if the span was fetched successfully.
    async fn fetch_and_cache_span(&self, span_id: u64) -> bool {
        match self.client.fetch_span(span_id).await {
            Ok(span) => {
                debug!(
                    target: "bor::prefetch",
                    span_id,
                    start_block = span.start_block,
                    end_block = span.end_block,
                    validators = span.validator_set.validators.len(),
                    "cached span"
                );
                self.consensus.insert_span(span);
                true
            }
            Err(heimdall_client::HeimdallError::NotFound) => {
                // Span doesn't exist yet (we're ahead of the chain)
                debug!(target: "bor::prefetch", span_id, "span not found on Heimdall (not yet produced)");
                false
            }
            Err(e) => {
                warn!(target: "bor::prefetch", span_id, error = %e, "failed to fetch span");
                false
            }
        }
    }

    /// Ensure spans are cached for the given block number and ahead.
    async fn ensure_spans_for_block(&mut self, block_number: u64) {
        let current_span_id = bor_primitives::span_id_at(block_number, self.span_size);

        // Fetch current span + PREFETCH_AHEAD spans
        for offset in 0..=PREFETCH_AHEAD {
            let span_id = current_span_id + offset;

            // Skip if already fetched
            if self.last_fetched_span.is_some_and(|last| span_id <= last) {
                continue;
            }

            if self.fetch_and_cache_span(span_id).await {
                self.last_fetched_span = Some(span_id);
            } else {
                // Stop prefetching ahead if we hit a missing span
                break;
            }
        }
    }

    /// Run the prefetcher as a background loop.
    ///
    /// `get_block_number` is a closure that returns the current best block number.
    /// The prefetcher polls this periodically and pre-fetches spans as needed.
    pub async fn run<F>(mut self, get_block_number: F)
    where
        F: Fn() -> Option<u64> + Send,
    {
        info!(target: "bor::prefetch", span_size = self.span_size, "span prefetcher started");

        // Fetch span 0 (genesis span) immediately
        self.fetch_and_cache_span(0).await;

        loop {
            if let Some(block_number) = get_block_number() {
                self.ensure_spans_for_block(block_number).await;
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Run a single prefetch pass for a given block number.
    /// Useful for testing or one-shot prefetching.
    pub async fn prefetch_for_block(&mut self, block_number: u64) {
        self.ensure_spans_for_block(block_number).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bor_primitives::{Span, Validator, ValidatorSet};
    use heimdall_client::MockHeimdallClient;
    use reth_chainspec::ChainSpec;

    fn make_test_consensus() -> Arc<BorConsensus<ChainSpec>> {
        use reth_chainspec::ChainSpecBuilder;
        let spec = ChainSpecBuilder::default()
            .chain(alloy_chains::Chain::from_id(137))
            .genesis(alloy_genesis::Genesis::default())
            .london_activated()
            .paris_activated()
            .build();
        Arc::new(BorConsensus::new(Arc::new(spec)))
    }

    fn make_span(id: u64, span_size: u64) -> Span {
        Span {
            id,
            start_block: id * span_size,
            end_block: (id + 1) * span_size - 1,
            validator_set: ValidatorSet {
                validators: vec![Validator {
                    id: 1,
                    address: alloy_primitives::Address::ZERO,
                    voting_power: 100,
                    signer: alloy_primitives::Address::ZERO,
                    proposer_priority: 0,
                }],
                proposer: None,
            },
            selected_producers: vec![],
            bor_chain_id: "137".to_string(),
        }
    }

    #[tokio::test]
    async fn test_prefetch_caches_spans() {
        let consensus = make_test_consensus();
        let mock = MockHeimdallClient::new()
            .with_span(0, make_span(0, 6400))
            .with_span(1, make_span(1, 6400))
            .with_span(2, make_span(2, 6400));

        let mut prefetcher = SpanPrefetcher::new(mock, consensus.clone());
        prefetcher.prefetch_for_block(6400).await; // span 1

        // Should have cached span 1, 2, and 3 (but 3 doesn't exist)
        assert!(consensus.span_cache.lock().unwrap().contains(1));
        assert!(consensus.span_cache.lock().unwrap().contains(2));
    }

    #[tokio::test]
    async fn test_prefetch_genesis_span() {
        let consensus = make_test_consensus();
        let mock = MockHeimdallClient::new()
            .with_span(0, make_span(0, 6400));

        let mut prefetcher = SpanPrefetcher::new(mock, consensus.clone());
        prefetcher.prefetch_for_block(0).await;

        assert!(consensus.span_cache.lock().unwrap().contains(0));
    }

    #[tokio::test]
    async fn test_prefetch_skips_already_cached() {
        let consensus = make_test_consensus();
        let mock = MockHeimdallClient::new()
            .with_span(0, make_span(0, 6400))
            .with_span(1, make_span(1, 6400));

        let mut prefetcher = SpanPrefetcher::new(mock, consensus.clone());

        // First fetch
        prefetcher.prefetch_for_block(0).await;
        assert!(consensus.span_cache.lock().unwrap().contains(0));

        // Second fetch at same block should not re-fetch
        prefetcher.prefetch_for_block(0).await;
        // Verifies no panics or errors on repeat fetch
    }
}
