use std::{convert::Infallible, fmt::Debug, sync::Arc};

use alloy_consensus::{BlockHeader, Header, Transaction, TxReceipt};
use alloy_eips::eip7685::Requests;
use alloy_evm::{
    block::{
        BlockExecutionError, BlockExecutionResult, BlockExecutor, BlockExecutorFactory,
        BlockExecutorFor, BlockValidationError, CommitChanges, ExecutableTx,
        InternalBlockExecutionError, OnStateHook,
    },
    eth::{
        receipt_builder::{ReceiptBuilder, ReceiptBuilderCtx},
        spec::{EthExecutorSpec, EthSpec},
        EthBlockExecutionCtx,
    },
    Database, EthEvmFactory, Evm, EvmEnv, EvmFactory, FromRecoveredTx, FromTxWithEncoded,
    IntoTxEnv,
};
use alloy_primitives::Log;
use alloy_primitives::{Bytes, U256};
use bor::params::BorParams;
use reth_chainspec::{ChainSpec, EthChainSpec, EthereumHardfork};
use reth_ethereum_primitives::{EthPrimitives, TransactionSigned};
use reth_evm::{ConfigureEvm, NextBlockEnvAttributes};
use reth_evm_ethereum::{
    revm_spec, revm_spec_by_timestamp_and_block_number, EthBlockAssembler, RethReceiptBuilder,
};
use reth_primitives::{Block, SealedBlock, SealedHeader};
use reth_revm::{context::BlockEnv, State};
use revm::{primitives::hardfork::SpecId, DatabaseCommit, Inspector};
use revm_context::{
    result::{ExecutionResult, ResultAndState},
    CfgEnv, TxEnv,
};

use alloy_eips::eip2718::Encodable2718;

use crate::executor::{
    constants::INITIAL_BASE_FEE, executor::BorBlockExecutorFactory, system_call::SystemCaller,
};

/// Ethereum-related EVM configuration.
#[derive(Debug, Clone)]
pub struct BorEvmConfig {
    /// Inner [`BscBlockExecutorFactory`].
    pub executor_factory:
        BorBlockExecutorFactory<RethReceiptBuilder, Arc<ChainSpec>, EthEvmFactory>,
    /// Ethereum block assembler.
    pub block_assembler: EthBlockAssembler<ChainSpec>,
}

impl BorEvmConfig {
    /// Creates a new Ethereum EVM configuration with the given chain spec.
    pub fn new(chain_spec: Arc<ChainSpec>, bor_params: Arc<BorParams>) -> Self {
        Self::ethereum(chain_spec, bor_params)
    }

    /// Creates a new Ethereum EVM configuration.
    pub fn ethereum(chain_spec: Arc<ChainSpec>, bor_params: Arc<BorParams>) -> Self {
        Self::new_with_evm_factory(chain_spec, EthEvmFactory::default(), bor_params)
    }

    /// Creates a new Ethereum EVM configuration for the ethereum mainnet.
    pub fn mainnet() -> Self {
        // Self::ethereum(MAINNET.clone())
        todo!()
    }
}

impl BorEvmConfig {
    /// Creates a new Ethereum EVM configuration with the given chain spec and EVM factory.
    pub fn new_with_evm_factory(
        chain_spec: Arc<ChainSpec>,
        evm_factory: EthEvmFactory,
        bor_params: Arc<BorParams>,
    ) -> Self {
        Self {
            block_assembler: EthBlockAssembler::new(chain_spec.clone()),
            executor_factory: BorBlockExecutorFactory::new(
                RethReceiptBuilder::default(),
                chain_spec,
                evm_factory,
                bor_params,
            ),
        }
    }

    /// Returns the chain spec associated with this configuration.
    pub const fn chain_spec(&self) -> &Arc<ChainSpec> {
        self.executor_factory.spec()
    }

    /// Sets the extra data for the block assembler.
    pub fn with_extra_data(mut self, extra_data: Bytes) -> Self {
        self.block_assembler.extra_data = extra_data;
        self
    }
}

impl ConfigureEvm for BorEvmConfig {
    type Primitives = EthPrimitives;
    type Error = Infallible;
    type NextBlockEnvCtx = NextBlockEnvAttributes;
    type BlockExecutorFactory =
        BorBlockExecutorFactory<RethReceiptBuilder, Arc<ChainSpec>, EthEvmFactory>;
    type BlockAssembler = EthBlockAssembler<ChainSpec>;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        &self.executor_factory
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        &self.block_assembler
    }

    fn evm_env(&self, header: &Header) -> EvmEnv<SpecId> {
        let spec = revm_spec(self.chain_spec(), header);
        // configure evm env based on parent block
        let cfg_env = CfgEnv::new()
            .with_chain_id(self.chain_spec().chain().id())
            .with_spec(spec);

        // TODO: confirm for prevrandao.
        let block_env = BlockEnv {
            number: header.number(),
            beneficiary: header.beneficiary(),
            timestamp: header.timestamp(),
            difficulty: header.difficulty(),
            prevrandao: None,
            gas_limit: header.gas_limit(),
            basefee: header.base_fee_per_gas().unwrap_or_default(),
            blob_excess_gas_and_price: None,
        };

        EvmEnv { cfg_env, block_env }
    }

    fn next_evm_env(
        &self,
        parent: &Header,
        attributes: &NextBlockEnvAttributes,
    ) -> Result<EvmEnv, Self::Error> {
        // ensure we're not missing any timestamp based hardforks
        let spec_id = revm_spec_by_timestamp_and_block_number(
            self.chain_spec(),
            attributes.timestamp,
            parent.number() + 1,
        );

        // configure evm env based on parent block
        let cfg = CfgEnv::new()
            .with_chain_id(self.chain_spec().chain().id())
            .with_spec(spec_id);

        let mut basefee = parent.next_block_base_fee(
            self.chain_spec()
                .base_fee_params_at_timestamp(attributes.timestamp),
        );

        let mut gas_limit = attributes.gas_limit;

        // If we are on the London fork boundary, we need to multiply the parent's gas limit by the
        // elasticity multiplier to get the new gas limit.
        if self
            .chain_spec()
            .fork(EthereumHardfork::London)
            .transitions_at_block(parent.number + 1)
        {
            let elasticity_multiplier = self
                .chain_spec()
                .base_fee_params_at_timestamp(attributes.timestamp)
                .elasticity_multiplier;

            // multiply the gas limit by the elasticity multiplier
            gas_limit *= elasticity_multiplier as u64;

            // set the base fee to the initial base fee from the EIP-1559 spec
            basefee = Some(INITIAL_BASE_FEE)
        }

        let block_env = BlockEnv {
            number: parent.number + 1,
            beneficiary: attributes.suggested_fee_recipient,
            timestamp: attributes.timestamp,
            difficulty: U256::ZERO,
            prevrandao: Some(attributes.prev_randao),
            gas_limit,
            // calculate basefee based on parent block's gas usage
            basefee: basefee.unwrap_or_default(),
            blob_excess_gas_and_price: None,
        };

        Ok((cfg, block_env).into())
    }

    fn context_for_block<'a>(&self, block: &'a SealedBlock<Block>) -> EthBlockExecutionCtx<'a> {
        EthBlockExecutionCtx {
            parent_hash: block.header().parent_hash,
            parent_beacon_block_root: None,
            ommers: &block.body().ommers,
            withdrawals: None,
        }
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader,
        attributes: Self::NextBlockEnvCtx,
    ) -> EthBlockExecutionCtx<'_> {
        EthBlockExecutionCtx {
            parent_hash: parent.hash(),
            parent_beacon_block_root: None,
            ommers: &[],
            withdrawals: None,
        }
    }
}
