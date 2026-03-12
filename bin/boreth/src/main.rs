//! Boreth — Polygon Bor execution client built on Reth.

use bor_chainspec::BorChainSpecParser;
use bor_consensus::BorConsensus;
use bor_evm::BorEvmConfig;
use bor_node::handshake::BorRlpxHandshake;
use clap::Parser;
use reth_chainspec::{EthereumHardforks, Hardforks};
use reth_ethereum_cli::interface::Cli;
use reth_evm::eth::spec::EthExecutorSpec;
use reth_network::{primitives::BasicNetworkPrimitives, NetworkHandle, NetworkManager, PeersInfo};
use reth_node_api::{PrimitivesTy, TxTy};
use reth_node_builder::{
    components::{ConsensusBuilder, ExecutorBuilder, NetworkBuilder},
    BuilderContext,
    node::{FullNodeTypes, NodeTypes},
};
use reth_node_ethereum::{EthereumAddOns, EthereumNode};
use reth_tracing::tracing::info;
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use std::sync::Arc;

/// Bor PoA consensus builder that replaces Ethereum's Beacon consensus.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct BorConsensusBuilder;

impl<Node> ConsensusBuilder<Node> for BorConsensusBuilder
where
    Node: FullNodeTypes<
        Types: reth_node_builder::node::NodeTypes<
            ChainSpec: reth_chainspec::EthChainSpec + reth_chainspec::EthereumHardforks,
            Primitives = reth_ethereum_primitives::EthPrimitives,
        >,
    >,
{
    type Consensus = Arc<BorConsensus<<Node::Types as reth_node_builder::node::NodeTypes>::ChainSpec>>;

    async fn build_consensus(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Consensus> {
        Ok(Arc::new(BorConsensus::new(ctx.chain_spec())))
    }
}

/// Bor network builder with custom eth/69 handshake.
///
/// Go-Bor's eth/69 Status message includes a TD field that standard eth/69
/// omits. This builder wires in [`BorRlpxHandshake`] to handle both formats.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct BorNetworkBuilder;

impl<Node, Pool> NetworkBuilder<Node, Pool> for BorNetworkBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec: Hardforks>>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>>
        + Unpin
        + 'static,
{
    type Network =
        NetworkHandle<BasicNetworkPrimitives<PrimitivesTy<Node::Types>, reth_transaction_pool::PoolPooledTx<Pool>>>;

    async fn build_network(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::Network> {
        let network_config_builder = ctx
            .network_config_builder()?
            .eth_rlpx_handshake(Arc::new(BorRlpxHandshake::default()));

        let network_config = ctx.build_network_config(network_config_builder);
        let network = NetworkManager::builder(network_config).await?;
        let handle = ctx.start_network(network, pool);
        info!(target: "boreth", enode=%handle.local_node_record(), "P2P networking initialized with Bor handshake");
        Ok(handle)
    }
}

/// Bor EVM executor builder that wires in the custom [`BorEvmConfig`].
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct BorExecutorBuilder;

impl<Types, Node> ExecutorBuilder<Node> for BorExecutorBuilder
where
    Types: reth_node_builder::node::NodeTypes<
        ChainSpec: EthExecutorSpec + EthereumHardforks + Clone,
        Primitives = reth_ethereum_primitives::EthPrimitives,
    >,
    Node: FullNodeTypes<Types = Types>,
{
    type EVM = BorEvmConfig<Types::ChainSpec>;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        Ok(BorEvmConfig::new(ctx.chain_spec()))
    }
}

fn main() {
    reth_cli_util::sigsegv_handler::install();

    if std::env::var_os("RUST_BACKTRACE").is_none() {
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }

    if let Err(err) =
        Cli::<BorChainSpecParser>::parse().run(async move |builder, _| {
            info!(target: "boreth", "Launching Boreth node with Bor PoA consensus");
            let handle = builder
                .with_types::<EthereumNode>()
                .with_components(
                    EthereumNode::components()
                        .consensus(BorConsensusBuilder)
                        .executor(BorExecutorBuilder)
                        .network(BorNetworkBuilder),
                )
                .with_add_ons(EthereumAddOns::default())
                .launch_with_debug_capabilities()
                .await?;

            handle.wait_for_node_exit().await
        })
    {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
