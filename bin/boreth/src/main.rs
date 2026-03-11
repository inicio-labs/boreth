//! Boreth — Polygon Bor execution client built on Reth.

use bor_chainspec::BorChainSpecParser;
use clap::Parser;
use reth_ethereum_cli::interface::Cli;
use reth_node_ethereum::EthereumNode;
use reth_tracing::tracing::info;

fn main() {
    reth_cli_util::sigsegv_handler::install();

    if std::env::var_os("RUST_BACKTRACE").is_none() {
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }

    if let Err(err) =
        Cli::<BorChainSpecParser>::parse().run(async move |builder, _| {
            info!(target: "boreth", "Launching Boreth node");
            let handle = builder
                .node(EthereumNode::default())
                .launch_with_debug_capabilities()
                .await?;

            handle.wait_for_node_exit().await
        })
    {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
