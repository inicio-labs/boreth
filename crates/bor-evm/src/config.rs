//! Bor EVM configuration with fork-aware precompile sets.

use alloy_primitives::{Address, address};
use bor_chainspec::BorHardfork;

/// P256VERIFY precompile address, added at the Napoli hardfork.
pub const P256_VERIFY_ADDRESS: Address = address!("0000000000000000000000000000000000000100");

/// KZG point evaluation precompile address (never active on Bor).
const KZG_ADDRESS: Address = address!("000000000000000000000000000000000000000a");

/// BorEvmConfig holds chain spec for fork-aware EVM configuration.
pub struct BorEvmConfig;

/// Returns the set of active precompile addresses at a given mainnet block number.
///
/// Standard precompiles (0x01-0x09) are always present.
/// Post-Napoli: adds P256VERIFY at 0x100.
/// KZG (0x0a) is never included on Bor.
pub fn bor_precompile_addresses(block: u64) -> Vec<Address> {
    let mut addrs: Vec<Address> = (1u64..=9)
        .map(|i| {
            let mut bytes = [0u8; 20];
            bytes[19] = i as u8;
            Address::from(bytes)
        })
        .collect();

    if block >= BorHardfork::Napoli.mainnet_block() {
        addrs.push(P256_VERIFY_ADDRESS);
    }

    addrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precompiles_pre_napoli() {
        // Block before Napoli activation should have exactly 9 standard precompiles.
        let block = BorHardfork::Napoli.mainnet_block() - 1;
        let addrs = bor_precompile_addresses(block);
        assert_eq!(addrs.len(), 9);
        for i in 1u64..=9 {
            let mut bytes = [0u8; 20];
            bytes[19] = i as u8;
            assert!(addrs.contains(&Address::from(bytes)));
        }
    }

    #[test]
    fn test_precompiles_post_napoli() {
        // Block at Napoli activation should have 10 precompiles (9 standard + P256VERIFY).
        let block = BorHardfork::Napoli.mainnet_block();
        let addrs = bor_precompile_addresses(block);
        assert_eq!(addrs.len(), 10);
        assert!(addrs.contains(&P256_VERIFY_ADDRESS));
    }

    #[test]
    fn test_no_kzg_precompile() {
        // KZG (0x0a) must never be present, regardless of block number.
        let pre = bor_precompile_addresses(0);
        assert!(!pre.contains(&KZG_ADDRESS));

        let post = bor_precompile_addresses(BorHardfork::Napoli.mainnet_block());
        assert!(!post.contains(&KZG_ADDRESS));

        let far_future = bor_precompile_addresses(u64::MAX);
        assert!(!far_future.contains(&KZG_ADDRESS));
    }
}
