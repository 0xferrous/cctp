use std::time::Duration;

use alloy_provider::Provider;
use alloy_sol_types::sol;
use cctp_rs::{CctpV2, MessageTransmitterV2Contract, TokenMessengerV2Contract};
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
    cli::BridgeArgs,
    commands::attestation::iris_client,
    error::{CliError, Result},
    execution::{
        build_bridge, decode_hex_bytes, load_wallet, max_fee_for_speed, named_chain,
        parse_recipient, polling_config, provider_with_wallet, resolve_rpc_url, usdc_address,
        wait_for_receipt,
    },
    output::print_json,
    solana::{
        DepositForBurnAccountsV2, DepositForBurnParamsV2, build_deposit_for_burn_instruction_v2,
        build_receive_message_instruction_from_canonical_message_v2, build_signed_transaction,
        cctp_v2_deposit_for_burn_pdas, derive_keypair_from_mnemonic, evm_address_to_pubkey_bytes32,
        finality_threshold_for_speed, latest_blockhash, parse_pubkey_arg,
        send_and_confirm_transaction as send_and_confirm_solana_transaction, solana_rpc_client,
        solana_usdc_mint,
    },
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

    match (source.kind, destination.kind) {
        (ChainKind::Evm, ChainKind::Evm) => run_evm_bridge(args, source, destination).await,
        (ChainKind::Evm, ChainKind::Solana) => {
            run_evm_to_solana_bridge(args, source, destination).await
        }
        (ChainKind::Solana, ChainKind::Evm) => run_solana_bridge(args, source, destination).await,
        _ => Err(CliError::InvalidInput(
            "execution currently supports EVM -> EVM, EVM -> Solana, and Solana -> EVM bridge flows only".into(),
        )),
    }
}

async fn run_evm_bridge(
    args: BridgeArgs,
    source: crate::chain::ChainInfo,
    destination: crate::chain::ChainInfo,
) -> Result<()> {
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
    let amount = amount_atomic;

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

    print_output(&out, args.json)
}

async fn run_evm_to_solana_bridge(
    args: BridgeArgs,
    source: crate::chain::ChainInfo,
    destination: crate::chain::ChainInfo,
) -> Result<()> {
    let amount_atomic = parse_usdc_amount(&args.amount)?;
    let recipient_token_account =
        parse_pubkey_arg("solana recipient token account", &args.recipient)?;
    let recipient_label = recipient_token_account.to_string();
    let recipient_bytes32 =
        alloy_primitives::FixedBytes::<32>::from(*recipient_token_account.as_array());

    let (evm_signer, evm_sender) = load_wallet(&args.wallet, source).await?;
    let source_rpc = resolve_rpc_url(source, args.rpc.rpc_url.as_deref())?;
    let source_provider = provider_with_wallet(&source_rpc, evm_signer)?;

    let mnemonic = args
        .solana_wallet
        .signer
        .solana_mnemonic
        .as_deref()
        .ok_or_else(|| {
            CliError::InvalidInput("missing --solana-mnemonic for Solana bridge".into())
        })?;
    let solana_signer = derive_keypair_from_mnemonic(
        mnemonic,
        Some(&args.solana_wallet.signer.solana_passphrase),
        args.solana_wallet.signer.solana_account_index,
    )?;
    let solana_payer = solana_signer.pubkey();
    let destination_rpc = resolve_rpc_url(destination, args.rpc.rpc_url.as_deref())?;
    let solana_client = solana_rpc_client(destination_rpc);

    let iris = iris_client(source.env);
    let (_fast_transfer, max_fee) = max_fee_for_speed(
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
        source_provider.clone(),
    );
    let approval_helper = build_bridge(
        source_provider.clone(),
        source_provider.clone(),
        named_chain(source)?,
        named_chain(source)?,
        alloy_primitives::Address::ZERO,
        false,
        alloy_primitives::U256::ZERO,
    );
    let approval_tx_hash = approval_helper
        .ensure_approval(usdc, evm_sender, amount_atomic)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    if let Some(tx_hash) = approval_tx_hash {
        wait_for_receipt(&source_provider, tx_hash, "approval").await?;
    }

    let tx_request = RawTokenMessengerV2::new(token_messenger.address(), source_provider.clone())
        .depositForBurn(
            amount_atomic,
            destination.cctp_domain,
            recipient_bytes32,
            usdc,
            alloy_primitives::FixedBytes::<32>::ZERO,
            max_fee,
            finality_threshold_for_speed(args.speed),
        )
        .from(evm_sender)
        .into_transaction_request();
    let pending_tx = source_provider
        .send_transaction(tx_request)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?;
    let burn_tx_hash = *pending_tx.tx_hash();
    wait_for_receipt(&source_provider, burn_tx_hash, "burn").await?;

    if args.no_wait {
        let out = BridgeOutput {
            source_chain: source.name.to_owned(),
            destination_chain: destination.name.to_owned(),
            amount: format_usdc(amount_atomic),
            recipient: recipient_label,
            approval_tx_hash: approval_tx_hash.map(|tx| tx.to_string()),
            burn_tx_hash: burn_tx_hash.to_string(),
            nonce: None,
            attestation_status: "burn_confirmed".into(),
            claim_tx_hash: None,
            final_phase: "burned".into(),
        };
        return print_output(&out, args.json);
    }

    let iris_attestation =
        wait_for_complete_attestation(&iris, source.cctp_domain, &burn_tx_hash.to_string()).await?;
    let nonce = iris_attestation.nonce.clone();
    let message_hex = iris_attestation
        .message
        .ok_or_else(|| CliError::InvalidInput("attestation message is not ready yet".into()))?;
    let attestation_hex = iris_attestation
        .attestation
        .ok_or_else(|| CliError::InvalidInput("attestation signature is not ready yet".into()))?;
    let message_bytes = decode_hex_bytes("message", &message_hex)?;
    let attestation_bytes = decode_hex_bytes("attestation", &attestation_hex)?;

    let instruction = build_receive_message_instruction_from_canonical_message_v2(
        &solana_client,
        solana_payer,
        destination.env,
        message_bytes,
        attestation_bytes,
    )?;
    let recent_blockhash = latest_blockhash(&solana_client)?;
    let claim_tx = build_signed_transaction(&[instruction], &solana_signer, recent_blockhash);
    let claim_sent = send_and_confirm_solana_transaction(&solana_client, &claim_tx)?;

    let out = BridgeOutput {
        source_chain: source.name.to_owned(),
        destination_chain: destination.name.to_owned(),
        amount: format_usdc(amount_atomic),
        recipient: recipient_label,
        approval_tx_hash: approval_tx_hash.map(|tx| tx.to_string()),
        burn_tx_hash: burn_tx_hash.to_string(),
        nonce,
        attestation_status: "complete".into(),
        claim_tx_hash: Some(claim_sent.signature.to_string()),
        final_phase: "claimed".into(),
    };

    print_output(&out, args.json)
}

async fn run_solana_bridge(
    args: BridgeArgs,
    source: crate::chain::ChainInfo,
    destination: crate::chain::ChainInfo,
) -> Result<()> {
    let mnemonic = args
        .solana_wallet
        .signer
        .solana_mnemonic
        .as_deref()
        .ok_or_else(|| {
            CliError::InvalidInput("missing --solana-mnemonic for Solana bridge".into())
        })?;
    let burn_token_account = parse_pubkey_arg(
        "solana token account",
        args.solana_wallet
            .solana_token_account
            .as_deref()
            .ok_or_else(|| {
                CliError::InvalidInput("missing --solana-token-account for Solana bridge".into())
            })?,
    )?;
    let amount_atomic = parse_usdc_amount(&args.amount)?;
    let amount = u64::try_from(amount_atomic)
        .map_err(|_| CliError::InvalidInput("Solana burn amount exceeds u64 token units".into()))?;
    let recipient = parse_recipient(&args.recipient)?;
    let recipient_pubkey = evm_address_to_pubkey_bytes32(recipient);
    let solana_signer = derive_keypair_from_mnemonic(
        mnemonic,
        Some(&args.solana_wallet.signer.solana_passphrase),
        args.solana_wallet.signer.solana_account_index,
    )?;
    let solana_sender = solana_signer.pubkey();
    let source_rpc = resolve_rpc_url(source, args.rpc.rpc_url.as_deref())?;
    let solana_client = solana_rpc_client(source_rpc);

    let (evm_signer, evm_sender) = load_wallet(&args.wallet, destination).await?;
    let destination_rpc = resolve_rpc_url(destination, args.rpc.rpc_url.as_deref())?;
    let destination_provider = provider_with_wallet(&destination_rpc, evm_signer)?;
    let destination_chain = named_chain(destination)?;
    let message_transmitter = MessageTransmitterV2Contract::new(
        destination_chain
            .message_transmitter_v2_address()
            .map_err(|e| {
                CliError::InvalidInput(format!("missing destination message transmitter: {e}"))
            })?,
        destination_provider.clone(),
    );

    let iris = iris_client(source.env);
    let (_fast_transfer, max_fee) = max_fee_for_speed(
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
    let pdas =
        cctp_v2_deposit_for_burn_pdas(&solana_sender, &burn_token_mint, destination.cctp_domain)?;
    let event_keypair = solana_keypair::Keypair::new();
    let instruction = build_deposit_for_burn_instruction_v2(
        &DepositForBurnAccountsV2 {
            owner: solana_sender,
            event_rent_payer: solana_sender,
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
    let recent_blockhash = latest_blockhash(&solana_client)?;
    let burn_tx = build_signed_transaction(&[instruction], &solana_signer, recent_blockhash);
    let burn_sent = send_and_confirm_solana_transaction(&solana_client, &burn_tx)?;

    if args.no_wait {
        let out = BridgeOutput {
            source_chain: source.name.to_owned(),
            destination_chain: destination.name.to_owned(),
            amount: format_usdc(amount_atomic),
            recipient: recipient.to_string(),
            approval_tx_hash: None,
            burn_tx_hash: burn_sent.signature.to_string(),
            nonce: None,
            attestation_status: "burn_confirmed".into(),
            claim_tx_hash: None,
            final_phase: "burned".into(),
        };
        return print_output(&out, args.json);
    }

    let iris_attestation =
        wait_for_complete_attestation(&iris, source.cctp_domain, &burn_sent.signature.to_string())
            .await?;
    let nonce = iris_attestation.nonce.clone();
    let message_hex = iris_attestation
        .message
        .ok_or_else(|| CliError::InvalidInput("attestation message is not ready yet".into()))?;
    let attestation_hex = iris_attestation
        .attestation
        .ok_or_else(|| CliError::InvalidInput("attestation signature is not ready yet".into()))?;
    let message_bytes = decode_hex_bytes("message", &message_hex)?;
    let attestation_bytes = decode_hex_bytes("attestation", &attestation_hex)?;

    let message_hash: [u8; 32] = alloy_primitives::keccak256(&message_bytes).into();
    let claim_tx_hash = if message_transmitter
        .is_message_received(message_hash)
        .await
        .map_err(|e| CliError::Rpc(e.to_string()))?
    {
        None
    } else {
        let tx_request = message_transmitter.receive_message_transaction(
            message_bytes.into(),
            attestation_bytes.into(),
            evm_sender,
        );
        let pending_tx = destination_provider
            .send_transaction(tx_request)
            .await
            .map_err(|e| CliError::Rpc(e.to_string()))?;
        let tx_hash = *pending_tx.tx_hash();
        wait_for_receipt(&destination_provider, tx_hash, "claim").await?;
        Some(tx_hash.to_string())
    };

    let out = BridgeOutput {
        source_chain: source.name.to_owned(),
        destination_chain: destination.name.to_owned(),
        amount: format_usdc(amount_atomic),
        recipient: recipient.to_string(),
        approval_tx_hash: None,
        burn_tx_hash: burn_sent.signature.to_string(),
        nonce,
        attestation_status: "complete".into(),
        claim_tx_hash: claim_tx_hash.clone(),
        final_phase: if claim_tx_hash.is_some() {
            "claimed".into()
        } else {
            "already_claimed".into()
        },
    };

    print_output(&out, args.json)
}

async fn wait_for_complete_attestation(
    iris: &circle_iris::IrisClient,
    source_domain: u32,
    tx_hash: &str,
) -> Result<circle_iris::AttestationResponse> {
    loop {
        let attestation = iris
            .attestation(
                source_domain,
                circle_iris::MessageLookup::TransactionHash(tx_hash),
            )
            .await
            .map_err(|e| CliError::Iris(e.to_string()))?;

        if attestation.status == circle_iris::AttestationStatus::Complete
            && attestation.message.is_some()
            && attestation.attestation.is_some()
        {
            return Ok(attestation);
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn print_output(out: &BridgeOutput, json: bool) -> Result<()> {
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
