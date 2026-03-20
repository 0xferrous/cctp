//! Circle Iris API client generated from Circle's published CCTP OpenAPI spec.
//!
//! Pinned spec source:
//! <https://developers.circle.com/openapi/cctp.yaml>
//!
//! The corresponding docs page where we discovered the spec reference:
//! <https://developers.circle.com/api-reference/cctp/all/get-public-keys-v2>
//!
//! # Examples
//!
//! ```rust,no_run
//! use circle_iris::{Environment, IrisClient, MessageLookup};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = IrisClient::new(Environment::Mainnet);
//!
//! let public_keys = client.public_keys_v2().await?;
//! println!("{} public keys", public_keys.len());
//!
//! let attestation = client
//!     .attestation(
//!         0,
//!         MessageLookup::TransactionHash(
//!             "0x912f22a13e9ccb979b621500f6952b2afd6e75be7eadaed93fc2625fe11c52a2",
//!         ),
//!     )
//!     .await?;
//!
//! println!("status: {}", attestation.status.as_str());
//! # Ok(())
//! # }
//! ```

/// Tolerant runtime models/client used for endpoints where real Iris payloads
/// do not fully conform to the published OpenAPI schema.
///
/// In particular, `GET /v2/messages/{sourceDomainId}` can return Solana/base58
/// values in decoded fields that the published spec models as EVM-style
/// `0x...` addresses, which breaks strict codegen deserialization.
pub mod compat;
pub mod metadata;

/// Generated low-level client and API types for the complete published CCTP API.
pub mod generated {
    progenitor::generate_api!(spec = "openapi/cctp.yaml");
}

use compat as compat_types;
use generated::{Client as GeneratedClient, types};
use progenitor::progenitor_client;

const BPS_PRECISION: u128 = 10_000;
const FAST_TRANSFER_FINALITY_THRESHOLD: i64 = 1000;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Environment {
    Mainnet,
    Testnet,
}

impl Environment {
    /// Returns the base URL for the selected Iris environment.
    pub const fn base_url(self) -> &'static str {
        match self {
            Self::Mainnet => "https://iris-api.circle.com",
            Self::Testnet => "https://iris-api-sandbox.circle.com",
        }
    }

    /// Creates the generated low-level client for the selected environment.
    pub fn raw_client(self) -> GeneratedClient {
        GeneratedClient::new(self.base_url())
    }
}

#[derive(Clone)]
pub struct IrisClient {
    raw: GeneratedClient,
    compat: compat::CompatClient,
}

impl IrisClient {
    /// Creates a high-level Iris client for the selected environment.
    pub fn new(env: Environment) -> Self {
        Self {
            raw: env.raw_client(),
            compat: compat::CompatClient::new(env),
        }
    }

    /// Returns the generated low-level client.
    ///
    /// This is useful when you want direct access to the full code-generated API.
    pub fn raw(&self) -> &GeneratedClient {
        &self.raw
    }

    /// Fetches the currently active legacy CCTP v1 attestation public keys.
    pub async fn public_keys_v1(&self) -> Result<Vec<String>, Error> {
        let response = self
            .raw
            .get_public_keys()
            .await
            .map_err(|e| Error::Api(e.to_string()))?;
        Ok(response.into_inner().public_keys)
    }

    /// Fetches the currently active attestation public keys across supported CCTP versions.
    pub async fn public_keys_v2(&self) -> Result<Vec<PublicKey>, Error> {
        let response = self.raw.get_public_keys_v2().await.map_err(map_error)?;
        Ok(response
            .into_inner()
            .public_keys
            .into_iter()
            .map(|key| PublicKey {
                public_key: key.public_key.unwrap_or_default(),
                cctp_version: key.cctp_version.map(|v| i64::from(v) as i32),
            })
            .collect())
    }

    /// Fetches a legacy CCTP v1 attestation by message hash.
    ///
    /// This wraps `GET /v1/attestations/{messageHash}`.
    pub async fn attestation_v1(
        &self,
        message_hash: &str,
    ) -> Result<LegacyAttestationResponse, Error> {
        let message_hash = message_hash
            .try_into()
            .map_err(|_| Error::InvalidValue("invalid message hash".into()))?;
        let response = self
            .raw
            .get_attestation(&message_hash)
            .await
            .map_err(map_error)?;
        let body = response.into_inner();
        Ok(LegacyAttestationResponse {
            status: Some(map_legacy_status(body.status)),
            attestation: normalize_pending_string(body.attestation),
        })
    }

    /// Fetches legacy CCTP v1 messages for a source domain and transaction hash.
    pub async fn messages_v1(
        &self,
        source_domain: u32,
        transaction_hash: &str,
    ) -> Result<Vec<LegacyMessage>, Error> {
        let transaction_hash = transaction_hash
            .try_into()
            .map_err(|_| Error::InvalidValue("invalid transaction hash".into()))?;
        let response = self
            .raw
            .get_messages(source_domain.into(), &transaction_hash)
            .await
            .map_err(map_error)?;
        Ok(response
            .into_inner()
            .messages
            .into_iter()
            .map(|message| LegacyMessage {
                attestation: normalize_pending_string(Some(message.attestation)),
                event_nonce: message.event_nonce.to_string(),
                message: message.message,
            })
            .collect())
    }

    /// Fetches CCTP v2 messages by source domain and either transaction hash or nonce.
    ///
    /// This uses the tolerant runtime parser because real Iris payloads for this
    /// endpoint do not always conform to the published OpenAPI schema.
    pub async fn messages_v2(
        &self,
        source_domain: u32,
        lookup: MessageLookup<'_>,
    ) -> Result<Vec<Message>, Error> {
        let (nonce, transaction_hash) = match lookup {
            MessageLookup::TransactionHash(tx) => (None, Some(tx)),
            MessageLookup::Nonce(nonce) => (Some(nonce), None),
        };

        let response = self
            .compat
            .get_messages_v2(source_domain, nonce, transaction_hash)
            .await
            .map_err(Error::Network)?;

        Ok(response.messages.into_iter().map(Message::from).collect())
    }

    /// Fetches the first v2 message/attestation match and normalizes it into a
    /// higher-level attestation view.
    ///
    /// This is the convenience method most callers will want for read-only
    /// transfer inspection.
    pub async fn attestation(
        &self,
        source_domain: u32,
        lookup: MessageLookup<'_>,
    ) -> Result<AttestationResponse, Error> {
        let burn_tx_hash = match lookup {
            MessageLookup::TransactionHash(tx) => Some(tx.to_owned()),
            MessageLookup::Nonce(_) => None,
        };

        let message = self
            .messages_v2(source_domain, lookup)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Api("Iris returned no messages".into()))?;

        Ok(AttestationResponse {
            status: message.status.unwrap_or(AttestationStatus::Pending),
            source_domain: message
                .decoded_message
                .as_ref()
                .and_then(|m| m.source_domain),
            destination_domain: message
                .decoded_message
                .as_ref()
                .and_then(|m| m.destination_domain),
            nonce: message.event_nonce,
            amount: message
                .decoded_message
                .as_ref()
                .and_then(|m| m.decoded_message_body.as_ref())
                .and_then(|b| b.amount.clone()),
            mint_recipient: message
                .decoded_message
                .as_ref()
                .and_then(|m| m.decoded_message_body.as_ref())
                .and_then(|b| b.mint_recipient.clone()),
            delay_reason: message.delay_reason,
            burn_tx_hash,
            message: message.message,
            attestation: message.attestation,
        })
    }

    /// Requests re-attestation for a pre-finality message nonce.
    pub async fn reattest_message(&self, nonce: &str) -> Result<ReattestationResponse, Error> {
        let response = self.raw.reattest_message(nonce).await.map_err(map_error)?;
        let body = response.into_inner();
        Ok(ReattestationResponse {
            message: body.message.unwrap_or_default(),
            nonce: body.nonce.map(|n| n.to_string()),
        })
    }

    /// Fetches the currently available Fast Burn USDC allowance.
    pub async fn fast_burn_usdc_allowance(&self) -> Result<FastBurnAllowanceResponse, Error> {
        let response = self
            .raw
            .get_fast_burn_usdc_allowance()
            .await
            .map_err(|e| Error::Api(e.to_string()))?;
        let body = response.into_inner();
        Ok(FastBurnAllowanceResponse {
            allowance: body.allowance,
            last_updated: body.last_updated.map(|ts| ts.to_string()),
        })
    }

    /// Fetches the published USDC transfer fee schedule between two domains.
    ///
    /// Set `forward` and `hyper_core_deposit` to include the extra Circle
    /// Forwarder-related fee information when needed.
    pub async fn burn_usdc_fees(
        &self,
        source_domain: u32,
        destination_domain: u32,
        forward: Option<bool>,
        hyper_core_deposit: Option<bool>,
    ) -> Result<Vec<BurnFee>, Error> {
        let response = self
            .raw
            .get_burn_usdc_fees(
                source_domain.into(),
                destination_domain.into(),
                forward,
                hyper_core_deposit,
            )
            .await
            .map_err(map_error)?;

        Ok(response
            .into_inner()
            .0
            .into_iter()
            .map(|tier| BurnFee {
                finality_threshold: tier.finality_threshold,
                minimum_fee_bps: tier.minimum_fee,
                forward_fee: tier.forward_fee.map(|f| ForwardFee {
                    low: f.low.unwrap_or_default(),
                    medium: f.medium.unwrap_or_default(),
                    high: f.high.unwrap_or_default(),
                }),
            })
            .collect())
    }

    /// Returns the minimum fast-transfer fee in scaled basis points for the
    /// given source/destination domain pair.
    ///
    /// The returned value uses `BPS_PRECISION = 10_000`, so `1 bps` becomes
    /// `10_000` and can be combined with integer arithmetic in downstream code.
    pub async fn fast_fee_bps(
        &self,
        source_domain: u32,
        destination_domain: u32,
    ) -> Result<Option<u128>, Error> {
        self.burn_usdc_fees(source_domain, destination_domain, None, None)
            .await?
            .into_iter()
            .find(|tier| tier.finality_threshold == FAST_TRANSFER_FINALITY_THRESHOLD)
            .map(|tier| minimum_fee_scaled_bps(tier.minimum_fee_bps))
            .transpose()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("api error: {0}")]
    Api(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

#[derive(Copy, Clone, Debug)]
pub enum MessageLookup<'a> {
    TransactionHash(&'a str),
    Nonce(&'a str),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AttestationStatus {
    Pending,
    PendingConfirmations,
    Complete,
}

impl AttestationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::PendingConfirmations => "pending_confirmations",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AttestationResponse {
    pub status: AttestationStatus,
    pub source_domain: Option<u32>,
    pub destination_domain: Option<u32>,
    pub nonce: Option<String>,
    pub amount: Option<String>,
    pub mint_recipient: Option<String>,
    pub delay_reason: Option<String>,
    pub burn_tx_hash: Option<String>,
    pub message: Option<String>,
    pub attestation: Option<String>,
}

impl AttestationResponse {
    pub fn into_state(self) -> AttestationState {
        match self.status {
            AttestationStatus::Complete => match CompleteAttestation::try_from(self) {
                Ok(complete) => AttestationState::Complete(complete),
                Err(pending) => AttestationState::Pending(pending),
            },
            _ => AttestationState::Pending(self),
        }
    }
}

#[derive(Debug)]
pub enum AttestationState {
    Pending(AttestationResponse),
    Complete(CompleteAttestation),
}

#[derive(Debug)]
pub struct CompleteAttestation {
    pub destination_domain: u32,
    pub nonce: String,
    pub amount: Option<String>,
    pub mint_recipient: Option<String>,
    pub delay_reason: Option<String>,
}

impl TryFrom<AttestationResponse> for CompleteAttestation {
    type Error = AttestationResponse;

    fn try_from(value: AttestationResponse) -> std::result::Result<Self, Self::Error> {
        let _source_domain = value.source_domain.ok_or_else(|| value.clone())?;
        let destination_domain = value.destination_domain.ok_or_else(|| value.clone())?;
        let nonce = value.nonce.clone().ok_or_else(|| value.clone())?;

        if value.message.is_none() || value.attestation.is_none() {
            return Err(value);
        }

        Ok(Self {
            destination_domain,
            nonce,
            amount: value.amount,
            mint_recipient: value.mint_recipient,
            delay_reason: value.delay_reason,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PublicKey {
    pub public_key: String,
    pub cctp_version: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct LegacyAttestationResponse {
    pub status: Option<AttestationStatus>,
    pub attestation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LegacyMessage {
    pub attestation: Option<String>,
    pub event_nonce: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub message: Option<String>,
    pub event_nonce: Option<String>,
    pub attestation: Option<String>,
    pub decoded_message: Option<DecodedMessage>,
    pub cctp_version: Option<u32>,
    pub status: Option<AttestationStatus>,
    pub delay_reason: Option<String>,
    pub forward_state: Option<String>,
    pub forward_tx_hash: Option<String>,
}

impl From<compat_types::MessageV2> for Message {
    fn from(value: compat_types::MessageV2) -> Self {
        Self {
            message: value.message,
            event_nonce: value.event_nonce,
            attestation: normalize_pending_string(value.attestation),
            decoded_message: value.decoded_message.map(DecodedMessage::from),
            cctp_version: value.cctp_version,
            status: value.status.as_deref().map(map_status_str),
            delay_reason: value.delay_reason,
            forward_state: value.forward_state,
            forward_tx_hash: value.forward_tx_hash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedMessage {
    pub source_domain: Option<u32>,
    pub destination_domain: Option<u32>,
    pub nonce: Option<String>,
    pub sender: Option<String>,
    pub recipient: Option<String>,
    pub destination_caller: Option<String>,
    pub min_finality_threshold: Option<String>,
    pub finality_threshold_executed: Option<String>,
    pub message_body: Option<String>,
    pub decoded_message_body: Option<DecodedMessageBody>,
}

impl From<compat_types::DecodedMessageV2> for DecodedMessage {
    fn from(value: compat_types::DecodedMessageV2) -> Self {
        Self {
            source_domain: value.source_domain.as_deref().and_then(parse_domain_id_str),
            destination_domain: value
                .destination_domain
                .as_deref()
                .and_then(parse_domain_id_str),
            nonce: value.nonce,
            sender: value.sender,
            recipient: value.recipient,
            destination_caller: value.destination_caller,
            min_finality_threshold: value.min_finality_threshold,
            finality_threshold_executed: value.finality_threshold_executed,
            message_body: value.message_body,
            decoded_message_body: value.decoded_message_body.map(DecodedMessageBody::from),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedMessageBody {
    pub burn_token: Option<String>,
    pub mint_recipient: Option<String>,
    pub amount: Option<String>,
    pub message_sender: Option<String>,
    pub max_fee: Option<String>,
    pub fee_executed: Option<String>,
    pub expiration_block: Option<String>,
    pub hook_data: Option<String>,
}

impl From<compat_types::DecodedMessageBodyV2> for DecodedMessageBody {
    fn from(value: compat_types::DecodedMessageBodyV2) -> Self {
        Self {
            burn_token: value.burn_token,
            mint_recipient: value.mint_recipient,
            amount: value.amount,
            message_sender: value.message_sender,
            max_fee: value.max_fee,
            fee_executed: value.fee_executed,
            expiration_block: value.expiration_block,
            hook_data: value.hook_data,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReattestationResponse {
    pub message: String,
    pub nonce: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FastBurnAllowanceResponse {
    pub allowance: Option<f64>,
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BurnFee {
    pub finality_threshold: i64,
    pub minimum_fee_bps: f64,
    pub forward_fee: Option<ForwardFee>,
}

#[derive(Debug, Clone)]
pub struct ForwardFee {
    pub low: i64,
    pub medium: i64,
    pub high: i64,
}

fn map_legacy_status(status: types::AttestationStatus) -> AttestationStatus {
    match status {
        types::AttestationStatus::Complete => AttestationStatus::Complete,
        types::AttestationStatus::PendingConfirmations => AttestationStatus::PendingConfirmations,
    }
}

fn map_status_str(status: &str) -> AttestationStatus {
    match status {
        "complete" => AttestationStatus::Complete,
        "pending_confirmations" => AttestationStatus::PendingConfirmations,
        _ => AttestationStatus::Pending,
    }
}

fn parse_domain_id_str(domain: &str) -> Option<u32> {
    domain.parse().ok()
}

fn normalize_pending_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        if value.eq_ignore_ascii_case("pending") || value.is_empty() {
            None
        } else {
            Some(value)
        }
    })
}

fn minimum_fee_scaled_bps(minimum_fee: f64) -> Result<u128, Error> {
    if !minimum_fee.is_finite() || minimum_fee.is_sign_negative() {
        return Err(Error::InvalidValue(format!(
            "invalid minimumFee value: {minimum_fee}"
        )));
    }

    Ok((minimum_fee * BPS_PRECISION as f64).round() as u128)
}

fn map_error(error: progenitor_client::Error<types::ErrorResponse>) -> Error {
    match error {
        progenitor_client::Error::ErrorResponse(response) => {
            let body = response.into_inner();
            Error::Api(format!("{} ({})", body.message, body.code))
        }
        other => Error::Api(other.to_string()),
    }
}
