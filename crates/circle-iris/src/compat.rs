//! Compatibility layer for real Iris runtime payloads that do not fully match
//! the published OpenAPI schema.
//!
//! Why this exists:
//! - the generated client is based on Circle's published spec
//! - real `GET /v2/messages/{sourceDomainId}` responses may contain Solana/base58
//!   values in decoded fields such as `sender`, `burnToken`, or `messageSender`
//! - the published schema models those fields as EVM `0x...` addresses
//! - strict generated deserialization therefore fails on valid real responses
//!
//! This module provides a more tolerant wire model just for that endpoint.

use reqwest::Client;
use serde::Deserialize;

use crate::Environment;

#[derive(Clone)]
pub struct CompatClient {
    client: Client,
    base_url: &'static str,
}

impl CompatClient {
    pub fn new(env: Environment) -> Self {
        Self {
            client: Client::new(),
            base_url: env.base_url(),
        }
    }

    pub async fn get_messages_v2(
        &self,
        source_domain_id: u32,
        nonce: Option<&str>,
        transaction_hash: Option<&str>,
    ) -> reqwest::Result<MessagesV2Response> {
        let url = format!("{}/v2/messages/{}", self.base_url, source_domain_id);

        self.client
            .get(url)
            .header("api-version", "1.0")
            .query(&[("nonce", nonce), ("transactionHash", transaction_hash)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagesV2Response {
    pub messages: Vec<MessageV2>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageV2 {
    pub message: Option<String>,
    pub event_nonce: Option<String>,
    pub attestation: Option<String>,
    pub decoded_message: Option<DecodedMessageV2>,
    pub cctp_version: Option<u32>,
    pub status: Option<String>,
    pub delay_reason: Option<String>,
    pub forward_state: Option<String>,
    pub forward_tx_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedMessageV2 {
    pub source_domain: Option<String>,
    pub destination_domain: Option<String>,
    pub nonce: Option<String>,
    pub sender: Option<String>,
    pub recipient: Option<String>,
    pub destination_caller: Option<String>,
    pub min_finality_threshold: Option<String>,
    pub finality_threshold_executed: Option<String>,
    pub message_body: Option<String>,
    pub decoded_message_body: Option<DecodedMessageBodyV2>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedMessageBodyV2 {
    pub burn_token: Option<String>,
    pub mint_recipient: Option<String>,
    pub amount: Option<String>,
    pub message_sender: Option<String>,
    pub max_fee: Option<String>,
    pub fee_executed: Option<String>,
    pub expiration_block: Option<String>,
    pub hook_data: Option<String>,
}
