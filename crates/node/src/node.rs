//! Ethereum Node types config.

use alloy_eips::{eip7840::BlobParams, merge::EPOCH_SLOTS};
use reth_chainspec::{ChainSpec, EthChainSpec, EthereumHardforks};
use reth_consensus::{ConsensusError, FullConsensus};
use reth_ethereum_consensus::EthBeaconConsensus;
use reth_ethereum_engine_primitives::{
    EthBuiltPayload, EthPayloadAttributes, EthPayloadBuilderAttributes,
};
use reth_ethereum_primitives::{EthPrimitives, PooledTransaction, TransactionSigned};
use reth_evm::{ConfigureEvm, EvmFactory, EvmFactoryFor, NextBlockEnvAttributes};
use reth_network::{EthNetworkPrimitives, NetworkHandle, PeersInfo};
use reth_node_api::{AddOnsContext, FullNodeComponents, NodeAddOns, NodePrimitives, TxTy};
use reth_node_builder::{
    components::{
        BasicPayloadServiceBuilder, ComponentsBuilder, ConsensusBuilder, ExecutorBuilder,
        NetworkBuilder, PoolBuilder,
    },
    node::{FullNodeTypes, NodeTypes},
    rpc::{
        EngineValidatorAddOn, EngineValidatorBuilder, EthApiBuilder, EthApiCtx, RethRpcAddOns,
        RpcAddOns, RpcHandle,
    },
    BuilderContext, DebugNode, Node, NodeAdapter, NodeComponentsBuilder, PayloadBuilderConfig,
    PayloadTypes,
};
use reth_node_ethereum::{
    node::{
        EthereumAddOns, EthereumConsensusBuilder, EthereumExecutorBuilder, EthereumNetworkBuilder,
        EthereumPayloadBuilder, EthereumPoolBuilder,
    },
    EthEngineTypes,
};
use reth_provider::{providers::ProviderFactoryBuilder, CanonStateSubscriptions, EthStorage};
use reth_rpc::{eth::core::EthApiFor, ValidationApi};
use reth_rpc_api::{eth::FullEthApiServer, servers::BlockSubmissionValidationApiServer};
use reth_rpc_builder::config::RethRpcServerConfig;
use reth_rpc_eth_types::{error::FromEvmError, EthApiError};
use reth_rpc_server_types::RethRpcModule;
use reth_tracing::tracing::{debug, info};
use reth_transaction_pool::{
    blobstore::{DiskFileBlobStore, DiskFileBlobStoreConfig},
    EthTransactionPool, PoolTransaction, TransactionPool, TransactionValidationTaskExecutor,
};
use reth_trie_db::MerklePatriciaTrie;
use revm::context::TxEnv;
use std::{default::Default, sync::Arc, time::SystemTime};

/// Type configuration for a regular Odyssey node.
#[derive(Debug, Clone, Default)]
pub struct BorNode {}

impl BorNode {
    /// Creates a new instance of the Optimism node type.
    pub fn new() -> Self {
        Self {}
    }

    /// Returns a [`ComponentsBuilder`] configured for a regular Ethereum node.
    pub fn components<Node>() -> ComponentsBuilder<
        Node,
        EthereumPoolBuilder,
        BasicPayloadServiceBuilder<EthereumPayloadBuilder>,
        EthereumNetworkBuilder,
        EthereumExecutorBuilder,
        EthereumConsensusBuilder,
    >
    where
        Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
        <Node::Types as NodeTypes>::Payload: PayloadTypes<
            BuiltPayload = EthBuiltPayload,
            PayloadAttributes = EthPayloadAttributes,
            PayloadBuilderAttributes = EthPayloadBuilderAttributes,
        >,
    {
        ComponentsBuilder::default()
            .node_types::<Node>()
            .pool(EthereumPoolBuilder::default())
            .executor(EthereumExecutorBuilder::default())
            .payload(BasicPayloadServiceBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .consensus(EthereumConsensusBuilder::default())
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
        EthereumExecutorBuilder,
        EthereumConsensusBuilder,
    >;

    type AddOns = ();

    fn components_builder(&self) -> Self::ComponentsBuilder {
        Self::components()
    }

    fn add_ons(&self) -> Self::AddOns {}
}
