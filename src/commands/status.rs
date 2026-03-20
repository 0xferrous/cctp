use circle_iris::{
    AttestationResponse, AttestationState, AttestationStatus, Environment, IrisClient,
    MessageLookup,
};
use serde::Serialize;

use crate::{
    chain::{BridgeEnvironment, find_chain, infer_chain_by_domain},
    claim::check_claimed_status,
    cli::StatusArgs,
    error::{CliError, Result},
    output::print_json,
};

#[derive(Debug, Serialize)]
struct StatusOutput<'a> {
    source_chain: &'a str,
    source_domain: u32,
    burn_tx_hash: String,
    phase: &'static str,
    attestation_status: &'static str,
    destination_domain: Option<u32>,
    destination_chain: Option<&'a str>,
    destination_kind: Option<&'a str>,
    nonce: Option<String>,
    amount: Option<String>,
    recipient: Option<String>,
    delay_reason: Option<String>,
    already_claimed: Option<bool>,
    next_action: &'static str,
}

pub async fn run(args: StatusArgs) -> Result<()> {
    let source = find_chain(&args.from_chain)?;
    let iris = IrisClient::new(iris_environment(source.env));
    let attestation = iris
        .attestation(source.cctp_domain, MessageLookup::TransactionHash(&args.tx))
        .await
        .map_err(|e| CliError::Iris(e.to_string()))?
        .into_state();

    let out = resolve_status(
        source.name,
        source.cctp_domain,
        args.tx,
        source.env,
        attestation,
    )
    .await?;

    if args.json {
        print_json(&out)?;
    } else {
        print_human(&out);
    }

    Ok(())
}

async fn resolve_status<'a>(
    source_chain: &'a str,
    source_domain: u32,
    burn_tx_hash: String,
    env: crate::chain::BridgeEnvironment,
    attestation: AttestationState,
) -> Result<StatusOutput<'a>> {
    Ok(match attestation {
        AttestationState::Pending(p) => {
            resolve_pending_status(source_chain, source_domain, burn_tx_hash, p)
        }
        AttestationState::Complete(c) => {
            let chain = infer_chain_by_domain(c.destination_domain, env);
            let already_claimed = if let Some(dest_chain) = chain {
                check_claimed_status(dest_chain, &c.nonce).await?
            } else {
                None
            };

            let (phase, next_action) = match already_claimed {
                Some(true) => ("claimed", "transfer already claimed on destination chain"),
                Some(false) => (
                    "ready_to_claim",
                    "run `cctp claim` once claim support is implemented",
                ),
                None => (
                    "ready_to_claim",
                    "claim readiness confirmed; destination-chain claim status unavailable",
                ),
            };

            StatusOutput {
                source_chain,
                source_domain,
                burn_tx_hash,
                phase,
                attestation_status: AttestationStatus::Complete.as_str(),
                destination_domain: Some(c.destination_domain),
                destination_chain: chain.map(|c| c.name),
                destination_kind: chain.map(|c| c.kind.as_str()),
                nonce: Some(c.nonce),
                amount: c.amount,
                recipient: c.mint_recipient,
                delay_reason: c.delay_reason,
                already_claimed,
                next_action,
            }
        }
    })
}

fn resolve_pending_status<'a>(
    source_chain: &'a str,
    source_domain: u32,
    burn_tx_hash: String,
    attestation: AttestationResponse,
) -> StatusOutput<'a> {
    StatusOutput {
        source_chain,
        source_domain,
        burn_tx_hash,
        phase: if attestation.status == AttestationStatus::PendingConfirmations {
            "awaiting_attestation"
        } else {
            "burn_submitted"
        },
        attestation_status: attestation.status.as_str(),
        destination_domain: None,
        destination_chain: None,
        destination_kind: None,
        nonce: attestation.nonce,
        amount: attestation.amount,
        recipient: attestation.mint_recipient,
        delay_reason: attestation.delay_reason,
        already_claimed: None,
        next_action: "wait for Circle Iris to produce a complete attestation",
    }
}

fn iris_environment(env: BridgeEnvironment) -> Environment {
    match env {
        BridgeEnvironment::Mainnet => Environment::Mainnet,
        BridgeEnvironment::Testnet => Environment::Testnet,
    }
}

fn print_human(out: &StatusOutput<'_>) {
    println!("Transfer status for {}", out.burn_tx_hash);
    println!(
        "Source: {} (domain {})",
        out.source_chain, out.source_domain
    );
    println!("Phase: {}", out.phase);
    println!("Attestation: {}", out.attestation_status);
    if let Some(domain) = out.destination_domain {
        println!("Destination domain: {domain}");
    }
    if let Some(chain) = out.destination_chain {
        println!("Destination chain: {chain}");
    }
    if let Some(kind) = out.destination_kind {
        println!("Destination kind: {kind}");
    }
    if let Some(claimed) = out.already_claimed {
        println!("Already claimed: {claimed}");
    }
    if let Some(nonce) = out.nonce.as_deref() {
        println!("Nonce: {nonce}");
    }
    if let Some(amount) = out.amount.as_deref() {
        println!("Amount: {amount}");
    }
    if let Some(recipient) = out.recipient.as_deref() {
        println!("Recipient: {recipient}");
    }
    if let Some(reason) = out.delay_reason.as_deref() {
        println!("Delay reason: {reason}");
    }
    println!("Next action: {}", out.next_action);
}
