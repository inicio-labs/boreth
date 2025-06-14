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

use crate::executor::{constants::INITIAL_BASE_FEE, system_call::SystemCaller};

// TODO: Removing Default here.
#[derive(Debug, Clone)]
pub struct BorBlockExecutorFactory<
    R = RethReceiptBuilder,
    Spec = EthSpec,
    EvmFactory = EthEvmFactory,
> {
    /// Receipt builder.
    receipt_builder: R,
    /// Chain specification.
    spec: Spec,
    /// EVM factory.
    evm_factory: EvmFactory,

    // TODO: All the following fields can be clubed together in a single struct.
    /// Bor params.
    bor_params: Arc<BorParams>,
}

impl<R, Spec, EvmFactory> BorBlockExecutorFactory<R, Spec, EvmFactory> {
    /// Creates a new [`EthBlockExecutorFactory`] with the given spec, [`EvmFactory`], and
    /// [`ReceiptBuilder`].
    pub const fn new(
        receipt_builder: R,
        spec: Spec,
        evm_factory: EvmFactory,
        bor_params: Arc<BorParams>,
    ) -> Self {
        Self {
            receipt_builder,
            spec,
            evm_factory,
            bor_params,
        }
    }

    /// Exposes the receipt builder.
    pub const fn receipt_builder(&self) -> &R {
        &self.receipt_builder
    }

    /// Exposes the chain specification.
    pub const fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Exposes the EVM factory.
    pub const fn evm_factory(&self) -> &EvmFactory {
        &self.evm_factory
    }

    pub fn bor_params(&self) -> &Arc<BorParams> {
        &self.bor_params
    }
}

impl<R, Spec, EvmF> BlockExecutorFactory for BorBlockExecutorFactory<R, Spec, EvmF>
where
    R: ReceiptBuilder<Transaction = TransactionSigned, Receipt: TxReceipt<Log = Log>>,
    // TODO: Add the bor spec here instead of eth spec
    Spec: EthExecutorSpec + EthChainSpec,
    EvmF: EvmFactory<Tx: FromRecoveredTx<TransactionSigned> + FromTxWithEncoded<TransactionSigned>>,
    Self: 'static,
    TxEnv: IntoTxEnv<EvmF::Tx>,
{
    type EvmFactory = EvmF;
    type ExecutionCtx<'a> = EthBlockExecutionCtx<'a>;
    type Transaction = R::Transaction;
    type Receipt = R::Receipt;

    fn evm_factory(&self) -> &Self::EvmFactory {
        &self.evm_factory
    }

    fn create_executor<'a, DB, I>(
        &'a self,
        evm: EvmF::Evm<&'a mut State<DB>, I>,
        ctx: Self::ExecutionCtx<'a>,
    ) -> impl BlockExecutorFor<'a, Self, DB, I>
    where
        DB: Database + 'a,
        I: Inspector<EvmF::Context<&'a mut State<DB>>> + 'a,
    {
        BorBlockExecutor::new(
            evm,
            ctx,
            &self.spec,
            &self.receipt_builder,
            self.bor_params.clone(),
        )
    }
}

/// Block executor for Ethereum.
#[derive(Debug)]
pub struct BorBlockExecutor<'a, Evm, Spec: EthChainSpec + Clone, R: ReceiptBuilder> {
    /// Reference to the specification object.
    spec: Spec,

    /// Context for block execution.
    pub ctx: EthBlockExecutionCtx<'a>,
    /// Inner EVM.
    evm: Evm,
    /// Utility to call system smart contracts.
    system_caller: SystemCaller<Spec>,
    /// Receipt builder.
    receipt_builder: R,

    /// Receipts of executed transactions.
    receipts: Vec<R::Receipt>,
    /// Total gas used by transactions in this block.
    gas_used: u64,

    bor_params: Arc<BorParams>,
}

impl<'a, Evm, Spec, R> BorBlockExecutor<'a, Evm, Spec, R>
where
    Spec: EthChainSpec + Clone,
    R: ReceiptBuilder,
{
    /// Creates a new [`EthBlockExecutor`]
    pub fn new(
        evm: Evm,
        ctx: EthBlockExecutionCtx<'a>,
        spec: Spec,
        receipt_builder: R,
        bor_params: Arc<BorParams>,
    ) -> Self {
        Self {
            evm,
            ctx,
            receipts: Vec::new(),
            gas_used: 0,
            system_caller: SystemCaller::new(spec.clone(), bor_params.clone()),
            spec,
            receipt_builder,
            bor_params,
        }
    }
}

impl<'db, DB, E, Spec: EthChainSpec + Clone, R> BlockExecutor for BorBlockExecutor<'_, E, Spec, R>
where
    DB: Database + 'db,
    E: Evm<
        DB = &'db mut State<DB>,
        Tx: FromRecoveredTx<TransactionSigned> + FromTxWithEncoded<TransactionSigned>,
    >,
    Spec: EthExecutorSpec,
    R: ReceiptBuilder<
        Transaction = TransactionSigned, /*TODO: Encodable2718 */
        Receipt: TxReceipt<Log = Log>,
    >,
    TxEnv: IntoTxEnv<E::Tx>,
{
    type Transaction = R::Transaction;
    type Receipt = R::Receipt;
    type Evm = E;

    fn apply_pre_execution_changes(&mut self) -> Result<(), BlockExecutionError> {
        Ok(())
    }

    fn execute_transaction_with_commit_condition(
        &mut self,
        tx: impl ExecutableTx<Self>,
        f: impl FnOnce(&ExecutionResult<<Self::Evm as Evm>::HaltReason>) -> CommitChanges,
    ) -> Result<Option<u64>, BlockExecutionError> {
        Ok(Some(0))
    }

    /// Executes all transactions in a block, applying pre and post execution changes.
    fn execute_block(
        mut self,
        transactions: impl IntoIterator<Item = impl ExecutableTx<Self>>,
    ) -> Result<BlockExecutionResult<Self::Receipt>, BlockExecutionError>
    where
        Self: Sized,
    {
        self.apply_pre_execution_changes()?;

        for tx in transactions {
            self.execute_transaction(tx)?;
        }

        if self
            .bor_params
            .bor_config
            .is_sprint_start(self.evm.block().number)
        {
            self.system_caller
                .check_and_apply_commit_span(&mut self.evm)
                .map_err(|e| InternalBlockExecutionError::other(e))?;

            self.system_caller
                .apply_state_sync_contract_call(&mut self.evm)
                .map_err(|e| InternalBlockExecutionError::other(e))?;
        }

        self.apply_post_execution_changes()
    }

    fn execute_transaction_with_result_closure(
        &mut self,
        tx: impl ExecutableTx<Self>,
        f: impl FnOnce(&ExecutionResult<<Self::Evm as Evm>::HaltReason>),
    ) -> Result<u64, BlockExecutionError> {
        // The sum of the transaction's gas limit, Tg, and the gas utilized in this block prior,
        // must be no greater than the block's gasLimit.
        let block_available_gas = self.evm.block().gas_limit - self.gas_used;

        if tx.tx().gas_limit() > block_available_gas {
            return Err(
                BlockValidationError::TransactionGasLimitMoreThanAvailableBlockGas {
                    transaction_gas_limit: tx.tx().gas_limit(),
                    block_available_gas,
                }
                .into(),
            );
        }

        // Execute transaction.
        let result_and_state = self
            .evm
            .transact(tx)
            .map_err(|err| BlockExecutionError::evm(err, tx.tx().trie_hash()))?;

        // TODO: Need to add the state hook here
        // self.system_caller.on_state(
        //     StateChangeSource::Transaction(self.receipts.len()),
        //     &result_and_state.state,
        // );

        let ResultAndState { result, state } = result_and_state;

        f(&result);

        let gas_used = result.gas_used();

        // append gas used
        self.gas_used += gas_used;

        // Push transaction changeset and calculate header bloom filter for receipt.
        self.receipts
            .push(self.receipt_builder.build_receipt(ReceiptBuilderCtx {
                tx: tx.tx(),
                evm: &self.evm,
                result,
                state: &state,
                cumulative_gas_used: self.gas_used,
            }));

        // Commit the state changes.
        self.evm.db_mut().commit(state);

        Ok(gas_used)
    }

    fn finish(
        mut self,
    ) -> Result<(Self::Evm, BlockExecutionResult<R::Receipt>), BlockExecutionError> {
        let requests = Requests::default();

        // TODO: Confirm that block gas usage doesn't include the gas used by the state sync contract.
        Ok((
            self.evm,
            BlockExecutionResult {
                receipts: self.receipts,
                requests,
                gas_used: self.gas_used,
            },
        ))
    }

    fn set_state_hook(&mut self, hook: Option<Box<dyn OnStateHook>>) {
        todo!()
    }

    fn evm_mut(&mut self) -> &mut Self::Evm {
        &mut self.evm
    }

    fn evm(&self) -> &Self::Evm {
        &self.evm
    }
}
