use std::{cmp::Ordering, time::SystemTime};

use alloy_primitives::{Address, Bytes, TxHash};

use alloy_rlp::RlpEncodable;
use serde::Deserialize;

pub const FETCH_STATE_SYNC_EVENTS_FORMAT: &str = "from-id={}&to-time={}&limit={}";
pub const FETCH_STATE_SYNC_EVENTS_PATH: &str = "clerk/event-record/list";

#[derive(Debug, Clone, Deserialize, RlpEncodable, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventRecord {
    pub id: u64,
    pub contract_address: Address,
    pub data: Bytes,
    pub tx_hash: TxHash,
    pub log_index: u64,
    pub chain_id: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct EventRecordWithTime {
    pub event_record: EventRecord,
    pub time: SystemTime,
}

impl PartialOrd for EventRecordWithTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EventRecordWithTime {
    fn cmp(&self, other: &Self) -> Ordering {
        self.event_record.id.cmp(&other.event_record.id)
    }
}

#[derive(Debug, Deserialize)]
pub struct StateSyncEventsResponse {
    #[allow(dead_code)]
    pub height: String,
    pub result: Option<Vec<EventRecordWithTime>>,
}
