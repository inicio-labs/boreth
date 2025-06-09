use std::{borrow::Cow, convert::Infallible, fmt::Debug, sync::Arc};

use alloy_consensus::{Header, Transaction, TxReceipt};
use alloy_eips::{eip7685::Requests, Encodable2718};
use alloy_evm::{
    block::{
        state_changes::{balance_increment_state, post_block_balance_increments},
        BlockExecutionError, BlockExecutionResult, BlockExecutor, BlockExecutorFactory,
        BlockExecutorFor, BlockValidationError, ExecutableTx, InternalBlockExecutionError,
        OnStateHook, StateChangePostBlockSource, StateChangeSource,
    },
    eth::{
        receipt_builder::{AlloyReceiptBuilder, ReceiptBuilder, ReceiptBuilderCtx},
        spec::{EthExecutorSpec, EthSpec},
        EthBlockExecutionCtx,
    },
    precompiles::PrecompilesMap,
    Database, EthEvmFactory, Evm, EvmEnv, EvmFactory, FromRecoveredTx, FromTxWithEncoded,
    IntoTxEnv,
};
use alloy_primitives::Bytes;
use alloy_primitives::Log;
use bor::heimdall::{client::HeimdallClient, genesis_contract_client::GenesisContractClient};
use reth_chainspec::{ChainSpec, EthChainSpec};
use reth_ethereum_primitives::{EthPrimitives, TransactionSigned};
use reth_evm::{
    execute::BlockBuilder, ConfigureEvm, EvmEnvFor, EvmFactoryFor, EvmFor, ExecutionCtxFor,
    InspectorFor, NextBlockEnvAttributes, TransactionEnv, TxEnvFor,
};
use reth_evm_ethereum::{EthBlockAssembler, RethReceiptBuilder};
use reth_revm::{context_interface::block::BlobExcessGasAndPrice, State};
use revm::{DatabaseCommit, Inspector};
use revm_context::{
    result::{ExecutionResult, ResultAndState},
    BlockEnv, CfgEnv,
};

use crate::executor::system_call::SystemCaller;

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
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self::ethereum(chain_spec)
    }

    /// Creates a new Ethereum EVM configuration.
    pub fn ethereum(chain_spec: Arc<ChainSpec>) -> Self {
        Self::new_with_evm_factory(chain_spec, EthEvmFactory::default())
    }

    /// Creates a new Ethereum EVM configuration for the ethereum mainnet.
    pub fn mainnet() -> Self {
        // Self::ethereum(MAINNET.clone())
        todo!()
    }
}

impl BorEvmConfig {
    /// Creates a new Ethereum EVM configuration with the given chain spec and EVM factory.
    pub fn new_with_evm_factory(chain_spec: Arc<ChainSpec>, evm_factory: EthEvmFactory) -> Self {
        Self {
            block_assembler: EthBlockAssembler::new(chain_spec.clone()),
            executor_factory: BorBlockExecutorFactory::new(
                RethReceiptBuilder::default(),
                chain_spec,
                evm_factory,
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

#[derive(Debug, Clone, Default, Copy)]
pub struct BorBlockExecutorFactory<
    R = AlloyReceiptBuilder,
    Spec = EthSpec,
    EvmFactory = EthEvmFactory,
> {
    /// Receipt builder.
    receipt_builder: R,
    /// Chain specification.
    spec: Spec,
    /// EVM factory.
    evm_factory: EvmFactory,
}

impl<R, Spec, EvmFactory> BorBlockExecutorFactory<R, Spec, EvmFactory> {
    /// Creates a new [`EthBlockExecutorFactory`] with the given spec, [`EvmFactory`], and
    /// [`ReceiptBuilder`].
    pub const fn new(receipt_builder: R, spec: Spec, evm_factory: EvmFactory) -> Self {
        Self {
            receipt_builder,
            spec,
            evm_factory,
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
}

impl<R, Spec, EvmF> BlockExecutorFactory for BorBlockExecutorFactory<R, Spec, EvmF>
where
    R: ReceiptBuilder<Transaction: Transaction + Encodable2718, Receipt: TxReceipt<Log = Log>>,
    // TODO: Add the bor spec here instead of eth spec
    Spec: EthExecutorSpec,
    EvmF: EvmFactory<Tx: FromRecoveredTx<R::Transaction> + FromTxWithEncoded<R::Transaction>>,
    Self: 'static,
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
        BorBlockExecutor::new(evm, ctx, &self.spec, &self.receipt_builder)
    }
}

/// Block executor for Ethereum.
#[derive(Debug)]
pub struct BorBlockExecutor<'a, Evm, Spec, R: ReceiptBuilder> {
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
}

impl<'a, Evm, Spec, R> BorBlockExecutor<'a, Evm, Spec, R>
where
    Spec: Clone,
    R: ReceiptBuilder,
{
    /// Creates a new [`EthBlockExecutor`]
    pub fn new(
        evm: Evm,
        ctx: EthBlockExecutionCtx<'a>,
        spec: Spec,
        receipt_builder: R,
        heimdall_client: HeimdallClient,
        genesis_contract_client: GenesisContractClient,
    ) -> Self {
        Self {
            evm,
            ctx,
            receipts: Vec::new(),
            gas_used: 0,
            system_caller: SystemCaller::new(
                spec.clone(),
                heimdall_client,
                genesis_contract_client,
            ),
            spec,
            receipt_builder,
        }
    }
}

impl<'db, DB, E, Spec, R> BlockExecutor for BorBlockExecutor<'_, E, Spec, R>
where
    DB: Database + 'db,
    E: Evm<
        DB = &'db mut State<DB>,
        Tx: FromRecoveredTx<R::Transaction>
                + FromTxWithEncoded<R::Transaction>
                + IntoTxEnv<R::Transaction>,
    >,
    Spec: EthExecutorSpec,
    R: ReceiptBuilder<
        Transaction: Transaction, /*TODO: Encodable2718 */
        Receipt: TxReceipt<Log = Log>,
    >,
{
    type Transaction = R::Transaction;
    type Receipt = R::Receipt;
    type Evm = E;

    fn apply_pre_execution_changes(&mut self) -> Result<(), BlockExecutionError> {
        self.system_caller
            .apply_state_sync_contract_call(&mut self.evm)
            .map_err(|e| InternalBlockExecutionError::other(e))?;

        Ok(())
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

        let mut balance_increments = post_block_balance_increments(
            &self.spec,
            self.evm.block(),
            self.ctx.ommers,
            self.ctx.withdrawals.as_deref(),
        );

        // increment balances
        self.evm
            .db_mut()
            .increment_balances(balance_increments.clone())
            .map_err(|_| BlockValidationError::IncrementBalanceFailed)?;

        // TODO: Need to add the state hook here
        // // call state hook with changes due to balance increments.
        // self.system_caller.try_on_state_with(|| {
        //     balance_increment_state(&balance_increments, self.evm.db_mut()).map(|state| {
        //         (
        //             StateChangeSource::PostBlock(StateChangePostBlockSource::BalanceIncrements),
        //             Cow::Owned(state),
        //         )
        //     })
        // })?;

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
