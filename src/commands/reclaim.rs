use circle_iris::MessageLookup;
use serde::Serialize;
use solana_sdk::signature::Signer as _;

use crate::{
    chain::{ChainKind, find_chain},
    cli::ReclaimArgs,
    commands::attestation::iris_client,
    error::{CliError, Result},
    execution::{decode_hex_bytes, resolve_rpc_url},
    output::print_json,
    solana::{
        build_reclaim_event_account_instruction_v2, build_signed_transaction,
        derive_keypair_from_mnemonic, extract_message_sent_event_account_from_transaction,
        latest_blockhash, parse_pubkey_arg, send_and_confirm_transaction, solana_rpc_client,
    },
};

#[derive(Debug, Serialize)]
struct ReclaimOutput {
    source_tx_hash: String,
    event_account: String,
    reclaimed: bool,
    reclaim_tx_hash: Option<String>,
    final_state: String,
}

pub async fn run(args: ReclaimArgs) -> Result<()> {
    let source = find_chain(&args.from_chain)?;
    if source.kind != ChainKind::Solana {
        return Err(CliError::InvalidInput(
            "reclaim currently supports Solana source chains only".into(),
        ));
    }

    let iris = iris_client(source.env);
    let attestation = iris
        .attestation(source.cctp_domain, MessageLookup::TransactionHash(&args.tx))
        .await
        .map_err(|e| CliError::Iris(e.to_string()))?;

    let destination_message = attestation
        .message
        .ok_or_else(|| CliError::InvalidInput("attestation message is not ready yet".into()))?;
    let attestation = attestation
        .attestation
        .ok_or_else(|| CliError::InvalidInput("attestation signature is not ready yet".into()))?;

    let destination_message = decode_hex_bytes("message", &destination_message)?;
    let attestation = decode_hex_bytes("attestation", &attestation)?;

    let rpc_url = resolve_rpc_url(source, args.rpc.rpc_url.as_deref())?;
    let client = solana_rpc_client(rpc_url);
    let event_account = match args.event_account.as_deref() {
        Some(event_account) => parse_pubkey_arg("event account", event_account)?,
        None => extract_message_sent_event_account_from_transaction(&client, &args.tx)?,
    };

    let mnemonic = args
        .solana_signer
        .solana_mnemonic
        .as_deref()
        .ok_or_else(|| CliError::InvalidInput("missing --solana-mnemonic for reclaim".into()))?;
    let signer = derive_keypair_from_mnemonic(
        mnemonic,
        Some(&args.solana_signer.solana_passphrase),
        args.solana_signer.solana_account_index,
    )?;
    let payer = signer.pubkey();

    let instruction = build_reclaim_event_account_instruction_v2(
        payer,
        event_account,
        &crate::solana::ReclaimEventAccountParamsV2 {
            destination_message,
            attestation,
        },
    )?;
    let recent_blockhash = latest_blockhash(&client)?;
    let tx = build_signed_transaction(&[instruction], &signer, recent_blockhash);
    let sent = send_and_confirm_transaction(&client, &tx)?;

    print_output(
        &ReclaimOutput {
            source_tx_hash: args.tx,
            event_account: event_account.to_string(),
            reclaimed: true,
            reclaim_tx_hash: Some(sent.signature.to_string()),
            final_state: "reclaimed".into(),
        },
        args.json,
    )
}

fn print_output(out: &ReclaimOutput, json: bool) -> Result<()> {
    if json {
        print_json(out)?;
    } else {
        println!("Source tx: {}", out.source_tx_hash);
        println!("Event account: {}", out.event_account);
        println!("Reclaimed: {}", out.reclaimed);
        if let Some(tx_hash) = out.reclaim_tx_hash.as_deref() {
            println!("Reclaim tx: {tx_hash}");
        }
        println!("Final state: {}", out.final_state);
    }

    Ok(())
}
