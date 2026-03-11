//! Primitive types for the Bor chain.

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

/// A Bor validator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub id: u64,
    pub address: Address,
    pub voting_power: i64,
    pub signer: Address,
    pub proposer_priority: i64,
}

/// A set of validators with an optional proposer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub validators: Vec<Validator>,
    pub proposer: Option<Validator>,
}

/// A Bor span defining a range of blocks and its validator set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub id: u64,
    pub start_block: u64,
    pub end_block: u64,
    pub validator_set: ValidatorSet,
    pub selected_producers: Vec<Validator>,
    pub bor_chain_id: String,
}

/// Returns the span ID for a given block number and span size.
pub fn span_id_at(block: u64, span_size: u64) -> u64 {
    block / span_size
}

/// Encodes a slice of validators into raw bytes by concatenating their 20-byte signer addresses.
pub fn encode_validator_bytes(validators: &[Validator]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(validators.len() * 20);
    for v in validators {
        bytes.extend_from_slice(v.signer.as_slice());
    }
    bytes
}

/// Decodes raw bytes (multiples of 20) back into a list of addresses.
pub fn decode_validator_bytes(bytes: &[u8]) -> Vec<Address> {
    bytes
        .chunks_exact(20)
        .map(|chunk| Address::from_slice(chunk))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_validator(id: u64, addr_byte: u8) -> Validator {
        Validator {
            id,
            address: Address::new([addr_byte; 20]),
            voting_power: 100,
            signer: Address::new([addr_byte; 20]),
            proposer_priority: 0,
        }
    }

    #[test]
    fn test_span_serde_roundtrip() {
        let span = Span {
            id: 1,
            start_block: 6400,
            end_block: 12799,
            validator_set: ValidatorSet {
                validators: vec![sample_validator(1, 0xaa)],
                proposer: Some(sample_validator(1, 0xaa)),
            },
            selected_producers: vec![sample_validator(1, 0xaa)],
            bor_chain_id: "137".to_string(),
        };

        let json = serde_json::to_string(&span).unwrap();
        let deserialized: Span = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, span.id);
        assert_eq!(deserialized.start_block, span.start_block);
        assert_eq!(deserialized.end_block, span.end_block);
        assert_eq!(deserialized.bor_chain_id, span.bor_chain_id);
        assert_eq!(deserialized.selected_producers.len(), 1);
    }

    #[test]
    fn test_validator_set_serde() {
        let vs = ValidatorSet {
            validators: vec![sample_validator(1, 0xbb), sample_validator(2, 0xcc)],
            proposer: None,
        };

        let json = serde_json::to_string(&vs).unwrap();
        let deserialized: ValidatorSet = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.validators.len(), 2);
        assert!(deserialized.proposer.is_none());
    }

    #[test]
    fn test_span_id_calculation() {
        assert_eq!(span_id_at(6400, 6400), 1);
        assert_eq!(span_id_at(0, 6400), 0);
    }

    #[test]
    fn test_validator_bytes_encoding() {
        let validators = vec![sample_validator(1, 0xaa), sample_validator(2, 0xbb)];
        let bytes = encode_validator_bytes(&validators);
        assert_eq!(bytes.len(), 40);
    }

    #[test]
    fn test_validator_bytes_decode() {
        let validators = vec![sample_validator(1, 0xaa), sample_validator(2, 0xbb)];
        let bytes = encode_validator_bytes(&validators);
        let addresses = decode_validator_bytes(&bytes);
        assert_eq!(addresses.len(), 2);
        assert_eq!(addresses[0], Address::new([0xaa; 20]));
        assert_eq!(addresses[1], Address::new([0xbb; 20]));
    }
}
