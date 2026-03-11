use bor_node::{BorNode, BorNodeConfig};

fn main() -> eyre::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let network = if args.iter().any(|a| a == "--amoy") {
        "amoy"
    } else {
        "mainnet"
    };

    let config = match network {
        "amoy" => BorNodeConfig::amoy(),
        _ => BorNodeConfig::mainnet(),
    };

    println!("boreth v{}", env!("CARGO_PKG_VERSION"));
    println!("Network: {:?}", config.network);
    println!("Chain ID: {}", config.chain_id());
    println!("Heimdall: {}", config.heimdall_url);
    println!("Data dir: {}", config.data_dir);

    let node = BorNode::new(config)?;
    println!("Node initialized (chain_id={})", node.chain_id());

    Ok(())
}
