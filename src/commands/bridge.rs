use serde::Serialize;

use crate::{
    amount::{format_usdc, parse_usdc_amount},
    chain::{ChainKind, find_chain},
    cli::BridgeArgs,
    commands::attestation::iris_client,
    error::{CliError, Result},
    execution::{
        build_bridge, load_wallet, max_fee_for_speed, named_chain, parse_recipient, polling_config,
        resolve_rpc_url, usdc_address, wait_for_receipt,
    },
    output::print_json,
};

#[derive(Debug, Serialize)]
struct BridgeOutput {
    source_chain: String,
    destination_chain: String,
    amount: String,
    recipient: String,
    approval_tx_hash: Option<String>,
    burn_tx_hash: String,
    nonce: Option<String>,
    attestation_status: String,
    claim_tx_hash: Option<String>,
    final_phase: String,
}

pub async fn run(args: BridgeArgs) -> Result<()> {
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
    let source_rpc = resolve_rpc_url(source, args.rpc.rpc_url.as_deref())?;
    let destination_rpc = resolve_rpc_url(destination, args.rpc.rpc_url.as_deref())?;
    let wallet = alloy_network::EthereumWallet::new(signer);
    let source_provider = alloy_provider::ProviderBuilder::new()
        .wallet(wallet.clone())
        .connect_http(
            source_rpc
                .parse()
                .map_err(|e| CliError::InvalidInput(format!("invalid source rpc url: {e}")))?,
        );
    let destination_provider =
        alloy_provider::ProviderBuilder::new()
            .wallet(wallet)
            .connect_http(destination_rpc.parse().map_err(|e| {
                CliError::InvalidInput(format!("invalid destination rpc url: {e}"))
            })?);

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
        source_provider.clone(),
        destination_provider.clone(),
        named_chain(source)?,
        named_chain(destination)?,
        recipient,
        fast_transfer,
        max_fee,
    );

    let usdc = usdc_address(source)?;
    let amount = alloy_primitives::U256::from(amount_atomic);

    let approval_tx_hash = bridge
        .ensure_approval(usdc, sender, amount)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    if let Some(tx_hash) = approval_tx_hash {
        wait_for_receipt(&source_provider, tx_hash, "approval").await?;
    }

    let burn_tx_hash = bridge
        .burn(amount, sender, usdc)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    wait_for_receipt(&source_provider, burn_tx_hash, "burn").await?;

    if args.no_wait {
        let out = BridgeOutput {
            source_chain: source.name.to_owned(),
            destination_chain: destination.name.to_owned(),
            amount: format_usdc(amount_atomic),
            recipient: recipient.to_string(),
            approval_tx_hash: approval_tx_hash.map(|tx| tx.to_string()),
            burn_tx_hash: burn_tx_hash.to_string(),
            nonce: None,
            attestation_status: "burn_confirmed".into(),
            claim_tx_hash: None,
            final_phase: "burned".into(),
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
            println!("Final phase: {}", out.final_phase);
        }
        return Ok(());
    }

    let (message, attestation) = bridge
        .get_attestation(burn_tx_hash, polling_config(args.speed))
        .await
        .map_err(|e| CliError::Iris(e.to_string()))?;

    let iris_attestation = iris
        .attestation(
            source.cctp_domain,
            circle_iris::MessageLookup::TransactionHash(&burn_tx_hash.to_string()),
        )
        .await
        .map_err(|e| CliError::Iris(e.to_string()))?;
    let nonce = iris_attestation.nonce.clone();

    let mint_result = bridge
        .mint_if_needed(message, attestation, sender)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;

    let (claim_tx_hash, final_phase) = match mint_result {
        cctp_rs::MintResult::Minted(tx_hash) => {
            wait_for_receipt(&destination_provider, tx_hash, "claim").await?;
            (Some(tx_hash.to_string()), "claimed".to_owned())
        }
        cctp_rs::MintResult::AlreadyRelayed => (None, "already_claimed".to_owned()),
    };

    let out = BridgeOutput {
        source_chain: source.name.to_owned(),
        destination_chain: destination.name.to_owned(),
        amount: format_usdc(amount_atomic),
        recipient: recipient.to_string(),
        approval_tx_hash: approval_tx_hash.map(|tx| tx.to_string()),
        burn_tx_hash: burn_tx_hash.to_string(),
        nonce,
        attestation_status: "complete".into(),
        claim_tx_hash,
        final_phase,
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
        if let Some(nonce) = out.nonce.as_deref() {
            println!("Nonce: {nonce}");
        }
        println!("Attestation: {}", out.attestation_status);
        if let Some(tx_hash) = out.claim_tx_hash.as_deref() {
            println!("Claim tx: {tx_hash}");
        }
        println!("Final phase: {}", out.final_phase);
    }

    Ok(())
}
