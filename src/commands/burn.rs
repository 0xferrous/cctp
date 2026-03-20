use serde::Serialize;

use crate::{
    amount::{format_usdc, parse_usdc_amount},
    chain::{ChainKind, find_chain},
    cli::BurnArgs,
    commands::attestation::iris_client,
    error::{CliError, Result},
    execution::{
        build_bridge, load_wallet, max_fee_for_speed, named_chain, parse_recipient,
        provider_with_wallet, resolve_rpc_url, usdc_address, wait_for_receipt,
    },
    output::print_json,
};

#[derive(Debug, Serialize)]
struct BurnOutput {
    source_chain: String,
    destination_chain: String,
    amount: String,
    recipient: String,
    approval_tx_hash: Option<String>,
    burn_tx_hash: String,
    source_domain: u32,
    destination_domain: u32,
    fast: bool,
    next_action: String,
}

pub async fn run(args: BurnArgs) -> Result<()> {
    let source = find_chain(&args.from_chain)?;
    let destination = find_chain(&args.to_chain)?;
    if source.env != destination.env {
        return Err(CliError::InvalidInput(
            "source and destination chains must be in the same environment".into(),
        ));
    }
    if source.kind != ChainKind::Evm || destination.kind != ChainKind::Evm {
        return Err(CliError::InvalidInput(
            "execution currently supports EVM -> EVM only".into(),
        ));
    }

    let amount_atomic = parse_usdc_amount(&args.amount)?;
    let recipient = parse_recipient(&args.recipient)?;
    let (signer, sender) = load_wallet(&args.wallet, source).await?;
    let rpc_url = resolve_rpc_url(source, args.rpc.rpc_url.as_deref())?;
    let provider = provider_with_wallet(&rpc_url, signer)?;
    let iris = iris_client(source.env);
    let (fast_transfer, max_fee) = max_fee_for_speed(
        args.speed,
        amount_atomic,
        source.cctp_domain,
        destination.cctp_domain,
        &iris,
    )
    .await?;

    let bridge = build_bridge(
        provider.clone(),
        provider.clone(),
        named_chain(source)?,
        named_chain(destination)?,
        recipient,
        fast_transfer,
        max_fee,
    );

    let usdc = usdc_address(source)?;
    let amount = amount_atomic;

    let approval_tx_hash = bridge
        .ensure_approval(usdc, sender, amount)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    if let Some(tx_hash) = approval_tx_hash {
        wait_for_receipt(&provider, tx_hash, "approval").await?;
    }

    let burn_tx_hash = bridge
        .burn(amount, sender, usdc)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    wait_for_receipt(&provider, burn_tx_hash, "burn").await?;

    let out = BurnOutput {
        source_chain: source.name.to_owned(),
        destination_chain: destination.name.to_owned(),
        amount: format_usdc(amount_atomic),
        recipient: recipient.to_string(),
        approval_tx_hash: approval_tx_hash.map(|tx| tx.to_string()),
        burn_tx_hash: burn_tx_hash.to_string(),
        source_domain: source.cctp_domain,
        destination_domain: destination.cctp_domain,
        fast: fast_transfer,
        next_action: format!(
            "run `cctp claim --source-chain {} --destination-chain {} --tx {}`",
            args.from_chain, args.to_chain, burn_tx_hash
        ),
    };

    if args.json {
        print_json(&out)?;
    } else {
        println!("Source chain: {}", out.source_chain);
        println!("Destination chain: {}", out.destination_chain);
        println!("Amount: {} USDC", out.amount);
        println!("Recipient: {}", out.recipient);
        if let Some(tx_hash) = out.approval_tx_hash.as_deref() {
            println!("Approval tx: {tx_hash}");
        }
        println!("Burn tx: {}", out.burn_tx_hash);
        println!("Source domain: {}", out.source_domain);
        println!("Destination domain: {}", out.destination_domain);
        println!("Mode: {}", if out.fast { "fast" } else { "standard" });
        println!("Next action: {}", out.next_action);
    }

    Ok(())
}
