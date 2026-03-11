//! Bor node configuration.

use url::Url;

/// Configuration for the Bor node.
#[derive(Debug, Clone)]
pub struct BorNodeConfig {
    /// The network to run on (mainnet or amoy).
    pub network: BorNetwork,
    /// Heimdall API endpoint URL.
    pub heimdall_url: Url,
    /// Data directory path.
    pub data_dir: String,
    /// RPC listen address.
    pub rpc_addr: String,
    /// RPC listen port.
    pub rpc_port: u16,
    /// P2P listen port.
    pub p2p_port: u16,
}

/// Which Bor network to connect to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorNetwork {
    /// Polygon PoS mainnet (chain ID 137).
    Mainnet,
    /// Polygon Amoy testnet (chain ID 80002).
    Amoy,
}

impl BorNodeConfig {
    /// Create a default config for mainnet.
    pub fn mainnet() -> Self {
        Self {
            network: BorNetwork::Mainnet,
            heimdall_url: Url::parse("https://heimdall-api.polygon.technology").unwrap(),
            data_dir: "~/.boreth".to_string(),
            rpc_addr: "127.0.0.1".to_string(),
            rpc_port: 8545,
            p2p_port: 30303,
        }
    }

    /// Create a default config for Amoy testnet.
    pub fn amoy() -> Self {
        Self {
            network: BorNetwork::Amoy,
            heimdall_url: Url::parse("https://heimdall-api-amoy.polygon.technology").unwrap(),
            data_dir: "~/.boreth-amoy".to_string(),
            rpc_addr: "127.0.0.1".to_string(),
            rpc_port: 8545,
            p2p_port: 30303,
        }
    }

    /// Get the chain ID for this network.
    pub fn chain_id(&self) -> u64 {
        match self.network {
            BorNetwork::Mainnet => bor_chainspec::constants::MAINNET_CHAIN_ID,
            BorNetwork::Amoy => bor_chainspec::constants::AMOY_CHAIN_ID,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mainnet_config() {
        let config = BorNodeConfig::mainnet();
        assert_eq!(config.chain_id(), 137);
        assert_eq!(config.network, BorNetwork::Mainnet);
    }

    #[test]
    fn test_amoy_config() {
        let config = BorNodeConfig::amoy();
        assert_eq!(config.chain_id(), 80002);
        assert_eq!(config.network, BorNetwork::Amoy);
    }
}
