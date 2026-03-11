//! Heimdall client for interacting with the Heimdall layer.

mod cache;
pub use cache::SpanCache;

use alloy_primitives::{Address, Bytes, B256};
use bor_primitives::Span;
use serde::{Deserialize, Serialize};

/// Errors that can occur when communicating with the Heimdall service.
#[derive(Debug, thiserror::Error)]
pub enum HeimdallError {
    /// A network-level error occurred (e.g. connection refused, DNS failure).
    #[error("network error: {0}")]
    NetworkError(String),

    /// The request timed out before a response was received.
    #[error("request timeout")]
    Timeout,

    /// The response was received but could not be parsed or was otherwise invalid.
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// The requested resource was not found on the Heimdall server.
    #[error("not found")]
    NotFound,

    /// The client has been rate-limited by the Heimdall server.
    #[error("rate limited")]
    RateLimited,
}

/// A state-sync event relayed from Ethereum L1 to Bor via Heimdall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncEvent {
    /// Unique monotonically increasing identifier for this event.
    pub id: u64,
    /// The L1 contract address that emitted the event.
    pub contract: Address,
    /// The ABI-encoded event data payload.
    pub data: Bytes,
    /// The L1 transaction hash that triggered this event.
    pub tx_hash: B256,
    /// The log index within the L1 transaction.
    pub log_index: u64,
    /// The Bor chain ID this event targets.
    pub bor_chain_id: String,
    /// Unix timestamp of the event.
    pub time: u64,
}

/// A Heimdall checkpoint covering a range of Bor blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The first Bor block included in this checkpoint.
    pub start_block: u64,
    /// The last Bor block included in this checkpoint.
    pub end_block: u64,
    /// The root hash of the checkpoint.
    pub root_hash: B256,
    /// The address of the validator that proposed this checkpoint.
    pub proposer: Address,
}

/// A Heimdall milestone covering a range of Bor blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// The first Bor block included in this milestone.
    pub start_block: u64,
    /// The last Bor block included in this milestone.
    pub end_block: u64,
    /// The hash of the end block.
    pub hash: B256,
    /// The address of the validator that proposed this milestone.
    pub proposer: Address,
}

/// Client interface for fetching data from the Heimdall layer.
pub trait HeimdallClient: Send + Sync {
    /// Fetch a specific span by its ID.
    fn fetch_span(
        &self,
        span_id: u64,
    ) -> impl Future<Output = Result<Span, HeimdallError>> + Send;

    /// Fetch the latest span.
    fn fetch_latest_span(
        &self,
    ) -> impl Future<Output = Result<Span, HeimdallError>> + Send;

    /// Fetch state-sync events starting from `from_id` up to `to_time`, returning at most `limit`
    /// events.
    fn fetch_state_sync_events(
        &self,
        from_id: u64,
        to_time: u64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<StateSyncEvent>, HeimdallError>> + Send;

    /// Fetch a specific checkpoint by its number.
    fn fetch_checkpoint(
        &self,
        number: u64,
    ) -> impl Future<Output = Result<Checkpoint, HeimdallError>> + Send;

    /// Fetch the latest milestone.
    fn fetch_milestone_latest(
        &self,
    ) -> impl Future<Output = Result<Milestone, HeimdallError>> + Send;
}
