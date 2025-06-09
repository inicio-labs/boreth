use std::time::SystemTime;

use crate::heimdall::{
    error::HeimdallError, event::EventRecordWithTime,
    genesis_contract_client::GenesisContractClient,
};

use alloy_rlp::encode;
use alloy_sol_types::{
    SolCall,
    private::{Bytes, Uint},
    sol,
};

impl GenesisContractClient {
    pub fn encode_state_sync_data(
        &self,
        event_record_with_time: EventRecordWithTime,
    ) -> Result<Vec<u8>, HeimdallError> {
        sol! {
            function SYSTEM_ADDRESS() view returns (address);
            function lastStateId() view returns (uint256);
            function commitState(uint256 syncTime, bytes recordBytes) returns (bool);
        }

        let record_bytes = encode(&event_record_with_time.event_record);
        let sync_time = event_record_with_time
            .time
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        let commit_state = commitStateCall {
            syncTime: Uint::from(sync_time),
            recordBytes: Bytes::from(record_bytes),
        };

        Ok(commit_state.abi_encode())
    }

    pub fn last_state_id(&self) -> Result<Vec<u8>, HeimdallError> {
        sol! {
            function lastStateId() view returns (uint256);
        }

        let last_state_id = lastStateIdCall {};

        Ok(last_state_id.abi_encode())
    }

    pub fn decode_last_state_id(&self, data: &Bytes) -> Result<u64, HeimdallError> {
        sol! {
            function lastStateId() view returns (uint256);
        }

        let last_state_id = lastStateIdCall::abi_decode_returns(&data)
            .map_err(|e| HeimdallError::SolDecodeError(e.to_string()))?;

        let result = last_state_id.try_into().map_err(|_| {
            HeimdallError::SolDecodeError("Failed to decode last state id".to_string())
        })?;

        Ok(result)
    }
}
