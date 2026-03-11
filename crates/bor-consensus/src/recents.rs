use alloy_primitives::Address;
use std::collections::BTreeMap;

/// Tracks recent block signers to prevent double-signing within a window.
/// Window size = len(validators) / 2 + 1
#[derive(Debug, Clone, Default)]
pub struct Recents {
    signers: BTreeMap<u64, Address>, // block_number -> signer
}

impl Recents {
    pub fn new() -> Self {
        Self {
            signers: BTreeMap::new(),
        }
    }

    /// Check if signer has signed within the recent window
    pub fn is_recently_signed(
        &self,
        signer: &Address,
        current_block: u64,
        validator_count: usize,
    ) -> bool {
        let window = validator_count / 2 + 1;
        let start = current_block.saturating_sub(window as u64);
        for (_block, recent_signer) in self.signers.range(start..current_block) {
            if recent_signer == signer {
                return true;
            }
        }
        false
    }

    /// Record a signer for a block
    pub fn add_signer(&mut self, block: u64, signer: Address) {
        self.signers.insert(block, signer);
    }

    /// Prune entries older than the window
    pub fn prune(&mut self, current_block: u64, validator_count: usize) {
        let window = validator_count / 2 + 1;
        let cutoff = current_block.saturating_sub(window as u64);
        self.signers = self.signers.split_off(&cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recent_window_size() {
        // 10 validators → window = 10 / 2 + 1 = 6
        let mut recents = Recents::new();
        let signer = Address::ZERO;

        // Add signer at block 5
        recents.add_signer(5, signer);

        // Block 11 is outside the window (11 - 6 = 5, range is 5..11 which includes 5)
        assert!(recents.is_recently_signed(&signer, 11, 10));

        // Block 12 is outside the window (12 - 6 = 6, range is 6..12 which excludes 5)
        assert!(!recents.is_recently_signed(&signer, 12, 10));
    }

    #[test]
    fn test_reject_recent_signer() {
        let mut recents = Recents::new();
        let signer = Address::with_last_byte(1);

        recents.add_signer(10, signer);

        // Within window of 6 (10 validators): current_block=13, start=7, range 7..13 includes 10
        assert!(recents.is_recently_signed(&signer, 13, 10));
    }

    #[test]
    fn test_allow_after_window() {
        let mut recents = Recents::new();
        let signer = Address::with_last_byte(2);

        recents.add_signer(5, signer);

        // Outside window: current_block=20, start=14, range 14..20 does not include 5
        assert!(!recents.is_recently_signed(&signer, 20, 10));
    }
}
