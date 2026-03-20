mod amount;
mod chain;
mod claim;
mod cli;
mod commands;
mod error;
#[path = "execution.rs"]
mod execution;
mod output;

use clap::Parser;
use error::Result;

use crate::cli::{Cli, Command};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Chains(args) => commands::chains::run(args),
        Command::Estimate(args) => commands::estimate::run(args).await,
        Command::Attestation(args) => commands::attestation::run(args).await,
        Command::Status(args) => commands::status::run(args).await,
        Command::Burn(args) => commands::burn::run(args).await,
        Command::Claim(args) => commands::claim::run(args).await,
        Command::Bridge(args) => commands::bridge::run(args).await,
        Command::Reattest(args) => commands::reattest::run(args).await,
    }
}

pub(crate) fn env_from_testnet(testnet: bool) -> chain::BridgeEnvironment {
    if testnet {
        chain::BridgeEnvironment::Testnet
    } else {
        chain::BridgeEnvironment::Mainnet
    }
}
