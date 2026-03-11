//! HTTP client for interacting with the Heimdall REST API.

use crate::{Checkpoint, HeimdallClient, HeimdallError, Milestone, StateSyncEvent};
use bor_primitives::Span;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

/// Maximum number of retry attempts for HTTP requests.
const MAX_RETRIES: u32 = 3;

/// Base delay between retries (doubled on each subsequent attempt).
const BASE_RETRY_DELAY: Duration = Duration::from_millis(500);

/// Wrapper for Heimdall API responses that nest the actual payload under a `"result"` key.
#[derive(Debug, Deserialize)]
struct HeimdallResponse<T> {
    result: T,
}

/// An HTTP-based Heimdall client that communicates with the Heimdall REST API.
#[derive(Debug, Clone)]
pub struct HttpHeimdallClient {
    /// The base URL of the Heimdall API (e.g. `http://localhost:1317`).
    base_url: String,
    /// The inner reqwest HTTP client.
    client: Client,
}

impl HttpHeimdallClient {
    /// Create a new [`HttpHeimdallClient`] with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        Self {
            base_url,
            client: Client::new(),
        }
    }

    /// Execute a GET request with retry logic (exponential backoff, up to [`MAX_RETRIES`]
    /// attempts).
    async fn get_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, HeimdallError> {
        let url = format!("{}{}", self.base_url, path);
        let mut last_err = HeimdallError::NetworkError("no attempts made".into());

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_RETRY_DELAY * 2u32.pow(attempt - 1);
                tokio::time::sleep(delay).await;
            }

            match self.client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return resp.json::<T>().await.map_err(|e| {
                            HeimdallError::InvalidResponse(format!(
                                "failed to parse response: {e}"
                            ))
                        });
                    } else if status.as_u16() == 404 {
                        return Err(HeimdallError::NotFound);
                    } else if status.as_u16() == 429 {
                        last_err = HeimdallError::RateLimited;
                        continue;
                    } else {
                        last_err = HeimdallError::InvalidResponse(format!(
                            "unexpected status code: {status}"
                        ));
                        continue;
                    }
                }
                Err(e) => {
                    if e.is_timeout() {
                        last_err = HeimdallError::Timeout;
                    } else {
                        last_err = HeimdallError::NetworkError(e.to_string());
                    }
                    continue;
                }
            }
        }

        Err(last_err)
    }
}

impl HeimdallClient for HttpHeimdallClient {
    async fn fetch_span(&self, span_id: u64) -> Result<Span, HeimdallError> {
        let resp: HeimdallResponse<Span> =
            self.get_with_retry(&format!("/bor/span/{span_id}")).await?;
        Ok(resp.result)
    }

    async fn fetch_latest_span(&self) -> Result<Span, HeimdallError> {
        let resp: HeimdallResponse<Span> = self.get_with_retry("/bor/latest-span").await?;
        Ok(resp.result)
    }

    async fn fetch_state_sync_events(
        &self,
        from_id: u64,
        to_time: u64,
        limit: usize,
    ) -> Result<Vec<StateSyncEvent>, HeimdallError> {
        let path = format!(
            "/clerk/event-record/list?from-id={from_id}&to-time={to_time}&limit={limit}"
        );
        let resp: HeimdallResponse<Vec<StateSyncEvent>> = self.get_with_retry(&path).await?;
        Ok(resp.result)
    }

    async fn fetch_checkpoint(&self, number: u64) -> Result<Checkpoint, HeimdallError> {
        let resp: HeimdallResponse<Checkpoint> =
            self.get_with_retry(&format!("/checkpoints/{number}")).await?;
        Ok(resp.result)
    }

    async fn fetch_milestone_latest(&self) -> Result<Milestone, HeimdallError> {
        let resp: HeimdallResponse<Milestone> =
            self.get_with_retry("/milestones/latest").await?;
        Ok(resp.result)
    }
}
