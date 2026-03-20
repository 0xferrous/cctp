use thiserror::Error;

pub type Result<T> = std::result::Result<T, CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("unknown or unsupported chain: {0}")]
    UnknownChain(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("wallet error: {0}")]
    Wallet(String),
    #[error("Iris request failed: {0}")]
    Iris(String),
    #[error("RPC request failed: {0}")]
    Rpc(String),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
