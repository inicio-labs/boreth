//! Bor node builder and configuration.
//!
//! Wires together all Bor components: consensus, EVM, payload builder,
//! RPC, storage, and Heimdall client into a complete node.

pub mod node;
pub mod config;
pub mod handshake;

pub use node::BorNode;
pub use config::BorNodeConfig;
