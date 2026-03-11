//! CLI chain spec parser for Boreth.
//!
//! Provides a [`BorChainSpecParser`] compatible with Reth's CLI that
//! resolves "amoy" and "polygon" to the appropriate chain specifications.

use std::sync::Arc;

use reth_chainspec::ChainSpec;
use reth_cli::chainspec::ChainSpecParser;

/// Chain spec parser that understands Polygon network names.
///
/// Supported chains:
/// - `"amoy"` — Polygon Amoy testnet (chain ID 80002)
/// - `"polygon"` / `"mainnet"` — Polygon PoS mainnet (chain ID 137)
/// - Any file path or inline JSON genesis — parsed via Reth's standard genesis parser
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct BorChainSpecParser;

impl ChainSpecParser for BorChainSpecParser {
    type ChainSpec = ChainSpec;

    const SUPPORTED_CHAINS: &'static [&'static str] = &["amoy", "polygon"];

    fn parse(s: &str) -> eyre::Result<Arc<ChainSpec>> {
        match s {
            "amoy" => Ok(Arc::new(crate::bor_amoy_genesis().into_inner())),
            "polygon" | "mainnet" => Ok(Arc::new(crate::bor_mainnet_genesis().into_inner())),
            _ => {
                // Fall back to parsing as a genesis JSON file path or inline JSON
                let genesis = reth_cli::chainspec::parse_genesis(s)?;
                Ok(Arc::new(genesis.into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_chainspec::EthChainSpec;

    #[test]
    fn test_parse_amoy() {
        let spec = BorChainSpecParser::parse("amoy").unwrap();
        assert_eq!(spec.chain_id(), 80002);
    }

    #[test]
    fn test_parse_polygon() {
        let spec = BorChainSpecParser::parse("polygon").unwrap();
        assert_eq!(spec.chain_id(), 137);
    }

    #[test]
    fn test_parse_mainnet_alias() {
        let spec = BorChainSpecParser::parse("mainnet").unwrap();
        assert_eq!(spec.chain_id(), 137);
    }

    #[test]
    fn test_parse_unknown_fails() {
        assert!(BorChainSpecParser::parse("nonexistent-chain").is_err());
    }
}
