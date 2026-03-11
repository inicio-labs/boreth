//! Bor chain specification.

pub mod constants;
pub use constants::*;

mod hardfork;
pub use hardfork::BorHardfork;

pub mod params;

mod chainspec;
pub use chainspec::{BorChainSpec, bor_amoy_chainspec, bor_mainnet_chainspec};

mod amoy;
pub use amoy::bor_amoy_genesis;

mod genesis;
pub use genesis::bor_mainnet_genesis;
