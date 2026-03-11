//! ExtraData parsing for Bor consensus headers.
//!
//! Bor header extra data layout:
//! `[vanity: 32 bytes] [validator_bytes: N * 20 bytes] [seal: 65 bytes]`

use alloy_primitives::Address;
use bor_chainspec::constants::{EXTRADATA_SEAL_LEN, EXTRADATA_VANITY_LEN};

/// Minimum extra data length: 32 bytes vanity + 65 bytes seal.
const MIN_EXTRA_DATA_LEN: usize = EXTRADATA_VANITY_LEN + EXTRADATA_SEAL_LEN;

/// Address size in bytes.
const ADDRESS_LEN: usize = 20;

/// Errors that can occur when parsing extra data.
#[derive(Debug, thiserror::Error)]
pub enum ExtraDataError {
    /// Extra data is too short (must be at least 97 bytes).
    #[error("extra data too short: {0} bytes, minimum is {MIN_EXTRA_DATA_LEN}")]
    TooShort(usize),

    /// Validator bytes length is not a multiple of 20.
    #[error("validator bytes length {0} is not a multiple of {ADDRESS_LEN}")]
    InvalidValidatorBytes(usize),
}

/// Parsed extra data from a Bor consensus header.
#[derive(Debug, Clone)]
pub struct ExtraData {
    /// 32-byte vanity field (arbitrary proposer data).
    pub vanity: [u8; EXTRADATA_VANITY_LEN],
    /// Raw validator bytes (N * 20 bytes, present only at sprint-end blocks).
    pub validator_bytes: Vec<u8>,
    /// 65-byte ECDSA signature (recovery-id ++ r ++ s).
    pub seal: Vec<u8>,
}

impl ExtraData {
    /// Parse extra data from raw header bytes.
    pub fn parse(extra: &[u8]) -> Result<Self, ExtraDataError> {
        if extra.len() < MIN_EXTRA_DATA_LEN {
            return Err(ExtraDataError::TooShort(extra.len()));
        }

        let vanity: [u8; EXTRADATA_VANITY_LEN] =
            extra[..EXTRADATA_VANITY_LEN].try_into().expect("vanity slice is exactly 32 bytes");

        let validator_bytes_len = extra.len() - EXTRADATA_VANITY_LEN - EXTRADATA_SEAL_LEN;
        if validator_bytes_len % ADDRESS_LEN != 0 {
            return Err(ExtraDataError::InvalidValidatorBytes(validator_bytes_len));
        }

        let validator_bytes =
            extra[EXTRADATA_VANITY_LEN..EXTRADATA_VANITY_LEN + validator_bytes_len].to_vec();

        let seal = extra[extra.len() - EXTRADATA_SEAL_LEN..].to_vec();

        Ok(Self { vanity, validator_bytes, seal })
    }

    /// Extract validator addresses from the validator bytes.
    pub fn validators(&self) -> Vec<Address> {
        self.validator_bytes
            .chunks_exact(ADDRESS_LEN)
            .map(|chunk| Address::from_slice(chunk))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_extradata() {
        // 32 vanity + 0 validator bytes + 65 seal = 97 bytes
        let mut data = vec![0u8; MIN_EXTRA_DATA_LEN];
        // Fill vanity with 0x01
        data[..EXTRADATA_VANITY_LEN].fill(0x01);
        // Fill seal with 0xff
        data[EXTRADATA_VANITY_LEN..].fill(0xff);

        let extra = ExtraData::parse(&data).unwrap();
        assert_eq!(extra.vanity, [0x01; EXTRADATA_VANITY_LEN]);
        assert!(extra.validator_bytes.is_empty());
        assert_eq!(extra.seal.len(), EXTRADATA_SEAL_LEN);
        assert!(extra.seal.iter().all(|&b| b == 0xff));
        assert!(extra.validators().is_empty());
    }

    #[test]
    fn test_parse_with_validators() {
        // 32 vanity + 40 validator bytes (2 validators) + 65 seal = 137 bytes
        let total_len = MIN_EXTRA_DATA_LEN + 2 * ADDRESS_LEN;
        let mut data = vec![0u8; total_len];

        // Vanity
        data[..EXTRADATA_VANITY_LEN].fill(0x00);
        // Validator 1: 0xaa repeated
        data[EXTRADATA_VANITY_LEN..EXTRADATA_VANITY_LEN + ADDRESS_LEN].fill(0xaa);
        // Validator 2: 0xbb repeated
        data[EXTRADATA_VANITY_LEN + ADDRESS_LEN..EXTRADATA_VANITY_LEN + 2 * ADDRESS_LEN]
            .fill(0xbb);
        // Seal
        data[total_len - EXTRADATA_SEAL_LEN..].fill(0xcc);

        let extra = ExtraData::parse(&data).unwrap();
        assert_eq!(extra.validator_bytes.len(), 40);
        assert_eq!(extra.seal.len(), EXTRADATA_SEAL_LEN);

        let validators = extra.validators();
        assert_eq!(validators.len(), 2);
        assert_eq!(validators[0], Address::new([0xaa; 20]));
        assert_eq!(validators[1], Address::new([0xbb; 20]));
    }

    #[test]
    fn test_reject_short_extradata() {
        // Less than 97 bytes should fail
        let data = vec![0u8; MIN_EXTRA_DATA_LEN - 1];
        let result = ExtraData::parse(&data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExtraDataError::TooShort(96)));
    }

    #[test]
    fn test_validator_extraction() {
        // 3 validators
        let total_len = MIN_EXTRA_DATA_LEN + 3 * ADDRESS_LEN;
        let mut data = vec![0u8; total_len];

        // Set distinct validator addresses
        for i in 0..3 {
            let offset = EXTRADATA_VANITY_LEN + i * ADDRESS_LEN;
            data[offset..offset + ADDRESS_LEN].fill((i + 1) as u8);
        }
        // Seal
        data[total_len - EXTRADATA_SEAL_LEN..].fill(0xff);

        let extra = ExtraData::parse(&data).unwrap();
        let validators = extra.validators();
        assert_eq!(validators.len(), 3);
        assert_eq!(validators[0], Address::new([0x01; 20]));
        assert_eq!(validators[1], Address::new([0x02; 20]));
        assert_eq!(validators[2], Address::new([0x03; 20]));
    }
}
