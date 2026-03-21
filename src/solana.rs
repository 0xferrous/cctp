#![allow(dead_code)]

use std::str::FromStr;

use alloy_primitives::Address;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{
        CommitmentConfig, CommitmentLevel, RpcSendTransactionConfig, RpcSimulateTransactionConfig,
    },
};
use solana_derivation_path::DerivationPath;
use solana_keypair::{Keypair, seed_derivable::keypair_from_seed_and_derivation_path};
use solana_sdk::{
    hash::{Hash, hash},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Signature, Signer},
    transaction::Transaction,
};
use solana_seed_phrase::generate_seed_from_seed_phrase_and_passphrase;

use crate::{
    chain::BridgeEnvironment,
    cli::TransferSpeedArg,
    error::{CliError, Result},
};

const SOLANA_CHANGE_INDEX: u32 = 0;

const MESSAGE_TRANSMITTER_V2_PROGRAM_ID: &str = "CCTPV2Sm4AdWt5296sk4P66VBZ7bEhcARwFaaS9YPbeC";
const TOKEN_MESSENGER_MINTER_V2_PROGRAM_ID: &str = "CCTPV2vPZJS2u2BBsUoscuikbYjnpFmbFsvVuJdgUMQe";
const SOLANA_MAINNET_USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const SOLANA_DEVNET_USDC_MINT: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID: &str =
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const FINALITY_THRESHOLD_FAST: u32 = 1_000;
const FINALITY_THRESHOLD_STANDARD: u32 = 2_000;

const SEED_MESSAGE_TRANSMITTER: &[u8] = b"message_transmitter";
const SEED_TOKEN_MESSENGER: &[u8] = b"token_messenger";
const SEED_TOKEN_MINTER: &[u8] = b"token_minter";
const SEED_LOCAL_TOKEN: &[u8] = b"local_token";
const SEED_REMOTE_TOKEN_MESSENGER: &[u8] = b"remote_token_messenger";
const SEED_USED_NONCE: &[u8] = b"used_nonce";
const SEED_TOKEN_PAIR: &[u8] = b"token_pair";
const SEED_CUSTODY: &[u8] = b"custody";
const SEED_SENDER_AUTHORITY: &[u8] = b"sender_authority";
const SEED_DENYLIST_ACCOUNT: &[u8] = b"denylist_account";
const SEED_MESSAGE_TRANSMITTER_AUTHORITY: &[u8] = b"message_transmitter_authority";
const SEED_EVENT_AUTHORITY: &[u8] = b"__event_authority";

#[derive(Clone, Debug)]
pub(crate) struct SolanaCctpV2Programs {
    pub message_transmitter: Pubkey,
    pub token_messenger_minter: Pubkey,
}

#[derive(Clone, Debug)]
pub(crate) struct SolanaDepositForBurnPdas {
    pub sender_authority: Pubkey,
    pub denylist_account: Pubkey,
    pub message_transmitter: Pubkey,
    pub token_messenger: Pubkey,
    pub remote_token_messenger: Pubkey,
    pub token_minter: Pubkey,
    pub local_token: Pubkey,
}

#[derive(Clone, Debug)]
pub(crate) struct SolanaReceiveMessagePdas {
    pub authority: Pubkey,
    pub used_nonce: Pubkey,
    pub message_transmitter: Pubkey,
    pub token_messenger: Pubkey,
    pub remote_token_messenger: Pubkey,
    pub token_minter: Pubkey,
    pub local_token: Pubkey,
    pub token_pair: Pubkey,
    pub custody_token_account: Pubkey,
    pub token_messenger_event_authority: Pubkey,
}

#[derive(Clone, Debug)]
pub(crate) struct DepositForBurnParamsV2 {
    pub amount: u64,
    pub destination_domain: u32,
    pub mint_recipient: Pubkey,
    pub destination_caller: Pubkey,
    pub max_fee: u64,
    pub min_finality_threshold: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ReceiveMessageParamsV2 {
    pub message: Vec<u8>,
    pub attestation: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct DepositForBurnAccountsV2 {
    pub owner: Pubkey,
    pub event_rent_payer: Pubkey,
    pub burn_token_account: Pubkey,
    pub burn_token_mint: Pubkey,
    pub message_sent_event_data: Pubkey,
    pub pdas: SolanaDepositForBurnPdas,
}

#[derive(Clone, Debug)]
pub(crate) struct ReceiveMessageAccountsV2 {
    pub payer: Pubkey,
    pub caller: Pubkey,
    pub receiver: Pubkey,
    pub pdas: SolanaReceiveMessagePdas,
    pub remaining_accounts: Vec<AccountMeta>,
}

#[derive(Clone, Debug)]
pub(crate) struct SimulatedTransaction {
    pub logs: Vec<String>,
    pub units_consumed: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct SentTransaction {
    pub signature: Signature,
}

pub(crate) fn derivation_path(account_index: u32) -> DerivationPath {
    DerivationPath::new_bip44(Some(account_index), Some(SOLANA_CHANGE_INDEX))
}

pub(crate) fn derive_keypair_from_mnemonic(
    mnemonic: &str,
    passphrase: Option<&str>,
    account_index: u32,
) -> Result<Keypair> {
    let mnemonic = mnemonic.trim();
    if mnemonic.is_empty() {
        return Err(CliError::InvalidInput("mnemonic is required".into()));
    }

    let seed = generate_seed_from_seed_phrase_and_passphrase(mnemonic, passphrase.unwrap_or(""));
    let path = derivation_path(account_index);
    keypair_from_seed_and_derivation_path(&seed, Some(path))
        .map_err(|e| CliError::Wallet(format!("failed to derive Solana keypair: {e}")))
}

pub(crate) fn pubkey_string(
    mnemonic: &str,
    passphrase: Option<&str>,
    account_index: u32,
) -> Result<String> {
    Ok(
        derive_keypair_from_mnemonic(mnemonic, passphrase, account_index)?
            .pubkey()
            .to_string(),
    )
}

pub(crate) fn solana_rpc_client(rpc_url: impl Into<String>) -> RpcClient {
    RpcClient::new_with_commitment(rpc_url.into(), CommitmentConfig::confirmed())
}

pub(crate) fn latest_blockhash(client: &RpcClient) -> Result<Hash> {
    client
        .get_latest_blockhash()
        .map_err(|e| CliError::Rpc(format!("failed to fetch latest Solana blockhash: {e}")))
}

pub(crate) fn build_signed_transaction(
    instructions: &[Instruction],
    signer: &Keypair,
    recent_blockhash: Hash,
) -> Transaction {
    Transaction::new_signed_with_payer(
        instructions,
        Some(&signer.pubkey()),
        &[signer],
        recent_blockhash,
    )
}

pub(crate) fn sign_transaction(
    transaction: &mut Transaction,
    signer: &Keypair,
    recent_blockhash: Hash,
) {
    transaction.sign(&[signer], recent_blockhash);
}

pub(crate) fn simulate_transaction(
    client: &RpcClient,
    transaction: &Transaction,
) -> Result<SimulatedTransaction> {
    let response = client
        .simulate_transaction_with_config(
            transaction,
            RpcSimulateTransactionConfig {
                sig_verify: true,
                commitment: Some(CommitmentConfig::confirmed()),
                ..RpcSimulateTransactionConfig::default()
            },
        )
        .map_err(|e| CliError::Rpc(format!("failed to simulate Solana transaction: {e}")))?;

    if let Some(err) = response.value.err {
        let logs = response.value.logs.unwrap_or_default().join("\n");
        return Err(CliError::Rpc(format!(
            "Solana transaction simulation failed: {err}. logs:\n{logs}"
        )));
    }

    Ok(SimulatedTransaction {
        logs: response.value.logs.unwrap_or_default(),
        units_consumed: response.value.units_consumed,
    })
}

pub(crate) fn send_transaction(
    client: &RpcClient,
    transaction: &Transaction,
) -> Result<SentTransaction> {
    let signature = client
        .send_transaction_with_config(
            transaction,
            RpcSendTransactionConfig {
                preflight_commitment: Some(CommitmentLevel::Confirmed),
                ..RpcSendTransactionConfig::default()
            },
        )
        .map_err(|e| CliError::Rpc(format!("failed to send Solana transaction: {e}")))?;

    Ok(SentTransaction { signature })
}

pub(crate) fn send_and_confirm_transaction(
    client: &RpcClient,
    transaction: &Transaction,
) -> Result<SentTransaction> {
    let signature = client
        .send_and_confirm_transaction(transaction)
        .map_err(|e| CliError::Rpc(format!("failed to send/confirm Solana transaction: {e}")))?;

    Ok(SentTransaction { signature })
}

pub(crate) fn cctp_v2_programs() -> Result<SolanaCctpV2Programs> {
    Ok(SolanaCctpV2Programs {
        message_transmitter: parse_pubkey(MESSAGE_TRANSMITTER_V2_PROGRAM_ID)?,
        token_messenger_minter: parse_pubkey(TOKEN_MESSENGER_MINTER_V2_PROGRAM_ID)?,
    })
}

pub(crate) fn solana_usdc_mint(env: BridgeEnvironment) -> Result<Pubkey> {
    match env {
        BridgeEnvironment::Mainnet => parse_pubkey(SOLANA_MAINNET_USDC_MINT),
        BridgeEnvironment::Testnet => parse_pubkey(SOLANA_DEVNET_USDC_MINT),
    }
}

pub(crate) fn finality_threshold_for_speed(speed: TransferSpeedArg) -> u32 {
    match speed {
        TransferSpeedArg::Fast => FINALITY_THRESHOLD_FAST,
        TransferSpeedArg::Standard => FINALITY_THRESHOLD_STANDARD,
    }
}

pub(crate) fn evm_address_to_pubkey_bytes32(address: Address) -> Pubkey {
    let mut bytes = [0u8; 32];
    bytes[12..].copy_from_slice(address.as_slice());
    Pubkey::new_from_array(bytes)
}

pub(crate) fn parse_pubkey_arg(label: &str, value: &str) -> Result<Pubkey> {
    Pubkey::from_str(value)
        .map_err(|e| CliError::InvalidInput(format!("invalid {label} pubkey: {e}")))
}

pub(crate) fn parse_message_nonce(message: &[u8]) -> Result<[u8; 32]> {
    read_array(message, 12)
}

pub(crate) fn parse_message_source_domain(message: &[u8]) -> Result<u32> {
    read_u32_be(message, 4)
}

pub(crate) fn parse_message_destination_domain(message: &[u8]) -> Result<u32> {
    read_u32_be(message, 8)
}

pub(crate) fn parse_message_body(message: &[u8]) -> Result<&[u8]> {
    message
        .get(148..)
        .ok_or_else(|| CliError::InvalidInput("malformed Solana CCTP message".into()))
}

pub(crate) fn parse_burn_message_remote_token(message_body: &[u8]) -> Result<Pubkey> {
    Ok(Pubkey::new_from_array(read_array(message_body, 4)?))
}

pub(crate) fn parse_burn_message_mint_recipient(message_body: &[u8]) -> Result<Pubkey> {
    Ok(Pubkey::new_from_array(read_array(message_body, 36)?))
}

pub(crate) fn parse_token_messenger_fee_recipient(account_data: &[u8]) -> Result<Pubkey> {
    Ok(Pubkey::new_from_array(read_array(account_data, 109)?))
}

pub(crate) fn associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Result<Pubkey> {
    let ata_program = parse_pubkey(SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID)?;
    let token_program = parse_pubkey(SPL_TOKEN_PROGRAM_ID)?;
    Ok(find_pda(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ata_program,
    ))
}

pub(crate) fn build_receive_message_instruction_from_canonical_message_v2(
    client: &RpcClient,
    payer: Pubkey,
    destination_env: BridgeEnvironment,
    message: Vec<u8>,
    attestation: Vec<u8>,
) -> Result<Instruction> {
    let local_token_mint = solana_usdc_mint(destination_env)?;
    let message_body = parse_message_body(&message)?;
    let remote_token = parse_burn_message_remote_token(message_body)?;
    let recipient_token_account = parse_burn_message_mint_recipient(message_body)?;
    let nonce = parse_message_nonce(&message)?;
    let remote_domain = parse_message_source_domain(&message)?;
    let pdas =
        cctp_v2_receive_message_pdas(&local_token_mint, &remote_token, remote_domain, &nonce)?;
    let token_messenger_data = client
        .get_account_data(&pdas.token_messenger)
        .map_err(|e| {
            CliError::Rpc(format!(
                "failed to fetch Solana token messenger account: {e}"
            ))
        })?;
    let fee_recipient = parse_token_messenger_fee_recipient(&token_messenger_data)?;
    let fee_recipient_token_account = associated_token_address(&fee_recipient, &local_token_mint)?;
    let programs = cctp_v2_programs()?;
    let token_program = parse_pubkey(SPL_TOKEN_PROGRAM_ID)?;

    build_receive_message_instruction_v2(
        &ReceiveMessageAccountsV2 {
            payer,
            caller: payer,
            receiver: programs.token_messenger_minter,
            pdas,
            remaining_accounts: vec![
                AccountMeta::new_readonly(
                    token_messenger_pda(&programs.token_messenger_minter),
                    false,
                ),
                AccountMeta::new_readonly(
                    remote_token_messenger_pda(&programs.token_messenger_minter, remote_domain),
                    false,
                ),
                AccountMeta::new(token_minter_pda(&programs.token_messenger_minter), false),
                AccountMeta::new(
                    local_token_pda(&programs.token_messenger_minter, &local_token_mint),
                    false,
                ),
                AccountMeta::new_readonly(
                    token_pair_pda(
                        &programs.token_messenger_minter,
                        remote_domain,
                        &remote_token,
                    ),
                    false,
                ),
                AccountMeta::new(fee_recipient_token_account, false),
                AccountMeta::new(recipient_token_account, false),
                AccountMeta::new(
                    custody_pda(&programs.token_messenger_minter, &local_token_mint),
                    false,
                ),
                AccountMeta::new_readonly(token_program, false),
                AccountMeta::new_readonly(
                    event_authority_pda(&programs.token_messenger_minter),
                    false,
                ),
                AccountMeta::new_readonly(programs.token_messenger_minter, false),
            ],
        },
        &ReceiveMessageParamsV2 {
            message,
            attestation,
        },
    )
}

pub(crate) fn cctp_v2_deposit_for_burn_pdas(
    owner: &Pubkey,
    burn_token_mint: &Pubkey,
    destination_domain: u32,
) -> Result<SolanaDepositForBurnPdas> {
    let programs = cctp_v2_programs()?;

    Ok(SolanaDepositForBurnPdas {
        sender_authority: sender_authority_pda(&programs.token_messenger_minter),
        denylist_account: denylist_account_pda(&programs.token_messenger_minter, owner),
        message_transmitter: message_transmitter_pda(&programs.message_transmitter),
        token_messenger: token_messenger_pda(&programs.token_messenger_minter),
        remote_token_messenger: remote_token_messenger_pda(
            &programs.token_messenger_minter,
            destination_domain,
        ),
        token_minter: token_minter_pda(&programs.token_messenger_minter),
        local_token: local_token_pda(&programs.token_messenger_minter, burn_token_mint),
    })
}

pub(crate) fn cctp_v2_receive_message_pdas(
    local_token_mint: &Pubkey,
    remote_token: &Pubkey,
    remote_domain: u32,
    nonce: &[u8],
) -> Result<SolanaReceiveMessagePdas> {
    let programs = cctp_v2_programs()?;

    Ok(SolanaReceiveMessagePdas {
        authority: message_transmitter_authority_pda(
            &programs.message_transmitter,
            &programs.token_messenger_minter,
        ),
        used_nonce: used_nonce_pda(&programs.message_transmitter, nonce),
        message_transmitter: message_transmitter_pda(&programs.message_transmitter),
        token_messenger: token_messenger_pda(&programs.token_messenger_minter),
        remote_token_messenger: remote_token_messenger_pda(
            &programs.token_messenger_minter,
            remote_domain,
        ),
        token_minter: token_minter_pda(&programs.token_messenger_minter),
        local_token: local_token_pda(&programs.token_messenger_minter, local_token_mint),
        token_pair: token_pair_pda(
            &programs.token_messenger_minter,
            remote_domain,
            remote_token,
        ),
        custody_token_account: custody_pda(&programs.token_messenger_minter, local_token_mint),
        token_messenger_event_authority: event_authority_pda(&programs.token_messenger_minter),
    })
}

pub(crate) fn message_transmitter_pda(program_id: &Pubkey) -> Pubkey {
    find_pda(&[SEED_MESSAGE_TRANSMITTER], program_id)
}

pub(crate) fn token_messenger_pda(program_id: &Pubkey) -> Pubkey {
    find_pda(&[SEED_TOKEN_MESSENGER], program_id)
}

pub(crate) fn token_minter_pda(program_id: &Pubkey) -> Pubkey {
    find_pda(&[SEED_TOKEN_MINTER], program_id)
}

pub(crate) fn local_token_pda(program_id: &Pubkey, mint: &Pubkey) -> Pubkey {
    find_pda(&[SEED_LOCAL_TOKEN, mint.as_ref()], program_id)
}

pub(crate) fn remote_token_messenger_pda(program_id: &Pubkey, domain: u32) -> Pubkey {
    let domain = domain.to_string();
    find_pda(
        &[SEED_REMOTE_TOKEN_MESSENGER, domain.as_bytes()],
        program_id,
    )
}

pub(crate) fn used_nonce_pda(program_id: &Pubkey, nonce: &[u8]) -> Pubkey {
    find_pda(&[SEED_USED_NONCE, nonce], program_id)
}

pub(crate) fn token_pair_pda(
    program_id: &Pubkey,
    remote_domain: u32,
    remote_token: &Pubkey,
) -> Pubkey {
    let remote_domain = remote_domain.to_string();
    find_pda(
        &[
            SEED_TOKEN_PAIR,
            remote_domain.as_bytes(),
            remote_token.as_ref(),
        ],
        program_id,
    )
}

pub(crate) fn custody_pda(program_id: &Pubkey, local_mint: &Pubkey) -> Pubkey {
    find_pda(&[SEED_CUSTODY, local_mint.as_ref()], program_id)
}

pub(crate) fn sender_authority_pda(program_id: &Pubkey) -> Pubkey {
    find_pda(&[SEED_SENDER_AUTHORITY], program_id)
}

pub(crate) fn denylist_account_pda(program_id: &Pubkey, owner: &Pubkey) -> Pubkey {
    find_pda(&[SEED_DENYLIST_ACCOUNT, owner.as_ref()], program_id)
}

pub(crate) fn message_transmitter_authority_pda(
    message_transmitter_program_id: &Pubkey,
    receiver_program_id: &Pubkey,
) -> Pubkey {
    find_pda(
        &[
            SEED_MESSAGE_TRANSMITTER_AUTHORITY,
            receiver_program_id.as_ref(),
        ],
        message_transmitter_program_id,
    )
}

pub(crate) fn event_authority_pda(program_id: &Pubkey) -> Pubkey {
    find_pda(&[SEED_EVENT_AUTHORITY], program_id)
}

pub(crate) fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash(format!("global:{name}").as_bytes()).to_bytes()[..8]);
    bytes
}

pub(crate) fn encode_deposit_for_burn_data_v2(params: &DepositForBurnParamsV2) -> Vec<u8> {
    let mut data = Vec::with_capacity(96);
    data.extend_from_slice(&anchor_discriminator("deposit_for_burn"));
    data.extend_from_slice(&params.amount.to_le_bytes());
    data.extend_from_slice(&params.destination_domain.to_le_bytes());
    data.extend_from_slice(params.mint_recipient.as_ref());
    data.extend_from_slice(params.destination_caller.as_ref());
    data.extend_from_slice(&params.max_fee.to_le_bytes());
    data.extend_from_slice(&params.min_finality_threshold.to_le_bytes());
    data
}

pub(crate) fn encode_receive_message_data_v2(params: &ReceiveMessageParamsV2) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(8 + 4 + params.message.len() + 4 + params.attestation.len());
    data.extend_from_slice(&anchor_discriminator("receive_message"));
    extend_borsh_bytes(&mut data, &params.message)?;
    extend_borsh_bytes(&mut data, &params.attestation)?;
    Ok(data)
}

pub(crate) fn build_deposit_for_burn_instruction_v2(
    accounts: &DepositForBurnAccountsV2,
    params: &DepositForBurnParamsV2,
) -> Result<Instruction> {
    let programs = cctp_v2_programs()?;
    let token_program = parse_pubkey(SPL_TOKEN_PROGRAM_ID)?;
    let system_program = parse_pubkey(SYSTEM_PROGRAM_ID)?;

    Ok(Instruction {
        program_id: programs.token_messenger_minter,
        accounts: vec![
            AccountMeta::new_readonly(accounts.owner, true),
            AccountMeta::new(accounts.event_rent_payer, true),
            AccountMeta::new_readonly(accounts.pdas.sender_authority, false),
            AccountMeta::new(accounts.burn_token_account, false),
            AccountMeta::new_readonly(accounts.pdas.denylist_account, false),
            AccountMeta::new(accounts.pdas.message_transmitter, false),
            AccountMeta::new_readonly(accounts.pdas.token_messenger, false),
            AccountMeta::new_readonly(accounts.pdas.remote_token_messenger, false),
            AccountMeta::new_readonly(accounts.pdas.token_minter, false),
            AccountMeta::new(accounts.pdas.local_token, false),
            AccountMeta::new(accounts.burn_token_mint, false),
            AccountMeta::new(accounts.message_sent_event_data, true),
            AccountMeta::new_readonly(programs.message_transmitter, false),
            AccountMeta::new_readonly(programs.token_messenger_minter, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(event_authority_pda(&programs.token_messenger_minter), false),
            AccountMeta::new_readonly(programs.token_messenger_minter, false),
        ],
        data: encode_deposit_for_burn_data_v2(params),
    })
}

pub(crate) fn build_receive_message_instruction_v2(
    accounts: &ReceiveMessageAccountsV2,
    params: &ReceiveMessageParamsV2,
) -> Result<Instruction> {
    let programs = cctp_v2_programs()?;
    let system_program = parse_pubkey(SYSTEM_PROGRAM_ID)?;
    let mut metas = vec![
        AccountMeta::new(accounts.payer, true),
        AccountMeta::new_readonly(accounts.caller, true),
        AccountMeta::new_readonly(accounts.pdas.authority, false),
        AccountMeta::new_readonly(accounts.pdas.message_transmitter, false),
        AccountMeta::new(accounts.pdas.used_nonce, false),
        AccountMeta::new_readonly(accounts.receiver, false),
        AccountMeta::new_readonly(system_program, false),
        AccountMeta::new_readonly(event_authority_pda(&programs.message_transmitter), false),
        AccountMeta::new_readonly(programs.message_transmitter, false),
    ];
    metas.extend(accounts.remaining_accounts.clone());

    Ok(Instruction {
        program_id: programs.message_transmitter,
        accounts: metas,
        data: encode_receive_message_data_v2(params)?,
    })
}

fn extend_borsh_bytes(buffer: &mut Vec<u8>, value: &[u8]) -> Result<()> {
    let len = u32::try_from(value.len())
        .map_err(|_| CliError::InvalidInput("solana instruction payload too large".into()))?;
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(value);
    Ok(())
}

fn read_array<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N]> {
    data.get(offset..offset + N)
        .ok_or_else(|| CliError::InvalidInput("malformed Solana CCTP message".into()))?
        .try_into()
        .map_err(|_| CliError::InvalidInput("malformed Solana CCTP message".into()))
}

fn read_u32_be(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(read_array(data, offset)?))
}

fn find_pda(seeds: &[&[u8]], program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program_id).0
}

fn parse_pubkey(value: &str) -> Result<Pubkey> {
    Pubkey::from_str(value)
        .map_err(|e| CliError::InvalidInput(format!("invalid pubkey `{value}`: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pubkey(value: &str) -> Pubkey {
        Pubkey::from_str(value).unwrap()
    }

    fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn encodes_expected_anchor_discriminators() {
        assert_eq!(
            anchor_discriminator("deposit_for_burn"),
            [0xd7, 0x3c, 0x3d, 0x2e, 0x72, 0x37, 0x80, 0xb0]
        );
        assert_eq!(
            anchor_discriminator("receive_message"),
            [0x26, 0x90, 0x7f, 0xe1, 0x1f, 0xe1, 0xee, 0x19]
        );
    }

    #[test]
    fn encodes_expected_v2_instruction_data() {
        let system = pubkey("11111111111111111111111111111111");

        let deposit = encode_deposit_for_burn_data_v2(&DepositForBurnParamsV2 {
            amount: 123_456_789,
            destination_domain: 6,
            mint_recipient: system,
            destination_caller: system,
            max_fee: 42,
            min_finality_threshold: 2_000,
        });

        assert_eq!(deposit.len(), 96);
        assert_eq!(
            bytes_to_hex(&deposit),
            "d73c3d2e723780b015cd5b070000000006000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002a00000000000000d0070000"
        );

        let receive = encode_receive_message_data_v2(&ReceiveMessageParamsV2 {
            message: vec![0x01, 0x02, 0x03, 0x04],
            attestation: vec![0xaa, 0xbb, 0xcc],
        })
        .unwrap();

        assert_eq!(
            bytes_to_hex(&receive),
            "26907fe11fe1ee19040000000102030403000000aabbcc"
        );
    }

    #[test]
    fn builds_expected_v2_instructions() {
        let owner = pubkey("11111111111111111111111111111111");
        let burn_token_mint = pubkey("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
        let burn_token_account = pubkey("AawthJCGRmggpfv9MMWV6Jmo9cue4gL9wUZgRBShg58W");
        let message_sent_event_data = pubkey("45hzrGLQ2EGo1Ln7QpXjDwb589GDQ9H2aEXXw6ds6BFE");
        let deposit_pdas = cctp_v2_deposit_for_burn_pdas(&owner, &burn_token_mint, 0).unwrap();

        let deposit_ix = build_deposit_for_burn_instruction_v2(
            &DepositForBurnAccountsV2 {
                owner,
                event_rent_payer: owner,
                burn_token_account,
                burn_token_mint,
                message_sent_event_data,
                pdas: deposit_pdas.clone(),
            },
            &DepositForBurnParamsV2 {
                amount: 123_456_789,
                destination_domain: 6,
                mint_recipient: owner,
                destination_caller: owner,
                max_fee: 42,
                min_finality_threshold: 2_000,
            },
        )
        .unwrap();

        assert_eq!(
            deposit_ix.program_id,
            cctp_v2_programs().unwrap().token_messenger_minter
        );
        assert_eq!(deposit_ix.accounts.len(), 18);
        assert_eq!(
            deposit_ix.accounts[0],
            AccountMeta::new_readonly(owner, true)
        );
        assert_eq!(deposit_ix.accounts[1], AccountMeta::new(owner, true));
        assert_eq!(
            deposit_ix.accounts[2],
            AccountMeta::new_readonly(deposit_pdas.sender_authority, false)
        );
        assert_eq!(
            deposit_ix.accounts[16],
            AccountMeta::new_readonly(
                event_authority_pda(&cctp_v2_programs().unwrap().token_messenger_minter),
                false
            )
        );
        assert_eq!(
            deposit_ix.accounts[17],
            AccountMeta::new_readonly(cctp_v2_programs().unwrap().token_messenger_minter, false)
        );

        let mut nonce = [0u8; 32];
        nonce[31] = 1;
        let receive_pdas =
            cctp_v2_receive_message_pdas(&burn_token_mint, &owner, 0, &nonce).unwrap();
        let receive_ix = build_receive_message_instruction_v2(
            &ReceiveMessageAccountsV2 {
                payer: owner,
                caller: owner,
                receiver: cctp_v2_programs().unwrap().token_messenger_minter,
                pdas: receive_pdas.clone(),
                remaining_accounts: vec![AccountMeta::new_readonly(owner, false)],
            },
            &ReceiveMessageParamsV2 {
                message: vec![1, 2, 3, 4],
                attestation: vec![0xaa, 0xbb, 0xcc],
            },
        )
        .unwrap();

        assert_eq!(
            receive_ix.program_id,
            cctp_v2_programs().unwrap().message_transmitter
        );
        assert_eq!(receive_ix.accounts.len(), 10);
        assert_eq!(receive_ix.accounts[0], AccountMeta::new(owner, true));
        assert_eq!(
            receive_ix.accounts[1],
            AccountMeta::new_readonly(owner, true)
        );
        assert_eq!(
            receive_ix.accounts[7],
            AccountMeta::new_readonly(
                event_authority_pda(&cctp_v2_programs().unwrap().message_transmitter),
                false
            )
        );
        assert_eq!(
            receive_ix.accounts[8],
            AccountMeta::new_readonly(cctp_v2_programs().unwrap().message_transmitter, false)
        );
        assert_eq!(
            receive_ix.accounts[9],
            AccountMeta::new_readonly(owner, false)
        );
    }

    #[test]
    fn derives_expected_v2_deposit_for_burn_pdas() {
        let owner = pubkey("11111111111111111111111111111111");
        let burn_token_mint = pubkey("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");

        let pdas = cctp_v2_deposit_for_burn_pdas(&owner, &burn_token_mint, 0).unwrap();

        assert_eq!(
            pdas.message_transmitter,
            pubkey("W1k5ijkaSTo5iA5zChNpfzcy796fLhkBxfmJuR8W8HU")
        );
        assert_eq!(
            pdas.token_messenger,
            pubkey("AawthJCGRmggpfv9MMWV6Jmo9cue4gL9wUZgRBShg58W")
        );
        assert_eq!(
            pdas.token_minter,
            pubkey("E1bQJ8eMMn3zmeSewW3HQ8zmJr7KR75JonbwAtWx2bux")
        );
        assert_eq!(
            pdas.local_token,
            pubkey("7MwmWTK2R9Na6rnoSAEt5gytFmSZj9WLVdazvxvru9AU")
        );
        assert_eq!(
            pdas.remote_token_messenger,
            pubkey("3EzN2mcmdfSNGXRCAixSpTteK6ywdmFDZZWvkMnznFt9")
        );
        assert_eq!(
            pdas.sender_authority,
            pubkey("45hzrGLQ2EGo1Ln7QpXjDwb589GDQ9H2aEXXw6ds6BFE")
        );
        assert_eq!(
            pdas.denylist_account,
            pubkey("CJPnLYncUgWDCwAeNvM5oGzP96NBjmHerSc9Zzhaa571")
        );
    }

    #[test]
    fn derives_expected_v2_receive_message_pdas() {
        let local_token_mint = pubkey("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
        let remote_token = pubkey("1111111111113EsMD5n1VA94D2fALdb1SAKLam8j");
        let mut nonce = [0u8; 32];
        nonce[31] = 1;

        let pdas =
            cctp_v2_receive_message_pdas(&local_token_mint, &remote_token, 0, &nonce).unwrap();

        assert_eq!(
            pdas.authority,
            pubkey("DsAdX23SVpTPYhKP2ua1mx8gTPqLyzx7a43cyxYjS2up")
        );
        assert_eq!(
            pdas.used_nonce,
            pubkey("2jw21GLFV5bC2iXdDsnqxz8cWzSKKr6ewNYgrqABE5VP")
        );
        assert_eq!(
            pdas.message_transmitter,
            pubkey("W1k5ijkaSTo5iA5zChNpfzcy796fLhkBxfmJuR8W8HU")
        );
        assert_eq!(
            pdas.token_messenger,
            pubkey("AawthJCGRmggpfv9MMWV6Jmo9cue4gL9wUZgRBShg58W")
        );
        assert_eq!(
            pdas.token_minter,
            pubkey("E1bQJ8eMMn3zmeSewW3HQ8zmJr7KR75JonbwAtWx2bux")
        );
        assert_eq!(
            pdas.local_token,
            pubkey("7MwmWTK2R9Na6rnoSAEt5gytFmSZj9WLVdazvxvru9AU")
        );
        assert_eq!(
            pdas.remote_token_messenger,
            pubkey("3EzN2mcmdfSNGXRCAixSpTteK6ywdmFDZZWvkMnznFt9")
        );
        assert_eq!(
            pdas.token_pair,
            pubkey("E4ZYkwNfR73MqQB6Kfmf515aQVBk2Wxjp57LbkaGWQ5q")
        );
        assert_eq!(
            pdas.custody_token_account,
            pubkey("CFUgYpbas5UdJkwwSobYgzhFqFuj6C8MfXwBetE3o4SY")
        );
        assert_eq!(
            pdas.token_messenger_event_authority,
            pubkey("6TCCnJ9R1m1RXFzyoH7GYH2J6NJDtZaUvfipPuLWxHNd")
        );
    }
}
