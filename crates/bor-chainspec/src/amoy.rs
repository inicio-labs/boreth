//! Amoy testnet (chain 80002) genesis configuration.

use std::collections::BTreeMap;

use alloy_chains::Chain;
use alloy_genesis::Genesis;
use reth_chainspec::ChainSpecBuilder;
use reth_ethereum_forks::ForkCondition;

use crate::{BorHardfork, chainspec::BorChainSpec, constants::AMOY_CHAIN_ID};

/// Build a complete Amoy testnet [`BorChainSpec`] with chain ID 80002.
///
/// This creates the full chain specification including:
/// - Chain ID 80002
/// - Ethereum fork activations (all activated at genesis for the testnet)
/// - All Bor-specific hardfork activations at their Amoy block numbers
pub fn bor_amoy_genesis() -> BorChainSpec {
    let inner = ChainSpecBuilder::default()
        .chain(Chain::from_id(AMOY_CHAIN_ID))
        .genesis(Genesis::default())
        .london_activated()
        .paris_activated()
        .shanghai_activated()
        .cancun_activated()
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
    fn test_amoy_all_bor_forks_active_at_genesis() {
        let spec = bor_amoy_genesis();
        // All Amoy hardforks activate at block 0
        for fork in BorHardfork::all() {
            assert!(
                spec.is_bor_fork_active_at_block(*fork, 0),
                "Bor hardfork {fork} should be active at block 0 on Amoy"
            );
        }
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
