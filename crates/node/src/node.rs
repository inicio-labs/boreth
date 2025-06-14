//! Ethereum Node types config.

use bor::params::BorParams;
use reth_chainspec::ChainSpec;

use reth_ethereum_primitives::EthPrimitives;
use reth_evm::{ConfigureEvm, NextBlockEnvAttributes};
use reth_node_ethereum::engine::EthPayloadAttributes;

use reth_node_ethereum::{
    node::{EthereumNetworkBuilder, EthereumPayloadBuilder},
    EthEngineTypes,
};

use reth_payload_primitives::PayloadTypes;
use reth_provider::EthStorage;

use reth::{
    api::{FullNodeComponents, FullNodeTypes, NodeTypes},
    builder::{
        components::{
            BasicPayloadServiceBuilder, ComponentsBuilder, ExecutorBuilder, NodeComponentsBuilder,
        },
        rpc::RpcAddOns,
        DebugNode, Node, NodeAdapter,
    },
    payload::{EthBuiltPayload, EthPayloadBuilderAttributes},
};

use reth_node_ethereum::node::EthereumPoolBuilder;

use reth_trie_db::MerklePatriciaTrie;
use std::default::Default;
use std::sync::Arc;

use crate::consensus::consensus::BorConsensusBuilder;
use crate::executor::BorExecutorBuilder;

use reth::rpc::eth::EthApi;

/// Type configuration for a regular Odyssey node.
#[derive(Debug, Clone)]
pub struct BorNode {
    bor_params: Arc<BorParams>,
}

impl BorNode {
    /// Creates a new instance of the Optimism node type.
    pub fn new(bor_params: Arc<BorParams>) -> Self {
        Self { bor_params }
    }

    /// Returns a [`ComponentsBuilder`] configured for a regular Ethereum node.
    pub fn components<Node>(
        &self,
    ) -> ComponentsBuilder<
        Node,
        EthereumPoolBuilder,
        BasicPayloadServiceBuilder<EthereumPayloadBuilder>,
        EthereumNetworkBuilder,
        BorExecutorBuilder,
        BorConsensusBuilder,
    >
    where
        Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
        <Node::Types as NodeTypes>::Payload: PayloadTypes<
            BuiltPayload = EthBuiltPayload,
            PayloadAttributes = EthPayloadAttributes,
            PayloadBuilderAttributes = EthPayloadBuilderAttributes,
        >,
        BorExecutorBuilder: ExecutorBuilder<Node>,
        <BorExecutorBuilder as ExecutorBuilder<Node>>::EVM:
            ConfigureEvm<NextBlockEnvCtx = NextBlockEnvAttributes>,
    {
        ComponentsBuilder::default()
            .node_types::<Node>()
            .pool(EthereumPoolBuilder::default())
            .executor(BorExecutorBuilder {
                bor_params: self.bor_params.clone(),
            })
            .payload(BasicPayloadServiceBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .consensus(BorConsensusBuilder {
                bor_params: self.bor_params.clone(),
            })
    }
}

//
impl NodeTypes for BorNode {
    type Primitives = EthPrimitives;
    type ChainSpec = ChainSpec;
    type StateCommitment = MerklePatriciaTrie;
    type Storage = EthStorage;
    type Payload = EthEngineTypes;
}

impl<N> Node<N> for BorNode
where
    N: FullNodeTypes<Types = Self>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        BasicPayloadServiceBuilder<EthereumPayloadBuilder>,
        EthereumNetworkBuilder,
        BorExecutorBuilder,
        BorConsensusBuilder,
    >;

    type AddOns = ();

    fn components_builder(&self) -> Self::ComponentsBuilder {
        Self::components(self)
    }

    fn add_ons(&self) -> Self::AddOns {}
}
