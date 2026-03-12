//! Bor EVM configuration implementing Reth's [`ConfigureEvm`] trait.
//!
//! This wires the custom [`BorBlockExecutorFactory`] into Reth's execution
//! pipeline, enabling Bor-specific system calls during block finalization.

use crate::block_executor::{BorBlockExecutionCtx, BorBlockExecutorFactory, BorExecutionCtx};
use crate::build::BorBlockAssembler;
use alloy_consensus::Header;
use alloy_eips::Decodable2718;
use alloy_primitives::{Bytes, U256};
use alloy_rpc_types_engine::ExecutionData;
use core::{convert::Infallible, fmt::Debug};
use reth_chainspec::{EthChainSpec, EthereumHardforks};
use reth_ethereum_primitives::{EthPrimitives, TransactionSigned};
use reth_evm::{
    eth::{
        EthBlockExecutionCtx, EthBlockExecutorFactory,
        spec::EthExecutorSpec,
    },
    precompiles::PrecompilesMap,
    ConfigureEngineEvm, ConfigureEvm, EvmEnv, EvmEnvFor, EvmFactory, EthEvmFactory,
    ExecutableTxIterator, ExecutionCtxFor,
    FromRecoveredTx, FromTxWithEncoded,
    NextBlockEnvAttributes, TransactionEnv,
};
use reth_evm_ethereum::{revm_spec_by_timestamp_and_block_number, RethReceiptBuilder};
use reth_primitives_traits::{constants::MAX_TX_GAS_LIMIT_OSAKA, SealedBlock, SealedHeader, SignedTransaction, TxTy};
use reth_storage_errors::any::AnyError;
use revm::context::{BlockEnv, CfgEnv};
use revm::context_interface::block::BlobExcessGasAndPrice;
use revm::primitives::hardfork::SpecId;
use std::borrow::Cow;
use std::sync::Arc;

/// Bor EVM configuration for Reth.
///
/// Analogous to [`reth_evm_ethereum::EthEvmConfig`] but uses
/// [`BorBlockExecutorFactory`] which injects Bor system calls during
/// block finalization.
#[derive(Debug, Clone)]
pub struct BorEvmConfig<C = reth_chainspec::ChainSpec, EvmF = EthEvmFactory> {
    /// Inner Bor block executor factory.
    pub executor_factory: BorBlockExecutorFactory<RethReceiptBuilder, Arc<C>, EvmF>,
    /// Block assembler for Bor.
    pub block_assembler: BorBlockAssembler<C>,
    /// Chain spec.
    chain_spec: Arc<C>,
}

impl<C> BorEvmConfig<C> {
    /// Create a new Bor EVM configuration with the given chain spec.
    pub fn new(chain_spec: Arc<C>) -> Self {
        Self::new_with_evm_factory(chain_spec, EthEvmFactory::default())
    }
}

impl<C, EvmF> BorEvmConfig<C, EvmF> {
    /// Create a new Bor EVM configuration with a custom EVM factory.
    pub fn new_with_evm_factory(chain_spec: Arc<C>, evm_factory: EvmF) -> Self {
        let eth_factory = EthBlockExecutorFactory::new(
            RethReceiptBuilder::default(),
            chain_spec.clone(),
            evm_factory,
        );
        Self {
            block_assembler: BorBlockAssembler::new(chain_spec.clone()),
            executor_factory: BorBlockExecutorFactory::new(eth_factory),
            chain_spec,
        }
    }

    /// Returns the chain spec.
    pub fn chain_spec(&self) -> &Arc<C> {
        &self.chain_spec
    }
}

impl<C, EvmF> ConfigureEvm for BorEvmConfig<C, EvmF>
where
    C: EthExecutorSpec + EthChainSpec<Header = Header> + reth_chainspec::EthereumHardforks + Clone + 'static,
    EvmF: EvmFactory<
            Tx: TransactionEnv
                    + FromRecoveredTx<TransactionSigned>
                    + FromTxWithEncoded<TransactionSigned>,
            Spec = SpecId,
            BlockEnv = BlockEnv,
            Precompiles = PrecompilesMap,
        > + Clone
        + Debug
        + Send
        + Sync
        + Unpin
        + 'static,
{
    type Primitives = EthPrimitives;
    type Error = Infallible;
    type NextBlockEnvCtx = NextBlockEnvAttributes;
    type BlockExecutorFactory = BorBlockExecutorFactory<RethReceiptBuilder, Arc<C>, EvmF>;
    type BlockAssembler = BorBlockAssembler<C>;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        &self.executor_factory
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        &self.block_assembler
    }

    fn evm_env(&self, header: &Header) -> Result<EvmEnv<SpecId>, Self::Error> {
        Ok(EvmEnv::for_eth_block(
            header,
            &*self.chain_spec,
            self.chain_spec.chain().id(),
            self.chain_spec.blob_params_at_timestamp(header.timestamp),
        ))
    }

    fn next_evm_env(
        &self,
        parent: &Header,
        attributes: &NextBlockEnvAttributes,
    ) -> Result<EvmEnv, Self::Error> {
        use reth_evm::eth::NextEvmEnvAttributes;
        Ok(EvmEnv::for_eth_next_block(
            parent,
            NextEvmEnvAttributes {
                timestamp: attributes.timestamp,
                suggested_fee_recipient: attributes.suggested_fee_recipient,
                prev_randao: attributes.prev_randao,
                gas_limit: attributes.gas_limit,
            },
            self.chain_spec
                .next_block_base_fee(parent, attributes.timestamp)
                .unwrap_or_default(),
            &*self.chain_spec,
            self.chain_spec.chain().id(),
            self.chain_spec.blob_params_at_timestamp(attributes.timestamp),
        ))
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<reth_ethereum_primitives::Block>,
    ) -> Result<BorBlockExecutionCtx<'a>, Self::Error> {
        Ok(BorBlockExecutionCtx {
            eth: EthBlockExecutionCtx {
                tx_count_hint: Some(block.transaction_count()),
                parent_hash: block.header().parent_hash,
                parent_beacon_block_root: block.header().parent_beacon_block_root,
                ommers: &block.body().ommers,
                withdrawals: block.body().withdrawals.as_ref().map(Cow::Borrowed),
                extra_data: block.header().extra_data.clone(),
            },
            // System call data will be populated by the pipeline/node
            // before execution. For now, default to no-op.
            bor: BorExecutionCtx::default(),
        })
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader,
        attributes: Self::NextBlockEnvCtx,
    ) -> Result<BorBlockExecutionCtx<'_>, Self::Error> {
        Ok(BorBlockExecutionCtx {
            eth: EthBlockExecutionCtx {
                tx_count_hint: None,
                parent_hash: parent.hash(),
                parent_beacon_block_root: attributes.parent_beacon_block_root,
                ommers: &[],
                withdrawals: attributes.withdrawals.map(Cow::Owned),
                extra_data: Default::default(),
            },
            bor: BorExecutionCtx::default(),
        })
    }
}

impl<C, EvmF> ConfigureEngineEvm<ExecutionData> for BorEvmConfig<C, EvmF>
where
    C: EthExecutorSpec + EthChainSpec<Header = Header> + EthereumHardforks + Clone + 'static,
    EvmF: EvmFactory<
            Tx: TransactionEnv
                    + FromRecoveredTx<TransactionSigned>
                    + FromTxWithEncoded<TransactionSigned>,
            Spec = SpecId,
            BlockEnv = BlockEnv,
            Precompiles = PrecompilesMap,
        > + Clone
        + Debug
        + Send
        + Sync
        + Unpin
        + 'static,
{
    fn evm_env_for_payload(&self, payload: &ExecutionData) -> Result<EvmEnvFor<Self>, Self::Error> {
        let timestamp = payload.payload.timestamp();
        let block_number = payload.payload.block_number();

        let blob_params = self.chain_spec().blob_params_at_timestamp(timestamp);
        let spec =
            revm_spec_by_timestamp_and_block_number(self.chain_spec(), timestamp, block_number);

        let mut cfg_env = CfgEnv::new()
            .with_chain_id(self.chain_spec().chain().id())
            .with_spec_and_mainnet_gas_params(spec);

        if let Some(blob_params) = &blob_params {
            cfg_env.set_max_blobs_per_tx(blob_params.max_blobs_per_tx);
        }

        if self.chain_spec().is_osaka_active_at_timestamp(timestamp) {
            cfg_env.tx_gas_limit_cap = Some(MAX_TX_GAS_LIMIT_OSAKA);
        }

        let blob_excess_gas_and_price =
            payload.payload.excess_blob_gas().zip(blob_params).map(|(excess_blob_gas, params)| {
                let blob_gasprice = params.calc_blob_fee(excess_blob_gas);
                BlobExcessGasAndPrice { excess_blob_gas, blob_gasprice }
            });

        let block_env = BlockEnv {
            number: U256::from(block_number),
            beneficiary: payload.payload.fee_recipient(),
            timestamp: U256::from(timestamp),
            difficulty: if spec >= SpecId::MERGE {
                U256::ZERO
            } else {
                payload.payload.as_v1().prev_randao.into()
            },
            prevrandao: (spec >= SpecId::MERGE).then(|| payload.payload.as_v1().prev_randao),
            gas_limit: payload.payload.gas_limit(),
            basefee: payload.payload.saturated_base_fee_per_gas(),
            blob_excess_gas_and_price,
        };

        Ok(EvmEnv { cfg_env, block_env })
    }

    fn context_for_payload<'a>(
        &self,
        payload: &'a ExecutionData,
    ) -> Result<ExecutionCtxFor<'a, Self>, Self::Error> {
        Ok(BorBlockExecutionCtx {
            eth: EthBlockExecutionCtx {
                tx_count_hint: Some(payload.payload.transactions().len()),
                parent_hash: payload.parent_hash(),
                parent_beacon_block_root: payload.sidecar.parent_beacon_block_root(),
                ommers: &[],
                withdrawals: payload.payload.withdrawals().map(|w| Cow::Owned(w.clone().into())),
                extra_data: payload.payload.as_v1().extra_data.clone(),
            },
            bor: BorExecutionCtx::default(),
        })
    }

    fn tx_iterator_for_payload(
        &self,
        payload: &ExecutionData,
    ) -> Result<impl ExecutableTxIterator<Self>, Self::Error> {
        let txs = payload.payload.transactions().clone();
        let convert = |tx: Bytes| {
            let tx =
                TxTy::<Self::Primitives>::decode_2718_exact(tx.as_ref()).map_err(AnyError::new)?;
            let signer = tx.try_recover().map_err(AnyError::new)?;
            Ok::<_, AnyError>(tx.with_signer(signer))
        };

        Ok((txs, convert))
    }
}
