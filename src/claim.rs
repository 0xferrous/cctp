use alloy_primitives::{B256, hex};
use alloy_provider::ProviderBuilder;
use cctp_rs::MessageTransmitterV2Contract;

use crate::{
    chain::{ChainInfo, ChainKind},
    error::{CliError, Result},
};

pub async fn check_claimed_status(chain: ChainInfo, nonce: &str) -> Result<Option<bool>> {
    if chain.kind != ChainKind::Evm {
        return Ok(None);
    }

    let Some(rpc_url) = chain.rpc_url else {
        return Ok(None);
    };
    let Some(message_transmitter) = chain.message_transmitter else {
        return Ok(None);
    };

    let nonce_key = parse_nonce_key(nonce)?;
    let provider = ProviderBuilder::new().connect_http(
        rpc_url
            .parse()
            .map_err(|e| CliError::InvalidInput(format!("invalid rpc url: {e}")))?,
    );
    let contract = MessageTransmitterV2Contract::new(message_transmitter, provider);
    let used = contract
        .is_message_received(nonce_key.into())
        .await
        .map_err(|e| CliError::Rpc(format!("failed to check destination claim status: {e}")))?;

    Ok(Some(used))
}

fn parse_nonce_key(nonce: &str) -> Result<B256> {
    let nonce_hex = nonce.trim_start_matches("0x");
    let nonce_bytes = hex::decode(nonce_hex)
        .map_err(|e| CliError::InvalidInput(format!("invalid nonce hex: {e}")))?;
    if nonce_bytes.len() != 32 {
        return Err(CliError::InvalidInput(format!(
            "invalid nonce length: expected 32 bytes, got {}",
            nonce_bytes.len()
        )));
    }

    let mut nonce_b256 = [0u8; 32];
    nonce_b256.copy_from_slice(&nonce_bytes);
    Ok(B256::from(nonce_b256))
}
