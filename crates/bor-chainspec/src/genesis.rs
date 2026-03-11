//! Polygon mainnet genesis configuration (chain 137).

use alloy_chains::Chain;
use alloy_genesis::Genesis;
use reth_chainspec::ChainSpecBuilder;
use reth_ethereum_forks::{ChainHardforks, EthereumHardfork, ForkCondition};

use crate::chainspec::{BorChainSpec, bor_mainnet_chainspec};
use crate::BorHardfork;

/// Berlin and London activated together on Polygon PoS mainnet.
const BERLIN_BLOCK: u64 = 29_231_616;
const LONDON_BLOCK: u64 = 29_231_616;

/// Build the Polygon PoS mainnet genesis configuration (chain 137).
pub fn bor_mainnet_genesis() -> BorChainSpec {
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
        (Box::new(EthereumHardfork::Berlin), ForkCondition::Block(BERLIN_BLOCK)),
        (Box::new(EthereumHardfork::London), ForkCondition::Block(LONDON_BLOCK)),
    ]);

    let inner = ChainSpecBuilder::default()
        .chain(Chain::from_id(137))
        .genesis(Genesis::default())
        .with_forks(hardforks)
        .build();

    bor_mainnet_chainspec(inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_chainspec::EthChainSpec;
    use reth_ethereum_forks::Hardforks;

    #[test]
    fn test_mainnet_chain_id() {
        let spec = bor_mainnet_genesis();
        assert_eq!(spec.chain_id(), 137);
    }

    #[test]
    fn test_mainnet_has_all_bor_hardforks() {
        let spec = bor_mainnet_genesis();
        for fork in BorHardfork::all() {
            assert!(
                spec.bor_hardforks().contains_key(fork),
                "Missing Bor hardfork: {fork}"
            );
        }
    }

    #[test]
    fn test_mainnet_delhi_active() {
        let spec = bor_mainnet_genesis();
        assert!(spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 38_189_056));
        assert!(!spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 38_189_055));
    }

    #[test]
    fn test_mainnet_lisovo_active() {
        let spec = bor_mainnet_genesis();
        assert!(spec.is_bor_fork_active_at_block(BorHardfork::Lisovo, 83_756_500));
        assert!(!spec.is_bor_fork_active_at_block(BorHardfork::Lisovo, 83_756_499));
    }

    #[test]
    fn test_mainnet_london_block() {
        let spec = bor_mainnet_genesis();
        let london = spec.inner().fork(EthereumHardfork::London);
        assert_eq!(london, ForkCondition::Block(29_231_616));
    }
}
