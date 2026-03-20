use circle_iris::{AttestationResponse, Environment, IrisClient, MessageLookup};
use serde::Serialize;

use crate::{
    chain::{BridgeEnvironment, find_chain},
    cli::AttestationArgs,
    error::{CliError, Result},
    output::print_json,
};

#[derive(Debug, Serialize)]
struct AttestationOutput {
    status: &'static str,
    source_domain: Option<u32>,
    destination_domain: Option<u32>,
    nonce: Option<String>,
    amount: Option<String>,
    mint_recipient: Option<String>,
    delay_reason: Option<String>,
    burn_tx_hash: Option<String>,
    message: Option<String>,
    attestation: Option<String>,
}

pub async fn run(args: AttestationArgs) -> Result<()> {
    let source = find_chain(&args.from_chain)?;
    let iris = iris_client(source.env);

    let lookup = match (args.tx.as_deref(), args.nonce.as_deref()) {
        (Some(tx), None) => MessageLookup::TransactionHash(tx),
        (None, Some(nonce)) => MessageLookup::Nonce(nonce),
        _ => {
            return Err(CliError::InvalidInput(
                "pass exactly one of --tx or --nonce".into(),
            ));
        }
    };

    let attestation = iris
        .attestation(source.cctp_domain, lookup)
        .await
        .map_err(|e| CliError::Iris(e.to_string()))?;
    let out = to_output(attestation);

    if args.json {
        print_json(&out)?;
    } else {
        println!("Status: {}", out.status);
        if let Some(burn_tx_hash) = out.burn_tx_hash.as_deref() {
            println!("Burn tx: {burn_tx_hash}");
        }
        if let Some(source_domain) = out.source_domain {
            println!("Source domain: {source_domain}");
        }
        if let Some(destination_domain) = out.destination_domain {
            println!("Destination domain: {destination_domain}");
        }
        if let Some(nonce) = out.nonce.as_deref() {
            println!("Nonce: {nonce}");
        }
        if let Some(amount) = out.amount.as_deref() {
            println!("Amount: {amount}");
        }
        if let Some(recipient) = out.mint_recipient.as_deref() {
            println!("Mint recipient: {recipient}");
        }
        if let Some(reason) = out.delay_reason.as_deref() {
            println!("Delay reason: {reason}");
        }
        if let Some(message) = out.message.as_deref() {
            println!("Message: {message}");
        }
        if let Some(attestation) = out.attestation.as_deref() {
            println!("Attestation: {attestation}");
        }
    }

    Ok(())
}

pub(crate) fn iris_client(env: BridgeEnvironment) -> IrisClient {
    IrisClient::new(iris_environment(env))
}

fn iris_environment(env: BridgeEnvironment) -> Environment {
    match env {
        BridgeEnvironment::Mainnet => Environment::Mainnet,
        BridgeEnvironment::Testnet => Environment::Testnet,
    }
}

fn to_output(attestation: AttestationResponse) -> AttestationOutput {
    AttestationOutput {
        status: attestation.status.as_str(),
        source_domain: attestation.source_domain,
        destination_domain: attestation.destination_domain,
        nonce: attestation.nonce,
        amount: attestation.amount,
        mint_recipient: attestation.mint_recipient,
        delay_reason: attestation.delay_reason,
        burn_tx_hash: attestation.burn_tx_hash,
        message: attestation.message,
        attestation: attestation.attestation,
    }
}
