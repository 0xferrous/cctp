use alloy_provider::Provider;
use alloy_sol_types::sol;
use cctp_rs::{CctpV2, TokenMessengerV2Contract};
use serde::Serialize;
use solana_sdk::{pubkey::Pubkey, signature::Signer as _};

sol! {
    #[allow(missing_docs)]
    #[allow(clippy::too_many_arguments)]
    #[sol(rpc)]
    contract RawTokenMessengerV2 {
        function depositForBurn(uint256 amount, uint32 destinationDomain, bytes32 mintRecipient, address burnToken, bytes32 destinationCaller, uint256 maxFee, uint32 minFinalityThreshold) public returns (uint64 nonce);
    }
}

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
    solana::{
        DepositForBurnAccountsV2, DepositForBurnParamsV2, build_deposit_for_burn_instruction_v2,
        build_signed_transaction, cctp_v2_deposit_for_burn_pdas, derive_keypair_from_mnemonic,
        evm_address_to_pubkey_bytes32, finality_threshold_for_speed, latest_blockhash,
        parse_pubkey_arg, send_and_confirm_transaction, solana_rpc_client, solana_usdc_mint,
    },
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

    match (source.kind, destination.kind) {
        (ChainKind::Evm, ChainKind::Evm) => run_evm_burn(args, source, destination).await,
        (ChainKind::Evm, ChainKind::Solana) => run_evm_to_solana_burn(args, source, destination).await,
        (ChainKind::Solana, ChainKind::Evm) => run_solana_burn(args, source, destination).await,
        _ => Err(CliError::InvalidInput(
            "execution currently supports EVM -> EVM, EVM -> Solana, and Solana -> EVM burn flows only".into(),
        )),
    }
}

async fn run_evm_burn(
    args: BurnArgs,
    source: crate::chain::ChainInfo,
    destination: crate::chain::ChainInfo,
) -> Result<()> {
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

    print_output(&out, args.json)
}

async fn run_evm_to_solana_burn(
    args: BurnArgs,
    source: crate::chain::ChainInfo,
    destination: crate::chain::ChainInfo,
) -> Result<()> {
    let amount_atomic = parse_usdc_amount(&args.amount)?;
    let recipient = parse_pubkey_arg("solana recipient token account", &args.recipient)?;
    let recipient_label = recipient.to_string();
    let recipient_bytes32 = alloy_primitives::FixedBytes::<32>::from(*recipient.as_array());
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

    let usdc = usdc_address(source)?;
    let token_messenger = TokenMessengerV2Contract::new(
        named_chain(source)?
            .token_messenger_v2_address()
            .map_err(|e| CliError::InvalidInput(format!("missing source token messenger: {e}")))?,
        provider.clone(),
    );

    let bridge = build_bridge(
        provider.clone(),
        provider.clone(),
        named_chain(source)?,
        named_chain(source)?,
        alloy_primitives::Address::ZERO,
        false,
        alloy_primitives::U256::ZERO,
    );
    let approval_tx_hash = bridge
        .ensure_approval(usdc, sender, amount_atomic)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    if let Some(tx_hash) = approval_tx_hash {
        wait_for_receipt(&provider, tx_hash, "approval").await?;
    }

    let tx_request = RawTokenMessengerV2::new(token_messenger.address(), provider.clone())
        .depositForBurn(
            amount_atomic,
            destination.cctp_domain,
            recipient_bytes32,
            usdc,
            alloy_primitives::FixedBytes::<32>::ZERO,
            max_fee,
            finality_threshold_for_speed(args.speed),
        )
        .from(sender)
        .into_transaction_request();
    let pending_tx = provider
        .send_transaction(tx_request)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    let burn_tx_hash = *pending_tx.tx_hash();
    wait_for_receipt(&provider, burn_tx_hash, "burn").await?;

    let out = BurnOutput {
        source_chain: source.name.to_owned(),
        destination_chain: destination.name.to_owned(),
        amount: format_usdc(amount_atomic),
        recipient: recipient_label,
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

    print_output(&out, args.json)
}

async fn run_solana_burn(
    args: BurnArgs,
    source: crate::chain::ChainInfo,
    destination: crate::chain::ChainInfo,
) -> Result<()> {
    let mnemonic = args
        .solana_wallet
        .signer
        .solana_mnemonic
        .as_deref()
        .ok_or_else(|| {
            CliError::InvalidInput("missing --solana-mnemonic for Solana burn".into())
        })?;
    let burn_token_account = parse_pubkey_arg(
        "solana token account",
        args.solana_wallet
            .solana_token_account
            .as_deref()
            .ok_or_else(|| {
                CliError::InvalidInput("missing --solana-token-account for Solana burn".into())
            })?,
    )?;
    let amount_atomic = parse_usdc_amount(&args.amount)?;
    let amount = u64::try_from(amount_atomic)
        .map_err(|_| CliError::InvalidInput("Solana burn amount exceeds u64 token units".into()))?;
    let recipient = parse_recipient(&args.recipient)?;
    let recipient_pubkey = evm_address_to_pubkey_bytes32(recipient);
    let signer = derive_keypair_from_mnemonic(
        mnemonic,
        Some(&args.solana_wallet.signer.solana_passphrase),
        args.solana_wallet.signer.solana_account_index,
    )?;
    let sender = signer.pubkey();
    let rpc_url = resolve_rpc_url(source, args.rpc.rpc_url.as_deref())?;
    let client = solana_rpc_client(rpc_url);
    let iris = iris_client(source.env);
    let (fast_transfer, max_fee) = max_fee_for_speed(
        args.speed,
        amount_atomic,
        source.cctp_domain,
        destination.cctp_domain,
        &iris,
    )
    .await?;
    let max_fee = u64::try_from(max_fee)
        .map_err(|_| CliError::InvalidInput("Solana max fee exceeds u64 token units".into()))?;
    let burn_token_mint = solana_usdc_mint(source.env)?;
    let pdas = cctp_v2_deposit_for_burn_pdas(&sender, &burn_token_mint, destination.cctp_domain)?;
    let event_keypair = solana_keypair::Keypair::new();
    let instruction = build_deposit_for_burn_instruction_v2(
        &DepositForBurnAccountsV2 {
            owner: sender,
            event_rent_payer: sender,
            burn_token_account,
            burn_token_mint,
            message_sent_event_data: event_keypair.pubkey(),
            pdas,
        },
        &DepositForBurnParamsV2 {
            amount,
            destination_domain: destination.cctp_domain,
            mint_recipient: recipient_pubkey,
            destination_caller: Pubkey::default(),
            max_fee,
            min_finality_threshold: finality_threshold_for_speed(args.speed),
        },
    )?;
    let recent_blockhash = latest_blockhash(&client)?;
    let tx = build_signed_transaction(&[instruction], &signer, recent_blockhash);
    let sent = send_and_confirm_transaction(&client, &tx)?;

    let out = BurnOutput {
        source_chain: source.name.to_owned(),
        destination_chain: destination.name.to_owned(),
        amount: format_usdc(amount_atomic),
        recipient: recipient.to_string(),
        approval_tx_hash: None,
        burn_tx_hash: sent.signature.to_string(),
        source_domain: source.cctp_domain,
        destination_domain: destination.cctp_domain,
        fast: fast_transfer,
        next_action: format!(
            "run `cctp attestation --from {} --tx {}`",
            args.from_chain, sent.signature
        ),
    };

    print_output(&out, args.json)
}

fn print_output(out: &BurnOutput, json: bool) -> Result<()> {
    if json {
        print_json(out)?;
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
