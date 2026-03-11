//! Bor receipt cumulative gas derivation.
//!
//! Bor system transactions (state sync, span commit) don't consume gas,
//! so cumulative gas for Bor receipts must be derived from regular transactions.

/// Derive cumulative gas used for Bor receipts in a block.
/// Bor system transactions (state sync, span commit) use 0 gas,
/// so cumulative gas is just the sum of regular transaction gas.
pub fn derive_bor_receipt_gas(regular_cumulative_gas: u64, _bor_tx_index: usize) -> u64 {
    // Bor system txs don't consume gas, so cumulative gas
    // for a Bor receipt equals the cumulative gas after the last regular tx
    regular_cumulative_gas
}

/// Check if a transaction at the given index is a Bor system transaction.
pub fn is_bor_system_tx(tx_index: usize, total_txs: usize, bor_tx_count: usize) -> bool {
    // Bor system txs are appended at the end of the block
    tx_index >= total_txs - bor_tx_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_bor_receipt_gas_returns_regular_cumulative() {
        assert_eq!(derive_bor_receipt_gas(1_000_000, 0), 1_000_000);
        assert_eq!(derive_bor_receipt_gas(5_000_000, 3), 5_000_000);
    }

    #[test]
    fn test_derive_bor_receipt_gas_zero() {
        assert_eq!(derive_bor_receipt_gas(0, 0), 0);
    }

    #[test]
    fn test_is_bor_system_tx_last_txs_are_bor() {
        // Block with 10 txs, last 2 are Bor system txs
        assert!(!is_bor_system_tx(0, 10, 2));
        assert!(!is_bor_system_tx(7, 10, 2));
        assert!(is_bor_system_tx(8, 10, 2));
        assert!(is_bor_system_tx(9, 10, 2));
    }

    #[test]
    fn test_is_bor_system_tx_single_bor_tx() {
        // Block with 5 txs, last 1 is Bor system tx
        assert!(!is_bor_system_tx(3, 5, 1));
        assert!(is_bor_system_tx(4, 5, 1));
    }

    #[test]
    fn test_is_bor_system_tx_all_bor() {
        // Block where all txs are Bor system txs
        assert!(is_bor_system_tx(0, 3, 3));
        assert!(is_bor_system_tx(1, 3, 3));
        assert!(is_bor_system_tx(2, 3, 3));
    }
}
