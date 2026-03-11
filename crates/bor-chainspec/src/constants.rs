//! Bor chain constants and well-known addresses.

use alloy_primitives::{Address, address};

/// System address used for system transactions (2^160 - 2).
pub const SYSTEM_ADDRESS: Address = address!("fffffffffffffffffffffffffffffffffffffffe");

/// Bor validator set contract address on the Bor chain.
pub const BOR_VALIDATOR_SET_ADDRESS: Address = address!("0000000000000000000000000000000000001000");

/// State receiver contract address on the Bor chain.
pub const STATE_RECEIVER_ADDRESS: Address = address!("0000000000000000000000000000000000001001");

/// Polygon PoS mainnet chain ID.
pub const MAINNET_CHAIN_ID: u64 = 137;

/// Polygon Amoy testnet chain ID.
pub const AMOY_CHAIN_ID: u64 = 80002;

/// Default sprint size (number of blocks per sprint).
pub const SPRINT_SIZE: u64 = 16;

/// Default block period in seconds.
pub const BLOCK_PERIOD: u64 = 2;

/// Length of the vanity portion of extra data (bytes).
pub const EXTRADATA_VANITY_LEN: usize = 32;

/// Length of the seal (signature) portion of extra data (bytes).
pub const EXTRADATA_SEAL_LEN: usize = 65;

/// State sync delay in seconds (post-Indore hard fork).
pub const STATE_SYNC_DELAY: u64 = 128;

/// Validator/producer timeout in seconds.
pub const VALIDATOR_PRODUCER_TIMEOUT: u64 = 8;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn test_system_address() {
        assert_eq!(SYSTEM_ADDRESS, address!("fffffffffffffffffffffffffffffffffffffffe"));
    }

    #[test]
    fn test_validator_contract() {
        assert_eq!(
            BOR_VALIDATOR_SET_ADDRESS,
            address!("0000000000000000000000000000000000001000")
        );
    }

    #[test]
    fn test_state_receiver() {
        assert_eq!(
            STATE_RECEIVER_ADDRESS,
            address!("0000000000000000000000000000000000001001")
        );
    }

    #[test]
    fn test_chain_ids() {
        assert_eq!(MAINNET_CHAIN_ID, 137);
        assert_eq!(AMOY_CHAIN_ID, 80002);
    }
}
