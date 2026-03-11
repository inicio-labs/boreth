//! Fork-dependent parameter functions for Polygon Bor.
//!
//! These functions return chain parameters that change at specific hardfork boundaries.
//! All block numbers reference Polygon PoS mainnet (chain 137).

use crate::BorHardfork;

/// Returns the sprint size at the given block number.
///
/// - Pre-Delhi: 64 blocks per sprint
/// - Post-Delhi: 16 blocks per sprint
pub fn sprint_size(block: u64) -> u64 {
    if block < BorHardfork::Delhi.mainnet_block() {
        64
    } else {
        16
    }
}

/// Returns the span size at the given block number.
///
/// - Pre-Rio: 6400 blocks per span
/// - Post-Rio: 1600 blocks per span
pub fn span_size(block: u64) -> u64 {
    if block < BorHardfork::Rio.mainnet_block() {
        6400
    } else {
        1600
    }
}

/// Returns the block gas limit at the given block number.
///
/// - Pre-Bhilai: 30,000,000
/// - Post-Bhilai: 45,000,000
pub fn block_gas_limit(block: u64) -> u64 {
    if block < BorHardfork::Bhilai.mainnet_block() {
        30_000_000
    } else {
        45_000_000
    }
}

/// Returns the base fee change denominator at the given block number.
///
/// - Pre-Delhi: 8
/// - Post-Delhi: 16
/// - Post-Bhilai: 64
pub fn base_fee_change_denominator(block: u64) -> u64 {
    if block >= BorHardfork::Bhilai.mainnet_block() {
        64
    } else if block >= BorHardfork::Delhi.mainnet_block() {
        16
    } else {
        8
    }
}

/// Returns `true` if the given block is the first block of a sprint.
///
/// Block 0 (genesis) is never a sprint start.
pub fn is_sprint_start(block: u64) -> bool {
    if block == 0 {
        return false;
    }
    let size = sprint_size(block);
    block % size == 0
}

/// Returns `true` if the given block is the start of a new span.
///
/// Block 0 (genesis) is never a span start.
pub fn is_span_start(block: u64, span_size: u64) -> bool {
    if block == 0 {
        return false;
    }
    block % span_size == 0
}

/// Returns the maximum contract code size at the given block number.
///
/// Currently always returns the standard EIP-170 limit of 24,576 bytes.
pub fn max_code_size(block: u64) -> usize {
    let _ = block;
    24_576
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprint_size_pre_delhi() {
        assert_eq!(sprint_size(38_189_055), 64);
    }

    #[test]
    fn test_sprint_size_post_delhi() {
        assert_eq!(sprint_size(38_189_056), 16);
    }

    #[test]
    fn test_span_size_pre_rio() {
        assert_eq!(span_size(77_414_655), 6400);
    }

    #[test]
    fn test_span_size_post_rio() {
        assert_eq!(span_size(77_414_656), 1600);
    }

    #[test]
    fn test_gas_limit_pre_bhilai() {
        assert_eq!(block_gas_limit(76_000_000 - 1), 30_000_000);
    }

    #[test]
    fn test_gas_limit_post_bhilai() {
        assert_eq!(block_gas_limit(76_000_000), 45_000_000);
    }

    #[test]
    fn test_base_fee_denom_evolution() {
        // Pre-Delhi
        assert_eq!(base_fee_change_denominator(1), 8);
        // Post-Delhi, pre-Bhilai
        assert_eq!(base_fee_change_denominator(38_189_056), 16);
        // Post-Bhilai
        assert_eq!(base_fee_change_denominator(76_000_000), 64);
    }

    #[test]
    fn test_is_sprint_start() {
        // Block 0 (genesis) is never a sprint start
        assert!(!is_sprint_start(0));

        // Pre-Delhi sprint size is 64
        assert!(is_sprint_start(64));
        assert!(!is_sprint_start(65));

        // Post-Delhi sprint size is 16
        assert!(is_sprint_start(38_189_056)); // Delhi block itself is divisible by 16
    }

    #[test]
    fn test_is_span_start() {
        // Block 0 (genesis) is never a span start
        assert!(!is_span_start(0, 6400));

        assert!(is_span_start(6400, 6400));
        assert!(!is_span_start(6401, 6400));
    }

    #[test]
    fn test_max_code_size() {
        assert_eq!(max_code_size(0), 24_576);
        assert_eq!(max_code_size(100_000_000), 24_576);
    }
}
