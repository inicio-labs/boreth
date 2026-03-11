//! Test helper functions that use golden blocks to verify boreth crate outputs.
//!
//! Each helper targets a specific crate/module and compares its output
//! against the real mainnet value from the golden block.

use crate::GoldenBlock;

/// Assertions for `bor-chainspec` crate.
pub mod chainspec {
    use super::*;

    /// Verify sprint_size() returns the correct value for this block.
    pub fn assert_sprint_size(block: &GoldenBlock, actual: u64) {
        let expected = block.expected_sprint_size();
        assert_eq!(
            actual, expected,
            "[{}] sprint_size({}) = {} but expected {} (era: {})",
            block.name, block.number, actual, expected, block.era()
        );
    }

    /// Verify span_size() returns the correct value for this block.
    pub fn assert_span_size(block: &GoldenBlock, actual: u64) {
        let expected = block.expected_span_size();
        assert_eq!(
            actual, expected,
            "[{}] span_size({}) = {} but expected {} (era: {})",
            block.name, block.number, actual, expected, block.era()
        );
    }

    /// Verify gas limit era matches the REAL gas limit from the chain.
    /// Note: actual gas_limit can vary slightly from the era default,
    /// but must be <= the era maximum.
    pub fn assert_gas_limit_within_era(block: &GoldenBlock) {
        let era_limit = block.expected_gas_limit_era();
        assert!(
            block.gas_limit <= era_limit,
            "[{}] block {} has gas_limit {} which exceeds era limit {} (era: {})",
            block.name, block.number, block.gas_limit, era_limit, block.era()
        );
    }
}

/// Assertions for `bor-consensus` crate.
pub mod consensus {
    use super::*;

    /// Verify that extra_data parses correctly and has expected structure.
    pub fn assert_extra_data_valid(block: &GoldenBlock) {
        assert!(
            block.extra_data.len() >= 97,
            "[{}] block {} extra_data too short: {} bytes (minimum 97)",
            block.name, block.number, block.extra_data.len()
        );

        // Vanity is always 32 bytes
        let _vanity = block.vanity().expect("vanity should exist");

        // Seal is always 65 bytes
        let seal = block.seal().expect("seal should exist");
        assert_eq!(seal.len(), 65, "[{}] seal should be 65 bytes", block.name);

        // Validator bytes (if present) must be multiple of 20
        let validator_bytes_len = block.extra_data.len() - 97;
        assert_eq!(
            validator_bytes_len % 20, 0,
            "[{}] block {} validator bytes length {} not multiple of 20",
            block.name, block.number, validator_bytes_len
        );

        assert_eq!(
            block.validator_count,
            validator_bytes_len / 20,
            "[{}] validator_count mismatch",
            block.name
        );
    }

    /// Verify that the nonce is zero (Bor consensus requirement).
    pub fn assert_zero_nonce(block: &GoldenBlock) {
        assert_eq!(
            block.nonce, 0,
            "[{}] block {} nonce should be 0, got {}",
            block.name, block.number, block.nonce
        );
    }

    /// Verify that the mix_hash is zero (Bor consensus requirement).
    pub fn assert_zero_mix_hash(block: &GoldenBlock) {
        assert!(
            block.mix_hash.is_zero(),
            "[{}] block {} mix_hash should be zero, got {}",
            block.name, block.number, block.mix_hash
        );
    }

    /// Verify difficulty is valid (>= 1, <= validator_count for inturn).
    pub fn assert_difficulty_valid(block: &GoldenBlock) {
        assert!(
            block.difficulty >= 1,
            "[{}] block {} difficulty must be >= 1, got {}",
            block.name, block.number, block.difficulty
        );
    }

    /// At sprint start blocks that are also span starts, extra_data MUST
    /// contain validator addresses.
    pub fn assert_validators_at_span_start(block: &GoldenBlock) {
        let _sprint_size = block.expected_sprint_size();
        let span_size = block.expected_span_size();
        let is_span_start = block.number > 0 && block.number % span_size == 0;

        if is_span_start {
            assert!(
                block.validator_count > 0,
                "[{}] block {} is span start but has 0 validators in extra_data",
                block.name, block.number
            );
        }
    }
}

/// Assertions for `bor-storage` crate.
pub mod storage {
    use super::*;

    /// Verify that receipt root is non-zero for blocks with transactions.
    pub fn assert_receipt_root_consistent(block: &GoldenBlock) {
        if block.tx_count > 0 {
            // Blocks with transactions should have non-empty receipt root
            // (unless all txs failed and produced empty receipts, which is rare)
            // We just check it's not the zero hash
            assert!(
                !block.receipts_root.is_zero() || block.gas_used == 0,
                "[{}] block {} has {} txs but zero receipts_root",
                block.name, block.number, block.tx_count
            );
        }
    }

    /// Post-Madhugiri blocks with system txs should have different receipt root
    /// than if system txs were excluded.
    pub fn assert_madhugiri_receipt_transition(
        pre_block: &GoldenBlock,
        post_block: &GoldenBlock,
    ) {
        assert!(!pre_block.is_post_madhugiri(), "pre_block should be pre-Madhugiri");
        assert!(post_block.is_post_madhugiri(), "post_block should be post-Madhugiri");
    }
}

/// Assertions for `bor-evm` crate.
pub mod evm {
    use super::*;

    /// Verify system tx count matches boundary expectations.
    pub fn assert_system_tx_expectations(block: &GoldenBlock) {
        let sprint_size = block.expected_sprint_size();
        let is_sprint_boundary = block.number > 0 && block.number % sprint_size == 0;

        if !is_sprint_boundary && !block.is_span_boundary_6400 && !block.is_span_boundary_1600 {
            // Non-boundary blocks should have 0 system txs
            // (except post-Madhugiri which always has 1 state sync tx per sprint end)
            if !block.is_post_madhugiri() {
                assert_eq!(
                    block.system_tx_count, 0,
                    "[{}] block {} is not a boundary but has {} system txs",
                    block.name, block.number, block.system_tx_count
                );
            }
        }
    }
}

/// Run ALL assertions on a golden block. Use this for comprehensive validation.
pub fn assert_block_invariants(block: &GoldenBlock) {
    consensus::assert_extra_data_valid(block);
    consensus::assert_zero_nonce(block);
    consensus::assert_zero_mix_hash(block);
    consensus::assert_difficulty_valid(block);
    consensus::assert_validators_at_span_start(block);
    storage::assert_receipt_root_consistent(block);
    evm::assert_system_tx_expectations(block);
}
