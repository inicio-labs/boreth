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

    /// Returns all unique, non-zero fork block numbers from Ethereum hardforks only,
    /// sorted in ascending order. Used for fork ID and fork filter computation.
    ///
    /// Bor-specific forks are intentionally excluded because Go-Bor's `NewID()` only
    /// includes standard Ethereum forks in the EIP-2124 fork ID checksum. Including
    /// Bor forks would produce a different fork hash than Go-Bor peers advertise.
    fn eth_fork_blocks(&self) -> Vec<u64> {
        let mut blocks: Vec<u64> = Vec::new();

        // Collect block-based forks from inner Ethereum hardforks only
        for (_, cond) in self.inner.forks_iter() {
            match cond {
                ForkCondition::Block(block) | ForkCondition::TTD { fork_block: Some(block), .. } => {
                    blocks.push(block);
                }
                _ => {}
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

        for block in self.eth_fork_blocks() {
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
        // Since fork ID only includes Ethereum forks (not Bor forks),
        // find the highest Ethereum fork block.
        let eth_blocks = self.eth_fork_blocks();
        if let Some(&last_block) = eth_blocks.last() {
            self.compute_fork_id(&Head { number: last_block, ..Default::default() })
        } else {
            self.compute_fork_id(&Head::default())
        }
    }

    fn fork_filter(&self, head: Head) -> ForkFilter {
        let genesis_hash = self.inner.genesis_hash();
        let genesis_timestamp = self.inner.genesis().timestamp;

        // Collect all fork keys from both Ethereum and Bor hardforks
        let forks = self.eth_fork_blocks().into_iter().map(ForkFilterKey::Block);

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

    use reth_ethereum_forks::{ChainHardforks, EthereumHardfork};

    /// Build a test mainnet spec with London at block 29_231_616 (like real Polygon mainnet).
    fn mainnet_spec() -> BorChainSpec {
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
            (Box::new(EthereumHardfork::Berlin), ForkCondition::Block(29_231_616)),
            (Box::new(EthereumHardfork::London), ForkCondition::Block(29_231_616)),
        ]);
        let inner = ChainSpecBuilder::default()
            .chain(Chain::from_id(137))
            .genesis(Genesis::default())
            .with_forks(hardforks)
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
        assert!(spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 38_189_057));
        assert!(!spec.is_bor_fork_active_at_block(BorHardfork::Delhi, 38_189_055));
    }

    #[test]
    fn test_hardfork_order_invariant() {
        let spec = mainnet_spec();
        let mut prev_block: Option<u64> = None;
        for (fork, condition) in &spec.bor_hardforks {
            if let ForkCondition::Block(block) = condition {
                if let Some(prev) = prev_block {
                    assert!(*block >= prev);
                }
                prev_block = Some(*block);
            }
        }
    }

    #[test]
    fn test_fork_id_excludes_bor_hardforks() {
        let spec = mainnet_spec();

        // Fork ID at genesis: next should be London block (29_231_616)
        let id_at_0 = spec.fork_id(&Head { number: 0, ..Default::default() });
        assert_eq!(id_at_0.next, 29_231_616, "next should be London block");

        // Fork ID after all Ethereum forks should have next=0
        let id_all = spec.fork_id(&Head { number: 100_000_000, ..Default::default() });
        assert_eq!(id_all.next, 0);

        // Fork ID should match inner (Bor forks excluded)
        let head = Head { number: 38_189_056, ..Default::default() };
        let bor_id = spec.fork_id(&head);
        let inner_id = spec.inner.fork_id(&head);
        assert_eq!(bor_id.hash, inner_id.hash);
    }

    #[test]
    fn test_fork_filter_at_genesis() {
        let spec = mainnet_spec();
        let head = Head { number: 0, ..Default::default() };
        let filter = spec.fork_filter(head);
        let id = filter.current();
        assert_eq!(id.next, 29_231_616, "fork filter next should be London block");
    }
}
