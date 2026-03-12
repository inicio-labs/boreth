//! Bor hardfork definitions.

use core::fmt;
use core::str::FromStr;

use reth_ethereum_forks::Hardfork;

/// All Polygon Bor hardforks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BorHardfork {
    /// Delhi hardfork.
    Delhi,
    /// Indore hardfork.
    Indore,
    /// Agra hardfork.
    Agra,
    /// Napoli hardfork.
    Napoli,
    /// Ahmedabad hardfork.
    Ahmedabad,
    /// Bhilai hardfork.
    Bhilai,
    /// Rio hardfork.
    Rio,
    /// Madhugiri hardfork.
    Madhugiri,
    /// Dandeli hardfork.
    Dandeli,
    /// Lisovo hardfork.
    Lisovo,
}

impl BorHardfork {
    /// Returns the mainnet (chain 137) activation block number for this hardfork.
    pub const fn mainnet_block(&self) -> u64 {
        match self {
            Self::Delhi => 38_189_056,
            Self::Indore => 44_934_656,
            Self::Agra => 50_523_000,
            Self::Napoli => 68_195_328,
            Self::Ahmedabad => 73_100_000,
            Self::Bhilai => 76_000_000,
            Self::Rio => 77_414_656,
            Self::Madhugiri => 80_084_800,
            Self::Dandeli => 81_900_000,
            Self::Lisovo => 83_756_500,
        }
    }

    /// Returns the Amoy testnet (chain 80002) activation block number for this hardfork.
    /// Values from Go-Bor's `AmoyChainConfig`.
    pub const fn amoy_block(&self) -> u64 {
        match self {
            Self::Delhi => 73_100,
            Self::Indore => 73_100,
            Self::Agra => 73_100,
            Self::Napoli => 73_100,
            Self::Ahmedabad => 11_865_856,
            Self::Bhilai => 22_765_056,
            Self::Rio => 26_272_256,
            Self::Madhugiri => 28_899_616,
            Self::Dandeli => 31_890_000,
            Self::Lisovo => 33_634_700,
        }
    }

    /// Returns all hardfork variants in activation order.
    pub const fn all() -> &'static [Self] {
        &[
            Self::Delhi,
            Self::Indore,
            Self::Agra,
            Self::Napoli,
            Self::Ahmedabad,
            Self::Bhilai,
            Self::Rio,
            Self::Madhugiri,
            Self::Dandeli,
            Self::Lisovo,
        ]
    }
}

impl Hardfork for BorHardfork {
    fn name(&self) -> &'static str {
        match self {
            Self::Delhi => "Delhi",
            Self::Indore => "Indore",
            Self::Agra => "Agra",
            Self::Napoli => "Napoli",
            Self::Ahmedabad => "Ahmedabad",
            Self::Bhilai => "Bhilai",
            Self::Rio => "Rio",
            Self::Madhugiri => "Madhugiri",
            Self::Dandeli => "Dandeli",
            Self::Lisovo => "Lisovo",
        }
    }
}

impl fmt::Display for BorHardfork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for BorHardfork {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "delhi" => Ok(Self::Delhi),
            "indore" => Ok(Self::Indore),
            "agra" => Ok(Self::Agra),
            "napoli" => Ok(Self::Napoli),
            "ahmedabad" => Ok(Self::Ahmedabad),
            "bhilai" => Ok(Self::Bhilai),
            "rio" => Ok(Self::Rio),
            "madhugiri" => Ok(Self::Madhugiri),
            "dandeli" => Ok(Self::Dandeli),
            "lisovo" => Ok(Self::Lisovo),
            _ => Err(format!("unknown Bor hardfork: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardfork_ordering() {
        let forks = BorHardfork::all();
        for window in forks.windows(2) {
            assert!(
                window[0].mainnet_block() < window[1].mainnet_block(),
                "{} (block {}) should activate before {} (block {})",
                window[0],
                window[0].mainnet_block(),
                window[1],
                window[1].mainnet_block(),
            );
        }
    }

    #[test]
    fn test_delhi_block() {
        assert_eq!(BorHardfork::Delhi.mainnet_block(), 38_189_056);
    }

    #[test]
    fn test_rio_block() {
        assert_eq!(BorHardfork::Rio.mainnet_block(), 77_414_656);
    }

    #[test]
    fn test_madhugiri_block() {
        assert_eq!(BorHardfork::Madhugiri.mainnet_block(), 80_084_800);
    }

    #[test]
    fn test_lisovo_block() {
        assert_eq!(BorHardfork::Lisovo.mainnet_block(), 83_756_500);
    }
}
