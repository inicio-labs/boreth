//! Bor EVM configuration and execution.

pub mod config;
pub use config::{BorEvmConfig, P256_VERIFY_ADDRESS, bor_precompile_addresses};

pub mod system_call;
pub use system_call::{CommitSpanCall, StateReceiveCall, prepare_state_sync_calls};
