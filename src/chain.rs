use std::str::FromStr;

use alloy_chains::NamedChain;
use alloy_primitives::Address;
use cctp_rs::{CctpV1, CctpV2};

use crate::error::{CliError, Result};

// Circle Iris currently uses domain 5 for Solana on both mainnet and devnet.
const SOLANA_MAINNET_DOMAIN: u32 = 5;
const SOLANA_DEVNET_DOMAIN: u32 = 5;

const MAINNET_EVM_CHAINS: &[EvmChainConfig] = &[
    EvmChainConfig::new(
        NamedChain::Mainnet,
        "Ethereum",
        Some("https://ethereum-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::Arbitrum,
        "Arbitrum",
        Some("https://arbitrum-one-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::Base,
        "Base",
        Some("https://base-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::Optimism,
        "Optimism",
        Some("https://optimism-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::Avalanche,
        "Avalanche",
        Some("https://avalanche-c-chain-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::Polygon,
        "Polygon",
        Some("https://polygon-bor-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(NamedChain::Unichain, "Unichain", None),
    EvmChainConfig::new(
        NamedChain::Linea,
        "Linea",
        Some("https://linea-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::Sonic,
        "Sonic",
        Some("https://sonic-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::Sei,
        "Sei",
        Some("https://sei-evm-rpc.publicnode.com"),
    ),
];

const TESTNET_EVM_CHAINS: &[EvmChainConfig] = &[
    EvmChainConfig::new(
        NamedChain::Sepolia,
        "Ethereum Sepolia",
        Some("https://ethereum-sepolia-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::ArbitrumSepolia,
        "Arbitrum Sepolia",
        Some("https://arbitrum-sepolia-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::BaseSepolia,
        "Base Sepolia",
        Some("https://base-sepolia-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::OptimismSepolia,
        "Optimism Sepolia",
        Some("https://optimism-sepolia-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::AvalancheFuji,
        "Avalanche Fuji",
        Some("https://avalanche-fuji-c-chain-rpc.publicnode.com"),
    ),
    EvmChainConfig::new(
        NamedChain::PolygonAmoy,
        "Polygon Amoy",
        Some("https://polygon-amoy-bor-rpc.publicnode.com"),
    ),
];

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BridgeEnvironment {
    Mainnet,
    Testnet,
}

impl BridgeEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Testnet => "testnet",
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ChainKind {
    Evm,
    Solana,
}

impl ChainKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Evm => "evm",
            Self::Solana => "solana",
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ChainInfo {
    pub id: Option<u64>,
    pub name: &'static str,
    pub kind: ChainKind,
    pub env: BridgeEnvironment,
    pub cctp_domain: u32,
    pub token_messenger: Option<Address>,
    pub message_transmitter: Option<Address>,
    pub rpc_url: Option<&'static str>,
    pub standard_time_seconds: u64,
    pub fast_time_seconds: u64,
}

impl ChainInfo {
    pub fn standard_time_label(&self) -> &'static str {
        format_duration(self.standard_time_seconds)
    }

    pub fn fast_time_label(&self) -> &'static str {
        format_duration(self.fast_time_seconds)
    }
}

#[derive(Copy, Clone)]
struct EvmChainConfig {
    chain: NamedChain,
    display_name: &'static str,
    rpc_url: Option<&'static str>,
}

impl EvmChainConfig {
    const fn new(
        chain: NamedChain,
        display_name: &'static str,
        rpc_url: Option<&'static str>,
    ) -> Self {
        Self {
            chain,
            display_name,
            rpc_url,
        }
    }

    fn into_chain_info(self) -> Option<ChainInfo> {
        let env = if self.chain.is_testnet() {
            BridgeEnvironment::Testnet
        } else {
            BridgeEnvironment::Mainnet
        };

        let (
            cctp_domain,
            token_messenger,
            message_transmitter,
            standard_time_seconds,
            fast_time_seconds,
        ) = if self.chain.supports_cctp_v2() {
            (
                self.chain.cctp_v2_domain_id().ok()?.as_u32(),
                Some(self.chain.token_messenger_v2_address().ok()?),
                Some(self.chain.message_transmitter_v2_address().ok()?),
                self.chain
                    .standard_transfer_confirmation_time_seconds()
                    .ok()?,
                self.chain.fast_transfer_confirmation_time_seconds().ok()?,
            )
        } else if self.chain.is_supported() {
            (
                self.chain.cctp_domain_id().ok()?.as_u32(),
                Some(self.chain.token_messenger_address().ok()?),
                Some(self.chain.message_transmitter_address().ok()?),
                self.chain.confirmation_average_time_seconds().ok()?,
                self.chain.confirmation_average_time_seconds().ok()?,
            )
        } else {
            return None;
        };

        Some(ChainInfo {
            id: Some(self.chain as u64),
            name: self.display_name,
            kind: ChainKind::Evm,
            env,
            cctp_domain,
            token_messenger,
            message_transmitter,
            rpc_url: self.rpc_url,
            standard_time_seconds,
            fast_time_seconds,
        })
    }
}

pub fn supported_chains(env: BridgeEnvironment) -> Vec<ChainInfo> {
    let evm = match env {
        BridgeEnvironment::Mainnet => MAINNET_EVM_CHAINS,
        BridgeEnvironment::Testnet => TESTNET_EVM_CHAINS,
    };

    let mut chains: Vec<_> = evm.iter().filter_map(|cfg| cfg.into_chain_info()).collect();
    chains.push(solana_chain(env));
    chains
}

pub fn find_chain(input: &str) -> Result<ChainInfo> {
    let normalized = input.trim().to_ascii_lowercase();

    if let Some(chain) = find_solana_chain(&normalized) {
        return Ok(chain);
    }

    let Some(named_chain) = parse_named_chain(&normalized) else {
        return Err(CliError::UnknownChain(input.into()));
    };

    config_for_chain(named_chain)
        .and_then(EvmChainConfig::into_chain_info)
        .ok_or_else(|| CliError::UnknownChain(input.into()))
}

pub fn infer_chain_by_domain(domain: u32, env: BridgeEnvironment) -> Option<ChainInfo> {
    supported_chains(env)
        .into_iter()
        .find(|chain| chain.cctp_domain == domain)
}

fn parse_named_chain(input: &str) -> Option<NamedChain> {
    match input {
        "ethereum" => Some(NamedChain::Mainnet),
        "ethereum-sepolia" => Some(NamedChain::Sepolia),
        _ => NamedChain::from_str(input).ok(),
    }
}

fn config_for_chain(chain: NamedChain) -> Option<EvmChainConfig> {
    MAINNET_EVM_CHAINS
        .iter()
        .chain(TESTNET_EVM_CHAINS.iter())
        .copied()
        .find(|cfg| cfg.chain == chain)
}

fn find_solana_chain(input: &str) -> Option<ChainInfo> {
    match input {
        "solana" => Some(solana_chain(BridgeEnvironment::Mainnet)),
        "solana-devnet" | "devnet" => Some(solana_chain(BridgeEnvironment::Testnet)),
        _ => None,
    }
}

fn solana_chain(env: BridgeEnvironment) -> ChainInfo {
    match env {
        BridgeEnvironment::Mainnet => ChainInfo {
            id: None,
            name: "Solana",
            kind: ChainKind::Solana,
            env,
            cctp_domain: SOLANA_MAINNET_DOMAIN,
            token_messenger: None,
            message_transmitter: None,
            rpc_url: Some("https://api.mainnet-beta.solana.com"),
            standard_time_seconds: 15 * 60,
            fast_time_seconds: 20,
        },
        BridgeEnvironment::Testnet => ChainInfo {
            id: None,
            name: "Solana Devnet",
            kind: ChainKind::Solana,
            env,
            cctp_domain: SOLANA_DEVNET_DOMAIN,
            token_messenger: None,
            message_transmitter: None,
            rpc_url: Some("https://api.devnet.solana.com"),
            standard_time_seconds: 15 * 60,
            fast_time_seconds: 20,
        },
    }
}

fn format_duration(seconds: u64) -> &'static str {
    match seconds {
        0..=10 => "~10 seconds",
        11..=30 => "~20 seconds",
        31..=59 => "<1 minute",
        60..=90 => "~1 minute",
        91..=300 => "a few minutes",
        301..=1200 => "~15 minutes",
        _ => "hours",
    }
}
