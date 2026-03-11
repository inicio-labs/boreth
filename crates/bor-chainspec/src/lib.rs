//! Bor chain specification.

pub mod constants;
pub use constants::*;

mod hardfork;
pub use hardfork::BorHardfork;

mod chainspec;
pub use chainspec::{BorChainSpec, bor_amoy_chainspec, bor_mainnet_chainspec};
