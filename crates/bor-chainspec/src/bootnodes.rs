//! Bootnode records for Polygon networks.

/// Amoy testnet Bor bootnodes (enode URLs).
pub const AMOY_BOOTNODES: &[&str] = &[
    "enode://d40ab6b340be9f78179bd1ec7aa4df346d43dc1462d85fb44c5d43f595991d2ec215d7c778a7588906cb4edf175b3df231cecce090986a739678cd3c620bf580@34.89.255.109:30303",
    "enode://13abba15caa024325f2209d3566fa77cd864281dda4f73bca4296277bfd919ac68cef4dbb508028e0310a24f6f9e23c761fa41ac735cdc87efdee76d5ff985a7@34.185.137.160:30303",
    "enode://fc5bd3856a4ce6389eef1d6bc637ce7617e6ba8013f7d722d9878cf13f1c5a5a95a9e26ccb0b38bcc330343941ce117ab50db9f61e72ba450dd528a1184d8e6a@34.89.119.250:30303",
    "enode://945e11d11bdeed301fb23a5c05aae77bfdde39a8f70308131682a5d2fc1f080531314554afc78718a72ae25cc09be7833f760bf8681516b4315ed36217fa8dab@34.89.40.235:30303",
];

/// Amoy testnet DNS discovery enrtree URL.
pub const AMOY_DNS_DISCOVERY: &str =
    "enrtree://AKUEZKN7PSKVNR65FZDHECMKOJQSGPARGTPPBI7WS2VUL4EGR6XPC@amoy.polygon-peers.io";

/// Polygon PoS mainnet Bor bootnodes.
pub const MAINNET_BOOTNODES: &[&str] = &[
    // Mainnet bootnodes can be added when needed
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amoy_bootnodes_parse() {
        for bootnode in AMOY_BOOTNODES {
            assert!(bootnode.starts_with("enode://"), "Invalid bootnode: {bootnode}");
            assert!(bootnode.contains(":30303"), "Missing port: {bootnode}");
        }
    }

    #[test]
    fn test_amoy_dns_discovery() {
        assert!(AMOY_DNS_DISCOVERY.starts_with("enrtree://"));
    }
}
