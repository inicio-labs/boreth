//! Golden test fixtures from real Polygon mainnet blocks.
//!
//! This crate provides immutable ground-truth block data fetched directly from
//! Polygon mainnet. Every field is the REAL on-chain value — not computed by boreth.
//!
//! ## How it works
//!
//! 1. `scripts/fetch_golden_blocks.py` fetches block data from Polygon RPC
//! 2. It writes JSON fixtures to `data/` and generates `src/generated.rs`
//! 3. Other crates use these fixtures to verify their computations against reality
//!
//! ## Usage in tests
//!
//! ```rust,ignore
//! use bor_testdata::blocks;
//!
//! #[test]
//! fn test_gas_limit_at_bhilai() {
//!     let pre = blocks::pre_bhilai();
//!     let post = blocks::bhilai_activation();
//!     assert_eq!(pre.gas_limit, 30_000_000);
//!     assert_eq!(post.gas_limit, 45_000_000);
//! }
//! ```
//!
//! ## Regenerating fixtures
//!
//! ```bash
//! cd crates/bor-testdata
//! python3 scripts/fetch_golden_blocks.py --rpc https://polygon-rpc.com
//! ```

pub mod blocks;
pub mod helpers;

use alloy_primitives::{Address, B256};

/// A golden block fixture with real mainnet values.
///
/// Every field here was fetched from the actual Polygon chain — it is NOT
/// computed by any boreth code. This makes it safe to use as test input
/// without circular "testing our code with our code" problems.
#[derive(Debug, Clone)]
pub struct GoldenBlock {
    // -- metadata --
    /// Fixture name (e.g., "delhi_activation")
    pub name: &'static str,
    /// Why this block was chosen
    pub why: &'static str,

    // -- header fields (from eth_getBlockByNumber) --
    pub number: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub state_root: B256,
    pub receipts_root: B256,
    pub miner: Address,
    pub difficulty: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub nonce: u64,
    pub mix_hash: B256,
    pub extra_data: Vec<u8>,
    pub base_fee_per_gas: u64,

    // -- transaction summary --
    pub tx_count: usize,
    pub system_tx_count: usize,

    // -- derived from extra_data --
    pub validator_count: usize,

    // -- boundary flags (computed from block number, NOT from boreth code) --
    pub is_sprint_boundary: bool,
    pub is_span_boundary_6400: bool,
    pub is_span_boundary_1600: bool,
}

impl GoldenBlock {
    /// Extract the 32-byte vanity prefix from extra_data.
    pub fn vanity(&self) -> Option<&[u8]> {
        if self.extra_data.len() >= 32 {
            Some(&self.extra_data[..32])
        } else {
            None
        }
    }

    /// Extract the 65-byte seal suffix from extra_data.
    pub fn seal(&self) -> Option<&[u8]> {
        if self.extra_data.len() >= 97 {
            Some(&self.extra_data[self.extra_data.len() - 65..])
        } else {
            None
        }
    }

    /// Extract validator addresses from extra_data (between vanity and seal).
    pub fn validator_addresses(&self) -> Vec<Address> {
        if self.extra_data.len() <= 97 {
            return vec![];
        }
        let validator_bytes = &self.extra_data[32..self.extra_data.len() - 65];
        validator_bytes
            .chunks_exact(20)
            .map(|chunk| Address::from_slice(chunk))
            .collect()
    }

    /// Returns true if this block is at or after the given hardfork block.
    pub fn is_at_or_after(&self, fork_block: u64) -> bool {
        self.number >= fork_block
    }

    /// Returns the expected sprint size for this block.
    /// Delhi (38_189_056): sprint 64 → 16
    pub fn expected_sprint_size(&self) -> u64 {
        if self.number < 38_189_056 { 64 } else { 16 }
    }

    /// Returns the expected span size for this block.
    /// Rio (77_414_656): span 6400 → 1600
    pub fn expected_span_size(&self) -> u64 {
        if self.number < 77_414_656 { 6400 } else { 1600 }
    }

    /// Returns the expected gas limit for this block's era.
    /// Bhilai (76_000_000): gas limit 30M → 45M
    pub fn expected_gas_limit_era(&self) -> u64 {
        if self.number < 76_000_000 { 30_000_000 } else { 45_000_000 }
    }

    /// Returns which hardfork era this block belongs to.
    pub fn era(&self) -> &'static str {
        if self.number < 38_189_056 { "pre-Delhi" }
        else if self.number < 44_934_656 { "Delhi" }
        else if self.number < 50_523_000 { "Indore" }
        else if self.number < 68_195_328 { "Agra" }
        else if self.number < 73_100_000 { "Napoli" }
        else if self.number < 76_000_000 { "Ahmedabad" }
        else if self.number < 77_414_656 { "Bhilai" }
        else if self.number < 80_084_800 { "Rio" }
        else if self.number < 81_900_000 { "Madhugiri" }
        else if self.number < 83_756_500 { "Dandeli" }
        else { "Lisovo" }
    }

    /// Whether this block is post-Madhugiri (unified receipt storage).
    pub fn is_post_madhugiri(&self) -> bool {
        self.number >= 80_084_800
    }
}

/// Block number constants for all hardforks (ground truth, not from boreth code).
pub mod forks {
    pub const DELHI: u64 = 38_189_056;
    pub const INDORE: u64 = 44_934_656;
    pub const AGRA: u64 = 50_523_000;
    pub const NAPOLI: u64 = 68_195_328;
    pub const AHMEDABAD: u64 = 73_100_000;
    pub const BHILAI: u64 = 76_000_000;
    pub const RIO: u64 = 77_414_656;
    pub const MADHUGIRI: u64 = 80_084_800;
    pub const DANDELI: u64 = 81_900_000;
    pub const LISOVO: u64 = 83_756_500;
}
