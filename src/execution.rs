use std::{str::FromStr, time::Duration};

use alloy_chains::NamedChain;
use alloy_network::EthereumWallet;
use alloy_primitives::{Address, TxHash, U256, address, hex};
use alloy_provider::{Provider, ProviderBuilder};
use cctp_rs::{CctpV2Bridge, PollingConfig};
use foundry_wallets::WalletOpts;

use crate::{
    amount,
    chain::ChainInfo,
    cli::TransferSpeedArg,
    error::{CliError, Result},
};

const RECEIPT_POLL_ATTEMPTS: u32 = 60;
const RECEIPT_POLL_INTERVAL_SECS: u64 = 2;

pub(crate) fn resolve_rpc_url(chain: ChainInfo, override_url: Option<&str>) -> Result<String> {
    override_url
        .map(str::to_owned)
        .or_else(|| chain.rpc_url.map(str::to_owned))
        .ok_or_else(|| {
            CliError::InvalidInput(format!(
                "missing RPC URL for {} (pass --rpc-url)",
                chain.name
            ))
        })
}

pub(crate) async fn load_wallet(
    wallet: &WalletOpts,
    chain: ChainInfo,
) -> Result<(foundry_wallets::WalletSigner, Address)> {
    let mut signer = wallet
        .signer()
        .await
        .map_err(|e| CliError::Wallet(e.to_string()))?;

    if let Some(chain_id) = chain.id {
        alloy_signer::Signer::set_chain_id(&mut signer, Some(chain_id));
    }

    let signer_address = alloy_signer::Signer::address(&signer);
    if let Some(from) = wallet.from
        && from != signer_address
    {
        return Err(CliError::Wallet(format!(
            "wallet signer address {} does not match --from {}",
            signer_address, from
        )));
    }

    Ok((signer, signer_address))
}

pub(crate) fn provider_with_wallet(
    rpc_url: &str,
    signer: foundry_wallets::WalletSigner,
) -> Result<impl Provider<alloy_network::Ethereum> + Clone> {
    let wallet = EthereumWallet::new(signer);
    let url = rpc_url
        .parse()
        .map_err(|e| CliError::InvalidInput(format!("invalid rpc url: {e}")))?;
    Ok(ProviderBuilder::new().wallet(wallet).connect_http(url))
}

pub(crate) fn named_chain(chain: ChainInfo) -> Result<NamedChain> {
    let chain_id = chain
        .id
        .ok_or_else(|| CliError::InvalidInput(format!("{} is not an EVM chain", chain.name)))?;

    NamedChain::try_from(chain_id)
        .map_err(|_| CliError::InvalidInput(format!("unsupported EVM chain id: {chain_id}")))
}

pub(crate) fn usdc_address(chain: ChainInfo) -> Result<Address> {
    let chain_id = chain
        .id
        .ok_or_else(|| CliError::InvalidInput(format!("{} is not an EVM chain", chain.name)))?;

    match chain_id {
        1 => Ok(address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")),
        42161 => Ok(address!("af88d065e77c8cC2239327C5EDb3A432268e5831")),
        8453 => Ok(address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")),
        10 => Ok(address!("0b2C639c533813f4Aa9D7837CaF62653d097Ff85")),
        43114 => Ok(address!("B97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E")),
        137 => Ok(address!("3c499c542cEF5E3811e1192ce70d8cC03d5c3359")),
        11155111 => Ok(address!("1c7D4B196Cb0C7B01d743Fbc6116a902379C7238")),
        421614 => Ok(address!("75faf114eafb1BDbe2F0316DF893fd58CE46AA4d")),
        84532 => Ok(address!("036CbD53842c5426634e7929541eC2318f3dCF7e")),
        11155420 => Ok(address!("5fd84259d66Cd46123540766Be93DFE6D43130D7")),
        43113 => Ok(address!("5425890298aed601595a70AB815c96711a31Bc65")),
        80002 => Ok(address!("41E94Eb019C0762f9BfFDebBCBAf3dB7F4A2C0cD")),
        _ => Err(CliError::InvalidInput(format!(
            "USDC address metadata is not configured for {}",
            chain.name
        ))),
    }
}

pub(crate) fn parse_recipient(recipient: &str) -> Result<Address> {
    Address::from_str(recipient)
        .map_err(|e| CliError::InvalidInput(format!("invalid recipient address: {e}")))
}

pub(crate) fn decode_hex_bytes(label: &str, value: &str) -> Result<Vec<u8>> {
    hex::decode(value.trim_start_matches("0x"))
        .map_err(|e| CliError::InvalidInput(format!("invalid {label} hex: {e}")))
}

pub(crate) async fn max_fee_for_speed(
    speed: TransferSpeedArg,
    amount_atomic: u128,
    source_domain: u32,
    destination_domain: u32,
    iris: &circle_iris::IrisClient,
) -> Result<(bool, U256)> {
    match speed {
        TransferSpeedArg::Standard => Ok((false, U256::ZERO)),
        TransferSpeedArg::Fast => {
            let fee_bps = iris
                .fast_fee_bps(source_domain, destination_domain)
                .await
                .map_err(|e| CliError::Iris(e.to_string()))?
                .ok_or_else(|| {
                    CliError::InvalidInput(
                        "fast transfer fee tier unavailable for this route".into(),
                    )
                })?;
            let fee = amount::calculate_fee(amount_atomic, fee_bps)?;
            let buffered_fee = fee.saturating_mul(110).div_ceil(100);
            Ok((true, U256::from(buffered_fee)))
        }
    }
}

pub(crate) fn polling_config(speed: TransferSpeedArg) -> PollingConfig {
    match speed {
        TransferSpeedArg::Fast => PollingConfig::fast_transfer(),
        TransferSpeedArg::Standard => PollingConfig::default(),
    }
}

pub(crate) async fn wait_for_receipt<P>(provider: &P, tx_hash: TxHash, label: &str) -> Result<()>
where
    P: Provider<alloy_network::Ethereum>,
{
    for _ in 0..RECEIPT_POLL_ATTEMPTS {
        if provider.get_transaction_receipt(tx_hash).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(RECEIPT_POLL_INTERVAL_SECS)).await;
    }

    Err(CliError::Rpc(format!(
        "timed out waiting for {label} transaction receipt: {tx_hash}"
    )))
}

pub(crate) fn build_bridge<P>(
    source_provider: P,
    destination_provider: P,
    source_chain: NamedChain,
    destination_chain: NamedChain,
    recipient: Address,
    fast_transfer: bool,
    max_fee: U256,
) -> CctpV2Bridge<P>
where
    P: Provider<alloy_network::Ethereum> + Clone,
{
    if fast_transfer {
        CctpV2Bridge::builder()
            .source_chain(source_chain)
            .destination_chain(destination_chain)
            .source_provider(source_provider)
            .destination_provider(destination_provider)
            .recipient(recipient)
            .fast_transfer(true)
            .max_fee(max_fee)
            .build()
    } else {
        CctpV2Bridge::builder()
            .source_chain(source_chain)
            .destination_chain(destination_chain)
            .source_provider(source_provider)
            .destination_provider(destination_provider)
            .recipient(recipient)
            .fast_transfer(false)
            .build()
    }
}
