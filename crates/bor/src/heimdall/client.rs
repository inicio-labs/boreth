use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;
use url::Url;

use crate::heimdall::{
    error::HeimdallError,
    event::{EventRecordWithTime, FETCH_STATE_SYNC_EVENTS_PATH, StateSyncEventsResponse},
    span::{FETCH_SPAN_FORMAT, HeimdallSpan, SpanResponse},
};

const API_HEIMDALL_TIMEOUT: Duration = Duration::from_secs(5);
const STATE_FETCH_LIMIT: u64 = 50;

#[derive(Debug, Clone)]
pub struct HeimdallClient {
    base_url: Url,
    client: reqwest::blocking::Client,
}

impl HeimdallClient {
    pub fn new(url_string: &str) -> Result<Self, HeimdallError> {
        let base_url = Url::parse(url_string)?;
        let client = reqwest::blocking::Client::builder()
            .timeout(API_HEIMDALL_TIMEOUT)
            .build()?;

        Ok(Self { base_url, client })
    }

    /// Fetches a span from Heimdall.
    /// Corresponds to `bor/span/%d`
    pub fn fetch_span(&self, span_id: u64) -> Result<HeimdallSpan, HeimdallError> {
        let url = span_url(&self.base_url, span_id)?;

        let response = self.client.get(url).send()?;

        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Err(HeimdallError::NoResponse);
        }

        if !response.status().is_success() {
            return Err(HeimdallError::UnsuccessfulResponse(response.status()));
        }

        let span_response = response.json::<SpanResponse>()?;
        Ok(span_response.result)
    }

    /// Fetches state sync events from Heimdall.
    /// Corresponds to `clerk/event-record/list`
    /// This method handles pagination as seen in the Go example.
    pub fn fetch_state_sync_events(
        &self,
        from_id: u64,
        to_time: u64,
    ) -> Result<Vec<EventRecordWithTime>, HeimdallError> {
        // TODO: Try to do some optimization later using state fetch limit

        let url = state_sync_url(&self.base_url, from_id, to_time)?;

        let response = self.client.get(url).send()?;

        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Err(HeimdallError::NoResponse);
        }

        if !response.status().is_success() {
            return Err(HeimdallError::UnsuccessfulResponse(response.status()));
        }

        let page = response.json::<StateSyncEventsResponse>()?;

        let mut event_records = page.result.ok_or(HeimdallError::NoResponse)?;

        event_records.sort();

        Ok(event_records)
    }
}

fn state_sync_url(base_url: &Url, from_id: u64, to_time: u64) -> Result<Url, HeimdallError> {
    let mut url = base_url.join(FETCH_STATE_SYNC_EVENTS_PATH)?;
    url.set_query(Some(&format!(
        "from-id={}&to-time={}&limit={}",
        from_id, to_time, STATE_FETCH_LIMIT
    )));

    Ok(url)
}

fn span_url(base_url: &Url, span_id: u64) -> Result<Url, HeimdallError> {
    let mut url = base_url.join(FETCH_SPAN_FORMAT)?;
    url.set_query(Some(&format!("span-id={}", span_id)));

    Ok(url)
}
