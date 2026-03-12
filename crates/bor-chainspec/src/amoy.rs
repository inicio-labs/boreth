//! Amoy testnet (chain 80002) genesis configuration.

use std::collections::BTreeMap;

use alloy_chains::Chain;
use alloy_genesis::Genesis;
use reth_chainspec::ChainSpecBuilder;
use reth_ethereum_forks::{ChainHardforks, EthereumHardfork, ForkCondition};

use crate::{BorHardfork, chainspec::BorChainSpec, constants::AMOY_CHAIN_ID};

// Amoy Ethereum fork activation blocks (from Go-Bor AmoyChainConfig).
const AMOY_LONDON_BLOCK: u64 = 73_100;
const AMOY_SHANGHAI_BLOCK: u64 = 73_100;
const AMOY_CANCUN_BLOCK: u64 = 5_423_600;
const AMOY_PRAGUE_BLOCK: u64 = 22_765_056;

/// Embedded Amoy genesis JSON (header fields + alloc from Go-Bor's genesis-amoy.json).
const AMOY_GENESIS_JSON: &str = include_str!("../res/amoy_genesis.json");

/// Build a complete Amoy testnet [`BorChainSpec`] with chain ID 80002.
///
/// Fork schedule matches Go-Bor's `AmoyChainConfig`. Note that on Polygon/Bor,
/// post-merge forks (Shanghai, Cancun, Prague) are block-based, not timestamp-based.
pub fn bor_amoy_genesis() -> BorChainSpec {
    let genesis: Genesis =
        serde_json::from_str(AMOY_GENESIS_JSON).expect("valid embedded Amoy genesis JSON");

    let hardforks = ChainHardforks::new(vec![
        (Box::new(EthereumHardfork::Frontier) as Box<dyn reth_ethereum_forks::Hardfork>, ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::Homestead), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::Tangerine), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::SpuriousDragon), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::Byzantium), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::Constantinople), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::Petersburg), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::Istanbul), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::MuirGlacier), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::Berlin), ForkCondition::Block(0)),
        (Box::new(EthereumHardfork::London), ForkCondition::Block(AMOY_LONDON_BLOCK)),
        (Box::new(EthereumHardfork::Paris), ForkCondition::TTD {
            total_difficulty: alloy_primitives::U256::ZERO,
            fork_block: None,
            activation_block_number: 0,
        }),
        (Box::new(EthereumHardfork::Shanghai), ForkCondition::Block(AMOY_SHANGHAI_BLOCK)),
        (Box::new(EthereumHardfork::Cancun), ForkCondition::Block(AMOY_CANCUN_BLOCK)),
        (Box::new(EthereumHardfork::Prague), ForkCondition::Block(AMOY_PRAGUE_BLOCK)),
    ]);

    let inner = ChainSpecBuilder::default()
        .chain(Chain::from_id(AMOY_CHAIN_ID))
        .genesis(genesis)
        .with_forks(hardforks)
        .build();

    let mut bor_hardforks = BTreeMap::new();
    for fork in BorHardfork::all() {
        bor_hardforks.insert(*fork, ForkCondition::Block(fork.amoy_block()));
    }

    BorChainSpec::new(inner, bor_hardforks)
}

#[cfg(test)]
mod tests {
    use reth_chainspec::EthChainSpec;

    use super::*;

    #[test]
    fn test_amoy_chain_id() {
        let spec = bor_amoy_genesis();
        assert_eq!(spec.chain_id(), 80002);
    }

    #[test]
    fn test_amoy_genesis_hash_not_default() {
        let spec = bor_amoy_genesis();
        // Should NOT be the default empty genesis hash
        assert_ne!(
            spec.genesis_hash(),
            alloy_primitives::B256::ZERO,
            "Genesis hash should not be zero"
        );
    }

    #[test]
    fn test_amoy_genesis_has_alloc() {
        let spec = bor_amoy_genesis();
        assert!(
            !spec.genesis().alloc.is_empty(),
            "Amoy genesis should have alloc entries"
        );
    }

    #[test]
    fn test_amoy_delhi_active_at_73100() {
        let spec = bor_amoy_genesis();
        assert!(spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 73_100));
        assert!(!spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 73_099));
    }

    #[test]
    fn test_amoy_ahmedabad_active_at_block() {
        let spec = bor_amoy_genesis();
        assert!(spec.is_bor_fork_active_at_block(BorHardfork::Ahmedabad, 11_865_856));
        assert!(!spec.is_bor_fork_active_at_block(BorHardfork::Ahmedabad, 11_865_855));
    }

    #[test]
    fn test_amoy_bor_hardfork_count() {
        let spec = bor_amoy_genesis();
        assert_eq!(
            spec.bor_hardforks().len(),
            BorHardfork::all().len(),
            "All Bor hardforks should be present in the Amoy spec"
        );
    }
}
