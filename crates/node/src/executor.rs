pub mod config;
pub mod constants;
pub mod executor;
pub mod system_call;

use bor::params::BorParams;
use std::sync::Arc;

use reth::{
    api::FullNodeTypes,
    builder::{components::ExecutorBuilder, BuilderContext},
};

use crate::{executor::config::BorEvmConfig, node::BorNode};

/// A regular ethereum evm and executor builder.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BorExecutorBuilder {
    pub bor_params: Arc<BorParams>,
}

impl<Node> ExecutorBuilder<Node> for BorExecutorBuilder
where
    Node: FullNodeTypes<Types = BorNode>,
{
    type EVM = BorEvmConfig;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        let evm_config = BorEvmConfig::new(ctx.chain_spec(), self.bor_params);
        Ok(evm_config)
    }
}
