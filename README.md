# cctp

CLI for Circle CCTP transfers, focused on EVM flows.

## Status

Implemented commands:
- `cctp chains`
- `cctp estimate`
- `cctp attestation`
- `cctp status`
- `cctp burn`
- `cctp claim`
- `cctp bridge`
- `cctp reattest`

Current execution scope:
- EVM -> EVM only
- CCTP v2-style execution path
- signer loading via `foundry-wallets::WalletOpts`

Not supported yet:
- Solana execution
- non-EVM destination execution
- broad chain coverage beyond the configured USDC metadata set

## Build

### With Cargo

```bash
cargo build
```

### With Nix dev shell

```bash
nix develop
cargo build
```

## Common commands

List supported chains:

```bash
cargo run -- chains
cargo run -- chains --testnet
```

Estimate fees:

```bash
cargo run -- estimate --from base-sepolia --to arbitrum-sepolia --amount 10
```

Query attestation:

```bash
cargo run -- attestation --from base-sepolia --tx 0x...
```

Check status:

```bash
cargo run -- status --from base-sepolia --tx 0x...
```

Burn:

```bash
cargo run -- burn \
  --source-chain base-sepolia \
  --destination-chain arbitrum-sepolia \
  --amount 1 \
  --recipient 0x... \
  --private-key 0x... \
  --rpc-url https://...
```

Claim:

```bash
cargo run -- claim \
  --source-chain base-sepolia \
  --destination-chain arbitrum-sepolia \
  --tx 0x... \
  --private-key 0x... \
  --rpc-url https://...
```

Bridge end-to-end:

```bash
cargo run -- bridge \
  --source-chain base-sepolia \
  --destination-chain arbitrum-sepolia \
  --amount 1 \
  --recipient 0x... \
  --private-key 0x... \
  --rpc-url https://...
```

Request re-attestation:

```bash
cargo run -- reattest --from base-sepolia --nonce 0x...
```

## Wallets

Execution commands use Foundry wallet options through `foundry-wallets`.
That means you can use familiar flags like:
- `--private-key`
- `--mnemonic`
- `--keystore`
- `--account`
- `--ledger`
- `--trezor`
- `--from`

Run command help to see the full set:

```bash
cargo run -- burn --help
```

## Output

Most read-only and execution commands support:
- human-readable output by default
- `--json` for machine-readable output

## CI

GitHub Actions runs:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace --locked`
- `cargo test --workspace --locked`

## Notes

- RPC URLs default from built-in chain metadata when available, or can be overridden with `--rpc-url`.
- Some chains listed by `chains` are read-only only until their execution metadata is wired in.
- Execution uses canonical Iris message+attestation data for claim flows.
