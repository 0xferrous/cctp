use alloy_provider::Provider;
use cctp_rs::{CctpV2, MessageTransmitterV2Contract};
use serde::Serialize;
use solana_sdk::signature::Signer as _;

use crate::{
    chain::{ChainKind, find_chain, infer_chain_by_domain},
    cli::ClaimArgs,
    commands::attestation::iris_client,
    error::{CliError, Result},
    execution::{
        decode_hex_bytes, load_wallet, named_chain, provider_with_wallet, resolve_rpc_url,
        wait_for_receipt,
    },
    output::print_json,
    solana::{
        build_receive_message_instruction_from_canonical_message_v2, build_signed_transaction,
        derive_keypair_from_mnemonic, latest_blockhash, send_and_confirm_transaction,
        solana_rpc_client,
    },
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
    if source.kind != ChainKind::Evm && source.kind != ChainKind::Solana {
        return Err(CliError::InvalidInput(
            "execution currently supports EVM or Solana source chains only".into(),
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

    if destination.kind != ChainKind::Evm && destination.kind != ChainKind::Solana {
        return Err(CliError::InvalidInput(
            "execution currently supports EVM or Solana destination chains only".into(),
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

    match destination.kind {
        ChainKind::Evm => {
            claim_to_evm(args, destination, nonce, message_bytes, attestation_bytes).await
        }
        ChainKind::Solana => {
            claim_to_solana(args, destination, nonce, message_bytes, attestation_bytes).await
        }
    }
}

async fn claim_to_evm(
    args: ClaimArgs,
    destination: crate::chain::ChainInfo,
    nonce: String,
    message_bytes: Vec<u8>,
    attestation_bytes: Vec<u8>,
) -> Result<()> {
    let (signer, sender) = load_wallet(&args.wallet, destination).await?;
    let destination_rpc = resolve_rpc_url(destination, args.rpc.rpc_url.as_deref())?;
    let provider = provider_with_wallet(&destination_rpc, signer)?;
    let destination_chain = named_chain(destination)?;
    let message_transmitter = MessageTransmitterV2Contract::new(
        destination_chain
            .message_transmitter_v2_address()
            .map_err(|e| {
                CliError::InvalidInput(format!("missing destination message transmitter: {e}"))
            })?,
        provider.clone(),
    );

    if message_transmitter
        .is_message_received(alloy_primitives::keccak256(&message_bytes).into())
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?
    {
        return print_output(
            &ClaimOutput {
                source_tx_hash: args.tx,
                destination_chain: destination.name.to_owned(),
                message_nonce: nonce,
                already_claimed: true,
                claim_tx_hash: None,
                final_state: "already_claimed".into(),
            },
            args.json,
        );
    }

    let tx_request = message_transmitter.receive_message_transaction(
        message_bytes.into(),
        attestation_bytes.into(),
        sender,
    );
    let pending_tx = provider
        .send_transaction(tx_request)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    let claim_tx_hash = *pending_tx.tx_hash();
    wait_for_receipt(&provider, claim_tx_hash, "claim").await?;

    print_output(
        &ClaimOutput {
            source_tx_hash: args.tx,
            destination_chain: destination.name.to_owned(),
            message_nonce: nonce,
            already_claimed: false,
            claim_tx_hash: Some(claim_tx_hash.to_string()),
            final_state: "claimed".into(),
        },
        args.json,
    )
}

async fn claim_to_solana(
    args: ClaimArgs,
    destination: crate::chain::ChainInfo,
    nonce: String,
    message_bytes: Vec<u8>,
    attestation_bytes: Vec<u8>,
) -> Result<()> {
    let mnemonic = args
        .solana_signer
        .solana_mnemonic
        .as_deref()
        .ok_or_else(|| {
            CliError::InvalidInput("missing --solana-mnemonic for Solana claim".into())
        })?;
    let signer = derive_keypair_from_mnemonic(
        mnemonic,
        Some(&args.solana_signer.solana_passphrase),
        args.solana_signer.solana_account_index,
    )?;
    let payer = signer.pubkey();
    let rpc_url = resolve_rpc_url(destination, args.rpc.rpc_url.as_deref())?;
    let client = solana_rpc_client(rpc_url);
    let instruction = build_receive_message_instruction_from_canonical_message_v2(
        &client,
        payer,
        destination.env,
        message_bytes.clone(),
        attestation_bytes,
    )?;
    let recent_blockhash = latest_blockhash(&client)?;
    let tx = build_signed_transaction(&[instruction], &signer, recent_blockhash);
    let sent = send_and_confirm_transaction(&client, &tx)?;

    print_output(
        &ClaimOutput {
            source_tx_hash: args.tx,
            destination_chain: destination.name.to_owned(),
            message_nonce: nonce,
            already_claimed: false,
            claim_tx_hash: Some(sent.signature.to_string()),
            final_state: "claimed".into(),
        },
        args.json,
    )
}

fn print_output(out: &ClaimOutput, json: bool) -> Result<()> {
    if json {
        print_json(out)?;
    } else {
        println!("Source tx: {}", out.source_tx_hash);
        println!("Destination chain: {}", out.destination_chain);
        println!("Nonce: {}", out.message_nonce);
        println!("Already claimed: {}", out.already_claimed);
        if let Some(tx_hash) = out.claim_tx_hash.as_deref() {
            println!("Claim tx: {tx_hash}");
        }
        println!("Final state: {}", out.final_state);
    }

    Ok(())
}
