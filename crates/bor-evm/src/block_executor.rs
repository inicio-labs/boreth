//! Custom Bor block executor that wraps Ethereum's executor to add Bor-specific
//! system calls during block finalization.
//!
//! # Bor System Calls
//!
//! Bor adds two types of system calls that are executed after user transactions
//! but before balance increments during block finalization:
//!
//! 1. **`commitSpan`** — At span boundaries, the next span's validator set is
//!    committed to the ValidatorSet contract at `0x1000`.
//!
//! 2. **`onStateReceive`** — At sprint boundaries (`block % sprint_size == 0`),
//!    state sync events from Heimdall L1 are relayed to the StateReceiver
//!    contract at `0x1001`.
//!
//! Both calls are executed as system calls from `SYSTEM_ADDRESS`
//! (`0xffffFFFfFFffffffffffffffFfFFFfffFFFfFFfE`).

use crate::system_call::{CommitSpanCall, StateReceiveCall};
use alloy_consensus::{Transaction, TransactionEnvelope, TxReceipt};
use alloy_eips::Encodable2718;
use alloy_primitives::{Bytes, Log, U256};
use reth_evm::{
    block::{
        BlockExecutionError, BlockExecutionResult, BlockExecutor, BlockExecutorFactory,
        BlockExecutorFor, ExecutableTx, OnStateHook,
    },
    eth::{
        EthBlockExecutionCtx, EthBlockExecutor, EthBlockExecutorFactory, EthTxResult,
        receipt_builder::ReceiptBuilder, spec::EthExecutorSpec,
    },
    Database, Evm, EvmFactory, FromRecoveredTx, FromTxWithEncoded,
};
use core::fmt::Debug;
use revm::{database::State, DatabaseCommit, Inspector};
use tracing::debug;

/// Pending span commitment data for system call execution.
#[derive(Debug, Clone)]
pub struct PendingCommitSpan {
    /// The span ID to commit.
    pub span_id: U256,
    /// ABI-encoded validator bytes.
    pub validator_bytes: Bytes,
}

/// Additional execution context specific to Bor consensus.
///
/// This is passed alongside `EthBlockExecutionCtx` and contains the
/// pre-computed system call data that the executor needs during `finish()`.
#[derive(Debug, Clone, Default)]
pub struct BorExecutionCtx {
    /// If set, a `commitSpan` system call will be executed during finalization.
    pub pending_commit_span: Option<PendingCommitSpan>,
    /// State sync events to relay via `onStateReceive` during finalization.
    /// Each entry is `(state_id, data)`.
    pub pending_state_syncs: Vec<(U256, Bytes)>,
}

/// Combined execution context for Bor block execution.
///
/// Contains both the standard Ethereum execution context (parent hash, ommers,
/// withdrawals) and Bor-specific data (pending system calls).
#[derive(Debug, Clone)]
pub struct BorBlockExecutionCtx<'a> {
    /// Standard Ethereum execution context.
    pub eth: EthBlockExecutionCtx<'a>,
    /// Bor-specific execution context with pending system calls.
    pub bor: BorExecutionCtx,
}

/// Block executor for Bor PoA consensus.
///
/// Wraps [`EthBlockExecutor`] and injects Bor system calls (`commitSpan`,
/// `onStateReceive`) in `finish()` before the Ethereum-level post-execution
/// changes (balance increments, etc.).
pub struct BorBlockExecutor<'a, E, Spec, R: ReceiptBuilder> {
    /// The inner Ethereum block executor. All fields are `pub` in alloy-evm
    /// so we can access the EVM and receipts directly.
    pub inner: EthBlockExecutor<'a, E, Spec, R>,
    /// Bor-specific execution context.
    pub bor_ctx: BorExecutionCtx,
}

impl<E: Debug, Spec: Debug, R: ReceiptBuilder> Debug for BorBlockExecutor<'_, E, Spec, R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BorBlockExecutor")
            .field("bor_ctx", &self.bor_ctx)
            .finish_non_exhaustive()
    }
}

impl<'a, E, Spec, R> BorBlockExecutor<'a, E, Spec, R>
where
    Spec: Clone,
    R: ReceiptBuilder,
{
    /// Create a new Bor block executor wrapping an Ethereum executor.
    pub fn new(
        evm: E,
        eth_ctx: EthBlockExecutionCtx<'a>,
        bor_ctx: BorExecutionCtx,
        spec: Spec,
        receipt_builder: R,
    ) -> Self {
        Self {
            inner: EthBlockExecutor::new(evm, eth_ctx, spec, receipt_builder),
            bor_ctx,
        }
    }
}

impl<'db, DB, E, Spec, R> BorBlockExecutor<'_, E, Spec, R>
where
    DB: Database + 'db,
    E: Evm<
        DB = &'db mut State<DB>,
        Tx: FromRecoveredTx<R::Transaction> + FromTxWithEncoded<R::Transaction>,
    >,
    Spec: EthExecutorSpec,
    R: ReceiptBuilder<Transaction: Transaction + Encodable2718>,
{
    /// Execute Bor system calls using the inner EVM.
    ///
    /// Called during `finish()` before delegating to the Ethereum executor's
    /// finalization logic. This ensures Bor state changes are committed to the
    /// DB before balance increments and state root computation.
    fn execute_bor_system_calls(&mut self) -> Result<(), BlockExecutionError> {
        // 1. commitSpan — update validator set at span boundaries
        if let Some(ref commit) = self.bor_ctx.pending_commit_span {
            let call = CommitSpanCall {
                span_id: commit.span_id,
                validator_bytes: commit.validator_bytes.clone(),
            };

            debug!(
                target: "bor::executor",
                span_id = %commit.span_id,
                "executing commitSpan system call"
            );

            let res = self
                .inner
                .evm
                .transact_system_call(
                    CommitSpanCall::caller(),
                    CommitSpanCall::to_address(),
                    call.call_data(),
                )
                .map_err(|e| BlockExecutionError::msg(format!("commitSpan failed: {e}")))?;

            self.inner.evm.db_mut().commit(res.state);
        }

        // 2. onStateReceive — relay state sync events at sprint boundaries
        for (state_id, data) in &self.bor_ctx.pending_state_syncs {
            let call = StateReceiveCall {
                state_id: *state_id,
                data: data.clone(),
            };

            debug!(
                target: "bor::executor",
                state_id = %state_id,
                "executing onStateReceive system call"
            );

            let res = self
                .inner
                .evm
                .transact_system_call(
                    StateReceiveCall::caller(),
                    StateReceiveCall::to_address(),
                    call.call_data(),
                )
                .map_err(|e| {
                    BlockExecutionError::msg(format!(
                        "onStateReceive failed for state_id {state_id}: {e}"
                    ))
                })?;

            self.inner.evm.db_mut().commit(res.state);
        }

        Ok(())
    }
}

impl<'db, DB, E, Spec, R> BlockExecutor for BorBlockExecutor<'_, E, Spec, R>
where
    DB: Database + 'db,
    E: Evm<
        DB = &'db mut State<DB>,
        Tx: FromRecoveredTx<R::Transaction> + FromTxWithEncoded<R::Transaction>,
    >,
    Spec: EthExecutorSpec,
    R: ReceiptBuilder<
        Transaction: Transaction + Encodable2718,
        Receipt: TxReceipt<Log = Log>,
    >,
{
    type Transaction = R::Transaction;
    type Receipt = R::Receipt;
    type Evm = E;
    type Result =
        EthTxResult<E::HaltReason, <R::Transaction as TransactionEnvelope>::TxType>;

    fn apply_pre_execution_changes(&mut self) -> Result<(), BlockExecutionError> {
        self.inner.apply_pre_execution_changes()
    }

    fn execute_transaction_without_commit(
        &mut self,
        tx: impl ExecutableTx<Self>,
    ) -> Result<Self::Result, BlockExecutionError> {
        self.inner.execute_transaction_without_commit(tx)
    }

    fn commit_transaction(&mut self, output: Self::Result) -> Result<u64, BlockExecutionError> {
        self.inner.commit_transaction(output)
    }

    fn finish(
        mut self,
    ) -> Result<(Self::Evm, BlockExecutionResult<R::Receipt>), BlockExecutionError> {
        // Execute Bor system calls BEFORE Ethereum's finish() handles
        // balance increments. This matches Go Bor's Finalize ordering:
        // user txs → commitSpan → onStateReceive → balance increments
        self.execute_bor_system_calls()?;

        // Delegate to Ethereum's finish for:
        // - Prague requests (no-op on Bor)
        // - Balance increments (no-op on Bor: no ommers, no withdrawals)
        // - DAO fork (no-op on Bor)
        self.inner.finish()
    }

    fn set_state_hook(&mut self, hook: Option<Box<dyn OnStateHook>>) {
        self.inner.set_state_hook(hook);
    }

    fn evm_mut(&mut self) -> &mut Self::Evm {
        self.inner.evm_mut()
    }

    fn evm(&self) -> &Self::Evm {
        self.inner.evm()
    }

    fn receipts(&self) -> &[Self::Receipt] {
        self.inner.receipts()
    }
}

/// Factory for creating [`BorBlockExecutor`] instances.
///
/// Wraps [`EthBlockExecutorFactory`] and constructs executors with Bor-specific
/// execution context.
#[derive(Debug)]
pub struct BorBlockExecutorFactory<R, Spec, EvmFactory> {
    /// Inner Ethereum factory.
    inner: EthBlockExecutorFactory<R, Spec, EvmFactory>,
}

impl<R: Clone, Spec: Clone, EvmF: Clone> Clone for BorBlockExecutorFactory<R, Spec, EvmF> {
    fn clone(&self) -> Self {
        Self {
            inner: EthBlockExecutorFactory::new(
                self.inner.receipt_builder().clone(),
                self.inner.spec().clone(),
                self.inner.evm_factory().clone(),
            ),
        }
    }
}

impl<R, Spec, EvmFactory> BorBlockExecutorFactory<R, Spec, EvmFactory> {
    /// Create a new Bor block executor factory.
    pub const fn new(inner: EthBlockExecutorFactory<R, Spec, EvmFactory>) -> Self {
        Self { inner }
    }

    /// Returns the inner Ethereum factory.
    pub const fn inner(&self) -> &EthBlockExecutorFactory<R, Spec, EvmFactory> {
        &self.inner
    }
}

impl<R, Spec, EvmF> BlockExecutorFactory for BorBlockExecutorFactory<R, Spec, EvmF>
where
    R: ReceiptBuilder<
        Transaction: Transaction + Encodable2718,
        Receipt: TxReceipt<Log = Log>,
    >,
    Spec: EthExecutorSpec,
    EvmF: EvmFactory<Tx: FromRecoveredTx<R::Transaction> + FromTxWithEncoded<R::Transaction>>,
    Self: 'static,
{
    type EvmFactory = EvmF;
    type ExecutionCtx<'a> = BorBlockExecutionCtx<'a>;
    type Transaction = R::Transaction;
    type Receipt = R::Receipt;

    fn evm_factory(&self) -> &Self::EvmFactory {
        self.inner.evm_factory()
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
            ctx.eth,
            ctx.bor,
            self.inner.spec(),
            self.inner.receipt_builder(),
        )
    }
}
