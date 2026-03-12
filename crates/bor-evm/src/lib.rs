//! Bor EVM configuration and execution.

pub mod block_executor;
pub use block_executor::{
    BorBlockExecutionCtx, BorBlockExecutor, BorBlockExecutorFactory, BorExecutionCtx,
    PendingCommitSpan,
};

pub mod build;
pub use build::BorBlockAssembler;

pub mod config;
pub use config::{BorEvmConfig as BorEvmConfigPrecompiles, P256_VERIFY_ADDRESS, bor_precompile_addresses};

pub mod evm_config;
pub use evm_config::BorEvmConfig;

pub mod executor;
pub use executor::{SystemTxPlan, SystemTxResult, SystemCallRecord, plan_system_txs, execute_system_tx_plan};

pub mod system_call;
pub use system_call::{CommitSpanCall, StateReceiveCall, prepare_state_sync_calls};
