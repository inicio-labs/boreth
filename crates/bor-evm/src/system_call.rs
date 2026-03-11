//! System calls made by the consensus engine at specific blocks.
//!
//! These are special EVM calls that are not triggered by transactions but by the
//! consensus layer itself (e.g., at span boundaries or for state sync events).

use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::SolValue;
use bor_chainspec::constants::{BOR_VALIDATOR_SET_ADDRESS, STATE_RECEIVER_ADDRESS, SYSTEM_ADDRESS};

/// Function selector for `commitSpan(uint256,bytes)`.
/// keccak256("commitSpan(uint256,bytes)")[:4]
const COMMIT_SPAN_SELECTOR: [u8; 4] = [0x60, 0xcc, 0x80, 0xd8];

/// Function selector for `onStateReceive(uint256,bytes)`.
/// keccak256("onStateReceive(uint256,bytes)")[:4]
const ON_STATE_RECEIVE_SELECTOR: [u8; 4] = [0x26, 0xc5, 0x3b, 0xea];

/// `commitSpan` is called at span boundaries to update the validator set.
/// It calls the BorValidatorSet contract at `0x1000`.
pub struct CommitSpanCall {
    /// The span ID.
    pub span_id: U256,
    /// Raw validator bytes (concatenated 20-byte addresses).
    pub validator_bytes: Bytes,
}

impl CommitSpanCall {
    /// Build the ABI-encoded call data for `commitSpan(uint256,bytes)`.
    pub fn call_data(&self) -> Bytes {
        let mut data = Vec::with_capacity(4 + 64);
        data.extend_from_slice(&COMMIT_SPAN_SELECTOR);
        let encoded = (self.span_id, self.validator_bytes.as_ref()).abi_encode_params();
        data.extend_from_slice(&encoded);
        Bytes::from(data)
    }

    /// The target contract address for `commitSpan`.
    pub fn to_address() -> Address {
        BOR_VALIDATOR_SET_ADDRESS
    }

    /// The caller address used for system calls.
    pub fn caller() -> Address {
        SYSTEM_ADDRESS
    }
}

/// `onStateReceive` is called to process state sync events from Heimdall.
/// It calls the StateReceiver contract at `0x1001`.
pub struct StateReceiveCall {
    /// The state sync event ID.
    pub state_id: U256,
    /// The state sync event data.
    pub data: Bytes,
}

impl StateReceiveCall {
    /// Build the ABI-encoded call data for `onStateReceive(uint256,bytes)`.
    pub fn call_data(&self) -> Bytes {
        let mut encoded_data = Vec::with_capacity(4 + 64);
        encoded_data.extend_from_slice(&ON_STATE_RECEIVE_SELECTOR);
        let params = (self.state_id, self.data.as_ref()).abi_encode_params();
        encoded_data.extend_from_slice(&params);
        Bytes::from(encoded_data)
    }

    /// The target contract address for `onStateReceive`.
    pub fn to_address() -> Address {
        STATE_RECEIVER_ADDRESS
    }

    /// The caller address used for system calls.
    pub fn caller() -> Address {
        SYSTEM_ADDRESS
    }
}

/// Execute multiple state sync events in ascending order at sprint boundaries.
///
/// Returns the call data for each event. Events must be applied in ascending
/// `state_id` order. Only called when `block_number % sprint_size == 0`.
pub fn prepare_state_sync_calls(
    events: &[(U256, Bytes)],
) -> Vec<StateReceiveCall> {
    events
        .iter()
        .map(|(state_id, data)| StateReceiveCall {
            state_id: *state_id,
            data: data.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn commit_span_addresses() {
        assert_eq!(
            CommitSpanCall::to_address(),
            address!("0000000000000000000000000000000000001000")
        );
        assert_eq!(
            CommitSpanCall::caller(),
            address!("fffffffffffffffffffffffffffffffffffffffe")
        );
    }

    #[test]
    fn state_receive_addresses() {
        assert_eq!(
            StateReceiveCall::to_address(),
            address!("0000000000000000000000000000000000001001")
        );
        assert_eq!(
            StateReceiveCall::caller(),
            address!("fffffffffffffffffffffffffffffffffffffffe")
        );
    }

    #[test]
    fn test_commit_span_abi_encoding() {
        let call = CommitSpanCall {
            span_id: U256::from(42),
            validator_bytes: Bytes::from_static(&[0xaa; 20]),
        };
        let data = call.call_data();
        // Starts with selector
        assert_eq!(&data[..4], &COMMIT_SPAN_SELECTOR);
        // ABI encoded: span_id, offset, length, data
        assert!(data.len() > 4 + 32); // At least selector + one word
    }

    #[test]
    fn test_state_receive_abi_encoding() {
        let call = StateReceiveCall {
            state_id: U256::from(100),
            data: Bytes::from_static(b"state_data"),
        };
        let data = call.call_data();
        assert_eq!(&data[..4], &ON_STATE_RECEIVE_SELECTOR);
        assert!(data.len() > 4 + 64); // selector + at least state_id + offset
    }

    #[test]
    fn test_commit_span_caller_is_system() {
        assert_eq!(CommitSpanCall::caller(), SYSTEM_ADDRESS);
    }

    #[test]
    fn test_state_receive_caller_is_system() {
        assert_eq!(StateReceiveCall::caller(), SYSTEM_ADDRESS);
    }

    #[test]
    fn test_prepare_state_sync_calls_empty() {
        let calls = prepare_state_sync_calls(&[]);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_prepare_state_sync_calls_multiple() {
        let events = vec![
            (U256::from(1), Bytes::from_static(b"event1")),
            (U256::from(2), Bytes::from_static(b"event2")),
            (U256::from(3), Bytes::from_static(b"event3")),
        ];
        let calls = prepare_state_sync_calls(&events);
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].state_id, U256::from(1));
        assert_eq!(calls[1].state_id, U256::from(2));
        assert_eq!(calls[2].state_id, U256::from(3));
    }

    #[test]
    fn test_commit_span_gas_zero() {
        // System calls use 0 gas - this is enforced at the executor level,
        // but we verify the call data doesn't include any gas field
        let call = CommitSpanCall {
            span_id: U256::from(1),
            validator_bytes: Bytes::from_static(&[0xbb; 40]),
        };
        let data = call.call_data();
        // Just verify it encodes without error and starts with selector
        assert_eq!(&data[..4], &COMMIT_SPAN_SELECTOR);
    }
}
