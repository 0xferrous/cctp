use std::fmt;

use clap::{Args, Parser, Subcommand, ValueEnum};
use foundry_wallets::WalletOpts;

#[derive(Parser, Debug)]
#[command(name = "cctp")]
#[command(about = "CLI for Circle CCTP transfers", long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// List supported chains
    Chains(ChainsArgs),
    /// Estimate CCTP fees for a transfer
    Estimate(EstimateArgs),
    /// Query Circle Iris attestation state
    Attestation(AttestationArgs),
    /// Summarize transfer status from a burn tx hash
    Status(StatusArgs),
    /// Burn USDC on the source chain for a CCTP transfer
    Burn(BurnArgs),
    /// Claim a completed CCTP transfer on the destination chain
    Claim(ClaimArgs),
    /// Run the full burn -> attestation -> claim flow
    Bridge(BridgeArgs),
    /// Request re-attestation for a CCTP nonce through Iris
    Reattest(ReattestArgs),
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum TransferSpeedArg {
    Fast,
    Standard,
}

impl TransferSpeedArg {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Standard => "standard",
        }
    }
}

impl fmt::Display for TransferSpeedArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Args, Debug)]
pub(crate) struct RpcArgs {
    /// RPC URL override for the selected chain
    #[arg(long, env = "ETH_RPC_URL", value_name = "URL")]
    pub rpc_url: Option<String>,
}

#[derive(Args, Debug)]
pub(crate) struct ChainsArgs {
    #[arg(long)]
    pub testnet: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct EstimateArgs {
    #[arg(long = "from")]
    pub from_chain: String,
    #[arg(long = "to")]
    pub to_chain: String,
    #[arg(long)]
    pub amount: String,
    #[arg(long, value_enum, default_value_t = TransferSpeedArg::Fast)]
    pub speed: TransferSpeedArg,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct AttestationArgs {
    #[arg(long = "from")]
    pub from_chain: String,
    #[arg(long, conflicts_with = "nonce")]
    pub tx: Option<String>,
    #[arg(long, conflicts_with = "tx")]
    pub nonce: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct StatusArgs {
    #[arg(long = "from")]
    pub from_chain: String,
    #[arg(long)]
    pub tx: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct BurnArgs {
    #[arg(long = "source-chain")]
    pub from_chain: String,
    #[arg(long = "destination-chain")]
    pub to_chain: String,
    #[arg(long)]
    pub amount: String,
    #[arg(long)]
    pub recipient: String,
    #[arg(long, value_enum, default_value_t = TransferSpeedArg::Fast)]
    pub speed: TransferSpeedArg,
    #[command(flatten)]
    pub wallet: WalletOpts,
    #[command(flatten)]
    pub solana_wallet: SolanaWalletArgs,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ClaimArgs {
    #[arg(long = "source-chain")]
    pub from_chain: String,
    #[arg(long)]
    pub tx: String,
    #[arg(long = "destination-chain")]
    pub to_chain: Option<String>,
    #[command(flatten)]
    pub wallet: WalletOpts,
    #[command(flatten)]
    pub solana_signer: SolanaSignerArgs,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct BridgeArgs {
    #[arg(long = "source-chain")]
    pub from_chain: String,
    #[arg(long = "destination-chain")]
    pub to_chain: String,
    #[arg(long)]
    pub amount: String,
    #[arg(long)]
    pub recipient: String,
    #[arg(long, value_enum, default_value_t = TransferSpeedArg::Fast)]
    pub speed: TransferSpeedArg,
    #[arg(long)]
    pub no_wait: bool,
    #[command(flatten)]
    pub wallet: WalletOpts,
    #[command(flatten)]
    pub solana_wallet: SolanaWalletArgs,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub(crate) struct ReattestArgs {
    #[arg(long = "from")]
    pub from_chain: String,
    #[arg(long)]
    pub nonce: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Default)]
pub(crate) struct SolanaSignerArgs {
    #[arg(long = "solana-mnemonic")]
    pub solana_mnemonic: Option<String>,
    #[arg(long = "solana-passphrase", default_value_t = String::new())]
    pub solana_passphrase: String,
    #[arg(long = "solana-account-index", default_value_t = 0)]
    pub solana_account_index: u32,
}

#[derive(Args, Debug, Default)]
pub(crate) struct SolanaWalletArgs {
    #[command(flatten)]
    pub signer: SolanaSignerArgs,
    #[arg(long = "solana-token-account")]
    pub solana_token_account: Option<String>,
}
