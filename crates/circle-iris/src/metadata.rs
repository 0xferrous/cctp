//! Circle-published supported chain/domain metadata.
//!
//! Sources:
//! - <https://developers.circle.com/cctp/concepts/supported-chains-and-domains>
//! - <https://developers.circle.com/cctp/references/contract-addresses>
//! - <https://developers.circle.com/cctp/references/solana-programs>
//!
//! Notes:
//! - These values are intended to reflect Circle's published CCTP metadata.
//! - Testnet networks share the same CCTP domain IDs as their corresponding
//!   mainnet families where Circle documents them that way.

use crate::Environment;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Ecosystem {
    Evm,
    Solana,
    Other,
}

impl Ecosystem {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Evm => "evm",
            Self::Solana => "solana",
            Self::Other => "other",
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ChainDomain {
    pub key: &'static str,
    pub name: &'static str,
    pub environment: Environment,
    pub ecosystem: Ecosystem,
    pub domain: u32,
}

const MAINNET_DOMAINS: &[ChainDomain] = &[
    ChainDomain {
        key: "ethereum",
        name: "Ethereum",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 0,
    },
    ChainDomain {
        key: "avalanche",
        name: "Avalanche",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 1,
    },
    ChainDomain {
        key: "optimism",
        name: "Optimism",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 2,
    },
    ChainDomain {
        key: "arbitrum",
        name: "Arbitrum",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 3,
    },
    ChainDomain {
        key: "noble",
        name: "Noble",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Other,
        domain: 4,
    },
    ChainDomain {
        key: "solana",
        name: "Solana",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Solana,
        domain: 5,
    },
    ChainDomain {
        key: "base",
        name: "Base",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 6,
    },
    ChainDomain {
        key: "polygon",
        name: "Polygon PoS",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 7,
    },
    ChainDomain {
        key: "sui",
        name: "Sui",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Other,
        domain: 8,
    },
    ChainDomain {
        key: "aptos",
        name: "Aptos",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Other,
        domain: 9,
    },
    ChainDomain {
        key: "unichain",
        name: "Unichain",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 10,
    },
    ChainDomain {
        key: "linea",
        name: "Linea",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 11,
    },
    ChainDomain {
        key: "codex",
        name: "Codex",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Other,
        domain: 12,
    },
    ChainDomain {
        key: "sonic",
        name: "Sonic",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 13,
    },
    ChainDomain {
        key: "world-chain",
        name: "World Chain",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 14,
    },
    ChainDomain {
        key: "hyperliquid",
        name: "Hyperliquid",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 15,
    },
    ChainDomain {
        key: "sei",
        name: "Sei",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 16,
    },
    ChainDomain {
        key: "plume",
        name: "Plume",
        environment: Environment::Mainnet,
        ecosystem: Ecosystem::Evm,
        domain: 17,
    },
];

const TESTNET_DOMAINS: &[ChainDomain] = &[
    ChainDomain {
        key: "ethereum-sepolia",
        name: "Ethereum Sepolia",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 0,
    },
    ChainDomain {
        key: "avalanche-fuji",
        name: "Avalanche Fuji",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 1,
    },
    ChainDomain {
        key: "optimism-sepolia",
        name: "Optimism Sepolia",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 2,
    },
    ChainDomain {
        key: "arbitrum-sepolia",
        name: "Arbitrum Sepolia",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 3,
    },
    ChainDomain {
        key: "noble-testnet",
        name: "Noble Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Other,
        domain: 4,
    },
    ChainDomain {
        key: "solana-devnet",
        name: "Solana Devnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Solana,
        domain: 5,
    },
    ChainDomain {
        key: "base-sepolia",
        name: "Base Sepolia",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 6,
    },
    ChainDomain {
        key: "polygon-amoy",
        name: "Polygon Amoy",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 7,
    },
    ChainDomain {
        key: "sui-testnet",
        name: "Sui Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Other,
        domain: 8,
    },
    ChainDomain {
        key: "aptos-testnet",
        name: "Aptos Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Other,
        domain: 9,
    },
    ChainDomain {
        key: "unichain-sepolia",
        name: "Unichain Sepolia",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 10,
    },
    ChainDomain {
        key: "linea-sepolia",
        name: "Linea Sepolia",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 11,
    },
    ChainDomain {
        key: "codex-testnet",
        name: "Codex Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Other,
        domain: 12,
    },
    ChainDomain {
        key: "sonic-testnet",
        name: "Sonic Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 13,
    },
    ChainDomain {
        key: "world-chain-sepolia",
        name: "World Chain Sepolia",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 14,
    },
    ChainDomain {
        key: "hyperliquid-testnet",
        name: "Hyperliquid Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 15,
    },
    ChainDomain {
        key: "sei-testnet",
        name: "Sei Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 16,
    },
    ChainDomain {
        key: "plume-testnet",
        name: "Plume Testnet",
        environment: Environment::Testnet,
        ecosystem: Ecosystem::Evm,
        domain: 17,
    },
];

/// Returns all Circle-published supported domains for the given environment.
pub const fn supported_domains(environment: Environment) -> &'static [ChainDomain] {
    match environment {
        Environment::Mainnet => MAINNET_DOMAINS,
        Environment::Testnet => TESTNET_DOMAINS,
    }
}

/// Finds a supported chain/domain entry by CCTP domain within an environment.
pub fn find_domain(environment: Environment, domain: u32) -> Option<ChainDomain> {
    supported_domains(environment)
        .iter()
        .copied()
        .find(|entry| entry.domain == domain)
}

/// Finds a supported chain/domain entry by normalized key within an environment.
pub fn find_chain(environment: Environment, key: &str) -> Option<ChainDomain> {
    let key = key.trim().to_ascii_lowercase();
    supported_domains(environment)
        .iter()
        .copied()
        .find(|entry| entry.key == key)
}
