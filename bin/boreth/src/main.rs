//! Boreth — Polygon Bor execution client built on Reth.

use bor_chainspec::BorChainSpecParser;
use bor_consensus::BorConsensus;
use clap::Parser;
use reth_ethereum_cli::interface::Cli;
use reth_node_builder::{components::ConsensusBuilder, BuilderContext, node::FullNodeTypes};
use reth_node_ethereum::{EthereumAddOns, EthereumNode};
use reth_tracing::tracing::info;
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
                        .consensus(BorConsensusBuilder),
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
