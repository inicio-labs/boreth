use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;
use url::Url;

const API_HEIMDALL_TIMEOUT: Duration = Duration::from_secs(5);
const STATE_FETCH_LIMIT: u64 = 50;

#[derive(Error, Debug)]
pub enum HeimdallError {
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("JSON deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Heimdall returned an unsuccessful status code: {0}")]
    UnsuccessfulResponse(reqwest::StatusCode),

    #[error("Heimdall returned no response body")]
    NoResponse,
}

// NOTE: This is a placeholder struct based on the Go code.
// You should fill in the fields to match the actual JSON response from the API.
#[derive(Debug, Deserialize)]
pub struct HeimdallSpan {
    pub span_id: u64,
    pub start_block: u64,
    pub end_block: u64,
}

#[derive(Debug, Deserialize)]
struct SpanResponse {
    #[allow(dead_code)]
    height: String,
    result: HeimdallSpan,
}

// NOTE: This is a placeholder struct.
// You should fill in the fields to match the actual JSON response.
// The `id` field is used for sorting, as seen in the Go code.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventRecordWithTime {
    #[serde(rename = "ID")]
    pub id: u64,
    // Add other fields from the event record here
    // pub tx_hash: String,
}

#[derive(Debug, Deserialize)]
struct StateSyncEventsResponse {
    #[allow(dead_code)]
    height: String,
    result: Option<Vec<EventRecordWithTime>>,
}

pub struct HeimdallClient {
    base_url: Url,
    client: reqwest::Client,
}

impl HeimdallClient {
    pub fn new(url_string: &str) -> Result<Self, HeimdallError> {
        let base_url = Url::parse(url_string)?;
        let client = reqwest::Client::builder()
            .timeout(API_HEIMDALL_TIMEOUT)
            .build()?;

        Ok(Self { base_url, client })
    }

    /// Fetches a span from Heimdall.
    /// Corresponds to `bor/span/%d`
    pub async fn fetch_span(&self, span_id: u64) -> Result<HeimdallSpan, HeimdallError> {
        let path = format!("bor/span/{}", span_id);
        let url = self.base_url.join(&path)?;

        let response = self.client.get(url).send().await?;

        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Err(HeimdallError::NoResponse);
        }

        if !response.status().is_success() {
            return Err(HeimdallError::UnsuccessfulResponse(response.status()));
        }

        let span_response = response.json::<SpanResponse>().await?;
        Ok(span_response.result)
    }

    /// Fetches state sync events from Heimdall.
    /// Corresponds to `clerk/event-record/list`
    /// This method handles pagination as seen in the Go example.
    pub async fn fetch_state_sync_events(
        &self,
        mut from_id: u64,
        to_time: i64,
    ) -> Result<Vec<EventRecordWithTime>, HeimdallError> {
        let mut event_records = Vec::new();
        let path = "clerk/event-record/list";

        loop {
            let mut url = self.base_url.join(path)?;
            url.set_query(Some(&format!(
                "from-id={}&to-time={}&limit={}",
                from_id, to_time, STATE_FETCH_LIMIT
            )));

            let response = self.client.get(url).send().await?;

            if response.status() == reqwest::StatusCode::NO_CONTENT {
                break;
            }

            if !response.status().is_success() {
                return Err(HeimdallError::UnsuccessfulResponse(response.status()));
            }

            let page = response.json::<StateSyncEventsResponse>().await?;
            let Some(mut results) = page.result else {
                break;
            };

            if results.is_empty() {
                break;
            }

            let fetched_count = results.len();
            event_records.append(&mut results);

            if (fetched_count as u64) < STATE_FETCH_LIMIT {
                break;
            }

            from_id += STATE_FETCH_LIMIT;
        }

        event_records.sort(); // sort_by_key(|e| e.id) is equivalent since Ord is derived on `id`

        Ok(event_records)
    }
}
