//! Bor chain specification.

pub mod constants;
pub use constants::*;

mod hardfork;
pub use hardfork::BorHardfork;

pub mod params;
