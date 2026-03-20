#![allow(dead_code)]

use std::{env, fs, path::PathBuf};

use crate::{
    chain::{ChainInfo, ChainKind},
    cli::{RpcArgs, WalletArgs},
    error::{CliError, Result},
};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedWalletConfig {
    pub account_index: u32,
    pub auth: WalletAuth,
}

#[derive(Debug, Clone)]
pub(crate) enum WalletAuth {
    PrivateKey(String),
    Mnemonic(String),
    Keystore {
        path: PathBuf,
        password: Option<String>,
    },
}

pub(crate) fn resolve_wallet_config(args: &WalletArgs) -> Result<ResolvedWalletConfig> {
    let private_key =
        resolve_explicit_or_env(args.private_key.clone(), args.private_key_env.as_deref())?;
    let mnemonic = resolve_explicit_or_env(args.mnemonic.clone(), args.mnemonic_env.as_deref())?;
    let keystore = args.keystore.as_ref().map(PathBuf::from);

    let configured =
        private_key.is_some() as u8 + mnemonic.is_some() as u8 + keystore.is_some() as u8;
    if configured == 0 {
        return Err(CliError::Wallet(
            "missing wallet flags: pass one of --private-key, --private-key-env, --mnemonic, --mnemonic-env, or --keystore".into(),
        ));
    }
    if configured > 1 {
        return Err(CliError::Wallet(
            "wallet flags are mutually exclusive: choose exactly one auth method".into(),
        ));
    }

    let auth = if let Some(private_key) = private_key {
        WalletAuth::PrivateKey(private_key)
    } else if let Some(mnemonic) = mnemonic {
        WalletAuth::Mnemonic(mnemonic)
    } else if let Some(path) = keystore {
        let password =
            resolve_explicit_or_env(args.password.clone(), args.password_env.as_deref())?;
        WalletAuth::Keystore { path, password }
    } else {
        unreachable!("validated exactly one wallet auth method")
    };

    Ok(ResolvedWalletConfig {
        account_index: args.account_index,
        auth,
    })
}

pub(crate) fn resolve_rpc_url(chain: ChainInfo, args: &RpcArgs) -> Result<String> {
    if chain.kind != ChainKind::Evm {
        return Err(CliError::InvalidInput(format!(
            "execution currently supports EVM chains only; got {}",
            chain.name
        )));
    }

    args.rpc_url
        .clone()
        .or_else(|| chain.rpc_url.map(str::to_owned))
        .ok_or_else(|| {
            CliError::InvalidInput(format!(
                "missing RPC URL for {} (pass --rpc-url)",
                chain.name
            ))
        })
}

pub(crate) fn wallet_summary(config: &ResolvedWalletConfig) -> &'static str {
    match config.auth {
        WalletAuth::PrivateKey(_) => "private-key",
        WalletAuth::Mnemonic(_) => "mnemonic",
        WalletAuth::Keystore { .. } => "keystore",
    }
}

pub(crate) fn read_keystore_bytes(path: &PathBuf) -> Result<Vec<u8>> {
    Ok(fs::read(path)?)
}

fn resolve_explicit_or_env(
    explicit: Option<String>,
    env_var: Option<&str>,
) -> Result<Option<String>> {
    match (explicit, env_var) {
        (Some(value), None) => Ok(Some(value)),
        (None, Some(var)) => env::var(var)
            .map(Some)
            .map_err(|_| CliError::Wallet(format!("environment variable {var} is not set"))),
        (Some(_), Some(_)) => Err(CliError::Wallet(
            "pass either a direct value or an --*-env flag, not both".into(),
        )),
        (None, None) => Ok(None),
    }
}
