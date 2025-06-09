pub mod config;
pub mod system_call;

use reth_node_api::FullNodeTypes;
use reth_node_builder::{components::ExecutorBuilder, BuilderContext};

use crate::{executor::config::BorEvmConfig, node::BorNode};

/// A regular ethereum evm and executor builder.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct BorExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for BorExecutorBuilder
where
    Node: FullNodeTypes<Types = BorNode>,
{
    type EVM = BorEvmConfig;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        let evm_config = BorEvmConfig::new(ctx.chain_spec());
        Ok(evm_config)
    }
}
