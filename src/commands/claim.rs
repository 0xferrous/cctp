use serde::Serialize;

use crate::{
    chain::{ChainKind, find_chain, infer_chain_by_domain},
    cli::ClaimArgs,
    commands::attestation::iris_client,
    error::{CliError, Result},
    execution::{
        build_bridge, decode_hex_bytes, load_wallet, named_chain, provider_with_wallet,
        resolve_rpc_url, wait_for_receipt,
    },
    output::print_json,
};

#[derive(Debug, Serialize)]
struct ClaimOutput {
    source_tx_hash: String,
    destination_chain: String,
    message_nonce: String,
    already_claimed: bool,
    claim_tx_hash: Option<String>,
    final_state: String,
}

pub async fn run(args: ClaimArgs) -> Result<()> {
    let source = find_chain(&args.from_chain)?;
    if source.kind != ChainKind::Evm {
        return Err(CliError::InvalidInput(
            "execution currently supports EVM source chains only".into(),
        ));
    }

    let iris = iris_client(source.env);
    let attestation = iris
        .attestation(
            source.cctp_domain,
            circle_iris::MessageLookup::TransactionHash(&args.tx),
        )
        .await
        .map_err(|e| CliError::Iris(e.to_string()))?;

    let destination = match args.to_chain.as_deref() {
        Some(chain) => {
            let destination = find_chain(chain)?;
            if Some(destination.cctp_domain) != attestation.destination_domain {
                return Err(CliError::InvalidInput(format!(
                    "destination chain {} does not match Iris destination domain {:?}",
                    destination.name, attestation.destination_domain
                )));
            }
            destination
        }
        None => infer_chain_by_domain(
            attestation.destination_domain.ok_or_else(|| {
                CliError::InvalidInput("attestation has no destination domain".into())
            })?,
            source.env,
        )
        .ok_or_else(|| {
            CliError::InvalidInput("could not infer destination chain from attestation".into())
        })?,
    };

    if destination.kind != ChainKind::Evm {
        return Err(CliError::InvalidInput(
            "execution currently supports EVM destination chains only".into(),
        ));
    }

    let message = attestation
        .message
        .ok_or_else(|| CliError::InvalidInput("attestation message is not ready yet".into()))?;
    let attestation_bytes = attestation
        .attestation
        .ok_or_else(|| CliError::InvalidInput("attestation signature is not ready yet".into()))?;
    let nonce = attestation
        .nonce
        .ok_or_else(|| CliError::InvalidInput("attestation nonce is missing".into()))?;

    let message_bytes = decode_hex_bytes("message", &message)?;
    let attestation_bytes = decode_hex_bytes("attestation", &attestation_bytes)?;

    let (signer, sender) = load_wallet(&args.wallet, destination).await?;
    let destination_rpc = resolve_rpc_url(destination, args.rpc.rpc_url.as_deref())?;
    let provider = provider_with_wallet(&destination_rpc, signer)?;

    let bridge = build_bridge(
        provider.clone(),
        provider.clone(),
        named_chain(source)?,
        named_chain(destination)?,
        sender,
        false,
        alloy_primitives::U256::ZERO,
    );

    if bridge
        .is_message_received(&message_bytes)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?
    {
        let out = ClaimOutput {
            source_tx_hash: args.tx,
            destination_chain: destination.name.to_owned(),
            message_nonce: nonce,
            already_claimed: true,
            claim_tx_hash: None,
            final_state: "already_claimed".into(),
        };

        if args.json {
            print_json(&out)?;
        } else {
            println!("Source tx: {}", out.source_tx_hash);
            println!("Destination chain: {}", out.destination_chain);
            println!("Nonce: {}", out.message_nonce);
            println!("Already claimed: true");
            println!("Final state: {}", out.final_state);
        }
        return Ok(());
    }

    let claim_tx_hash = bridge
        .mint(message_bytes, attestation_bytes, sender)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    wait_for_receipt(&provider, claim_tx_hash, "claim").await?;

    let out = ClaimOutput {
        source_tx_hash: args.tx,
        destination_chain: destination.name.to_owned(),
        message_nonce: nonce,
        already_claimed: false,
        claim_tx_hash: Some(claim_tx_hash.to_string()),
        final_state: "claimed".into(),
    };

    if args.json {
        print_json(&out)?;
    } else {
        println!("Source tx: {}", out.source_tx_hash);
        println!("Destination chain: {}", out.destination_chain);
        println!("Nonce: {}", out.message_nonce);
        println!("Already claimed: false");
        if let Some(tx_hash) = out.claim_tx_hash.as_deref() {
            println!("Claim tx: {tx_hash}");
        }
        println!("Final state: {}", out.final_state);
    }

    Ok(())
}
