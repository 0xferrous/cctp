use serde::Serialize;

use solana_keypair::Signer as _;

use crate::{
    cli::{DebugCommand, UsdcAtaArgs},
    env_from_testnet,
    error::Result,
    output::print_json,
    solana::{derive_keypair_from_mnemonic, solana_usdc_mint, usdc_associated_token_address},
};

#[derive(Debug, Serialize)]
struct UsdcAtaOutput {
    environment: String,
    owner: String,
    usdc_mint: String,
    usdc_ata: String,
}

pub async fn run(command: DebugCommand) -> Result<()> {
    match command {
        DebugCommand::UsdcAta(args) => run_usdc_ata(args),
    }
}

fn run_usdc_ata(args: UsdcAtaArgs) -> Result<()> {
    let mnemonic =
        args.signer.solana_mnemonic.as_deref().ok_or_else(|| {
            crate::error::CliError::InvalidInput("missing --solana-mnemonic".into())
        })?;
    let env = env_from_testnet(args.testnet);
    let signer = derive_keypair_from_mnemonic(
        mnemonic,
        Some(&args.signer.solana_passphrase),
        args.signer.solana_account_index,
    )?;
    let owner = signer.pubkey();
    let mint = solana_usdc_mint(env)?;
    let ata = usdc_associated_token_address(&owner, env)?;

    let out = UsdcAtaOutput {
        environment: env.as_str().to_owned(),
        owner: owner.to_string(),
        usdc_mint: mint.to_string(),
        usdc_ata: ata.to_string(),
    };

    if args.json {
        print_json(&out)?;
    } else {
        println!("Environment: {}", out.environment);
        println!("Owner: {}", out.owner);
        println!("USDC mint: {}", out.usdc_mint);
        println!("USDC ATA: {}", out.usdc_ata);
    }

    Ok(())
}
