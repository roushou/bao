//! bao — supervise your AI coding agents.

use clap::Parser;

mod commands;

use commands::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
