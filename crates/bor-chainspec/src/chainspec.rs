//! Bor chain specification — wraps reth's [`ChainSpec`] with Polygon-specific hardforks.

use std::collections::BTreeMap;
use std::fmt::Debug;

use alloy_chains::Chain;
use alloy_genesis::Genesis;
use alloy_primitives::B256;
use reth_chainspec::{ChainSpec, EthChainSpec};
use reth_ethereum_forks::{
    EthereumHardfork, EthereumHardforks, ForkCondition, ForkFilter, ForkFilterKey, ForkHash,
    ForkId, Hardfork, Hardforks, Head,
};

use crate::BorHardfork;

/// Polygon Bor chain specification.
///
/// Wraps the base Ethereum [`ChainSpec`] (which tracks standard Ethereum hardforks)
/// and adds Polygon-specific hardfork tracking via [`BorHardfork`].
#[derive(Debug, Clone)]
pub struct BorChainSpec {
    /// Base Ethereum chain spec (chain ID, genesis, ETH hardforks).
    inner: ChainSpec,
    /// Polygon-specific hardfork activation conditions, ordered by hardfork.
    bor_hardforks: BTreeMap<BorHardfork, ForkCondition>,
}

impl BorChainSpec {
    /// Create a new `BorChainSpec` from an inner [`ChainSpec`] and Bor hardfork map.
    ///
    /// # Panics
    ///
    /// Panics if the hardforks are not in ascending activation order.
    pub fn new(inner: ChainSpec, bor_hardforks: BTreeMap<BorHardfork, ForkCondition>) -> Self {
        // Validate hardfork ordering — block-activated forks must be in ascending order.
        let mut prev_block: Option<u64> = None;
        for (fork, condition) in &bor_hardforks {
            if let ForkCondition::Block(block) = condition {
                if let Some(prev) = prev_block {
                    assert!(
                        *block >= prev,
                        "Bor hardfork {fork} activates at block {block} which is before previous fork at block {prev}"
                    );
                }
                prev_block = Some(*block);
            }
        }

        Self { inner, bor_hardforks }
    }

    /// Returns a reference to the inner Ethereum [`ChainSpec`].
    pub fn inner(&self) -> &ChainSpec {
        &self.inner
    }

    /// Consumes self and returns the inner Ethereum [`ChainSpec`].
    pub fn into_inner(self) -> ChainSpec {
        self.inner
    }

    /// Returns a reference to the Bor hardfork map.
    pub fn bor_hardforks(&self) -> &BTreeMap<BorHardfork, ForkCondition> {
        &self.bor_hardforks
    }

    /// Returns all unique, non-zero fork block numbers from both Ethereum and Bor hardforks,
    /// sorted in ascending order. This is used for fork ID and fork filter computation.
    fn all_fork_blocks(&self) -> Vec<u64> {
        let mut blocks: Vec<u64> = Vec::new();

        // Collect block-based forks from inner Ethereum hardforks
        for (_, cond) in self.inner.forks_iter() {
            match cond {
                ForkCondition::Block(block) | ForkCondition::TTD { fork_block: Some(block), .. } => {
                    blocks.push(block);
                }
                _ => {}
            }
        }

        // Collect block-based forks from Bor hardforks
        for (_, cond) in &self.bor_hardforks {
            if let ForkCondition::Block(block) = cond {
                blocks.push(*block);
            }
        }

        blocks.sort_unstable();
        blocks.dedup();
        // Remove block 0 — genesis forks don't contribute to fork hash
        blocks.retain(|&b| b != 0);
        blocks
    }

    /// Compute the fork ID for the given head, including both Ethereum and Bor hardforks.
    /// Follows EIP-6122 / EIP-2124 spec.
    fn compute_fork_id(&self, head: &Head) -> ForkId {
        let mut forkhash = ForkHash::from(self.inner.genesis_hash());

        for block in self.all_fork_blocks() {
            if head.number >= block {
                forkhash += block;
            } else {
                return ForkId { hash: forkhash, next: block };
            }
        }

        // All Bor forks are block-based, no timestamp forks to process
        ForkId { hash: forkhash, next: 0 }
    }

    /// Check if a specific Bor hardfork is active at the given block number.
    pub fn is_bor_fork_active_at_block(&self, fork: BorHardfork, block: u64) -> bool {
        self.bor_hardforks
            .get(&fork)
            .map(|condition| match condition {
                ForkCondition::Block(activation_block) => block >= *activation_block,
                ForkCondition::Never => false,
                _ => false,
            })
            .unwrap_or(false)
    }
}

// Delegate `EthChainSpec` to the inner `ChainSpec`.
impl EthChainSpec for BorChainSpec {
    type Header = <ChainSpec as EthChainSpec>::Header;

    fn chain(&self) -> Chain {
        self.inner.chain()
    }

    fn chain_id(&self) -> u64 {
        self.inner.chain_id()
    }

    fn base_fee_params_at_timestamp(
        &self,
        timestamp: u64,
    ) -> alloy_eips::eip1559::BaseFeeParams {
        self.inner.base_fee_params_at_timestamp(timestamp)
    }

    fn blob_params_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Option<alloy_eips::eip7840::BlobParams> {
        self.inner.blob_params_at_timestamp(timestamp)
    }

    fn deposit_contract(&self) -> Option<&reth_chainspec::DepositContract> {
        self.inner.deposit_contract()
    }

    fn genesis(&self) -> &Genesis {
        self.inner.genesis()
    }

    fn genesis_hash(&self) -> B256 {
        self.inner.genesis_hash()
    }

    fn genesis_header(&self) -> &Self::Header {
        self.inner.genesis_header()
    }

    fn prune_delete_limit(&self) -> usize {
        self.inner.prune_delete_limit()
    }

    fn display_hardforks(&self) -> Box<dyn core::fmt::Display> {
        Box::new(self.inner.display_hardforks())
    }

    fn bootnodes(&self) -> Option<Vec<reth_network_peers::NodeRecord>> {
        self.inner.bootnodes()
    }

    fn final_paris_total_difficulty(&self) -> Option<alloy_primitives::U256> {
        self.inner.final_paris_total_difficulty()
    }
}

// Delegate `Hardforks` to the inner `ChainSpec`.
impl Hardforks for BorChainSpec {
    fn fork<H: Hardfork>(&self, fork: H) -> ForkCondition {
        // Check Bor hardforks first by name matching
        for (bor_fork, condition) in &self.bor_hardforks {
            if bor_fork.name() == fork.name() {
                return *condition;
            }
        }
        // Fall back to inner ChainSpec for Ethereum hardforks
        self.inner.fork(fork)
    }

    fn forks_iter(&self) -> impl Iterator<Item = (&dyn Hardfork, ForkCondition)> {
        // Combine inner forks with Bor forks
        self.inner.forks_iter().chain(
            self.bor_hardforks
                .iter()
                .map(|(fork, condition)| (fork as &dyn Hardfork, *condition)),
        )
    }

    fn fork_id(&self, head: &Head) -> ForkId {
        self.compute_fork_id(head)
    }

    fn latest_fork_id(&self) -> ForkId {
        // Find the last Bor hardfork that is not `Never`
        let last_bor = self.bor_hardforks.iter().rev().find(|(_, c)| !matches!(c, ForkCondition::Never));
        let last_eth = self.inner.forks_iter().last();

        // Pick the highest activation point across both sets
        let head = match (last_bor, last_eth) {
            (Some((_, ForkCondition::Block(bor_block))), Some((_, ForkCondition::Block(eth_block)))) => {
                let block = (*bor_block).max(eth_block);
                Head { number: block, ..Default::default() }
            }
            (Some((_, ForkCondition::Block(block))), _) => {
                Head { number: *block, ..Default::default() }
            }
            _ => {
                // Fall back to inner for timestamp-based or other scenarios
                return self.inner.latest_fork_id()
            }
        };
        self.compute_fork_id(&head)
    }

    fn fork_filter(&self, head: Head) -> ForkFilter {
        let genesis_hash = self.inner.genesis_hash();
        let genesis_timestamp = self.inner.genesis().timestamp;

        // Collect all fork keys from both Ethereum and Bor hardforks
        let forks = self.all_fork_blocks().into_iter().map(ForkFilterKey::Block);

        ForkFilter::new(head, genesis_hash, genesis_timestamp, forks)
    }
}

// Delegate `EthereumHardforks` to the inner `ChainSpec`.
impl EthereumHardforks for BorChainSpec {
    fn ethereum_fork_activation(&self, fork: EthereumHardfork) -> ForkCondition {
        self.inner.ethereum_fork_activation(fork)
    }
}

/// Build a mainnet `BorChainSpec` with Polygon PoS mainnet hardforks.
pub fn bor_mainnet_chainspec(inner: ChainSpec) -> BorChainSpec {
    let mut bor_hardforks = BTreeMap::new();
    for fork in BorHardfork::all() {
        bor_hardforks.insert(*fork, ForkCondition::Block(fork.mainnet_block()));
    }
    BorChainSpec::new(inner, bor_hardforks)
}

/// Build an Amoy testnet `BorChainSpec` with Polygon Amoy hardforks.
pub fn bor_amoy_chainspec(inner: ChainSpec) -> BorChainSpec {
    let mut bor_hardforks = BTreeMap::new();
    for fork in BorHardfork::all() {
        bor_hardforks.insert(*fork, ForkCondition::Block(fork.amoy_block()));
    }
    BorChainSpec::new(inner, bor_hardforks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_chainspec::ChainSpecBuilder;

    fn mainnet_spec() -> BorChainSpec {
        let inner = ChainSpecBuilder::default()
            .chain(Chain::from_id(137))
            .genesis(Genesis::default())
            .london_activated()
            .build();
        bor_mainnet_chainspec(inner)
    }

    #[test]
    fn test_chainspec_chain_id() {
        let spec = mainnet_spec();
        assert_eq!(spec.chain_id(), 137);
    }

    #[test]
    fn test_chainspec_is_fork_active() {
        let spec = mainnet_spec();
        // Delhi activates at block 38_189_056
        assert!(
            spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 38_189_057),
            "Delhi should be active at block 38_189_057"
        );
        assert!(
            !spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 38_189_055),
            "Delhi should not be active at block 38_189_055"
        );
    }

    #[test]
    fn test_hardfork_order_invariant() {
        let spec = mainnet_spec();
        let mut prev_block: Option<u64> = None;
        for (fork, condition) in &spec.bor_hardforks {
            if let ForkCondition::Block(block) = condition {
                if let Some(prev) = prev_block {
                    assert!(
                        *block >= prev,
                        "Hardfork {fork} at block {block} is before previous at block {prev}"
                    );
                }
                prev_block = Some(*block);
            }
        }
    }

    #[test]
    fn test_fork_id_includes_bor_hardforks() {
        let spec = mainnet_spec();

        // Fork ID at genesis should only include genesis hash (no forks activated at block 0)
        let id_at_0 = spec.fork_id(&Head { number: 0, ..Default::default() });
        // The "next" should be the first non-zero fork block
        assert_ne!(id_at_0.next, 0, "should have a next fork at genesis");

        // Fork ID after all forks should have next=0
        let id_all = spec.fork_id(&Head { number: 100_000_000, ..Default::default() });
        assert_eq!(id_all.next, 0, "all forks should be active at block 100M");

        // Fork ID should change when a Bor hardfork activates
        let id_before_delhi = spec.fork_id(&Head { number: 38_189_055, ..Default::default() });
        let id_at_delhi = spec.fork_id(&Head { number: 38_189_056, ..Default::default() });
        assert_ne!(
            id_before_delhi.hash, id_at_delhi.hash,
            "fork hash should change at Delhi activation"
        );
    }

    #[test]
    fn test_fork_id_differs_from_inner_only() {
        let spec = mainnet_spec();
        // After all Ethereum forks but before first Bor fork, BorChainSpec and inner
        // should produce different fork IDs (inner doesn't know about Bor forks)
        let head = Head { number: 38_189_056, ..Default::default() };
        let bor_id = spec.fork_id(&head);
        let inner_id = spec.inner.fork_id(&head);
        // The hashes should differ because Bor adds Delhi at 38_189_056
        assert_ne!(
            bor_id.hash, inner_id.hash,
            "BorChainSpec fork ID should differ from inner-only fork ID after Delhi"
        );
    }

    #[test]
    fn test_fork_filter_includes_bor_forks() {
        let spec = mainnet_spec();
        let head = Head { number: 0, ..Default::default() };
        let filter = spec.fork_filter(head);
        // The filter should exist and be valid
        let id = filter.current();
        assert_ne!(id.next, 0, "fork filter should have upcoming forks");
    }

    #[test]
    fn test_amoy_fork_id_all_at_genesis() {
        // Amoy has all Bor forks at block 0, so they shouldn't affect the fork hash
        let inner = ChainSpecBuilder::default()
            .chain(Chain::from_id(80002))
            .genesis(Genesis::default())
            .london_activated()
            .paris_activated()
            .build();
        let spec = bor_amoy_chainspec(inner);

        // All Bor forks at block 0 are filtered out, so fork ID should match
        // what inner produces (since block-0 forks don't contribute to fork hash)
        let head = Head { number: 100_000_000, ..Default::default() };
        let bor_id = spec.fork_id(&head);
        let inner_id = spec.inner.fork_id(&head);
        assert_eq!(
            bor_id.hash, inner_id.hash,
            "Amoy: all-at-genesis Bor forks should not change fork hash"
        );
    }
}
