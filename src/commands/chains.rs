use serde::Serialize;

use crate::{
    chain::supported_chains, cli::ChainsArgs, env_from_testnet, error::Result, output::print_json,
};

#[derive(Debug, Serialize)]
struct ChainRow<'a> {
    id: Option<u64>,
    name: &'a str,
    kind: &'a str,
    env: &'a str,
    cctp_domain: u32,
    token_messenger: Option<String>,
    message_transmitter: Option<String>,
    average_standard_time_seconds: u64,
    average_fast_time_seconds: u64,
}

pub fn run(args: ChainsArgs) -> Result<()> {
    let env = env_from_testnet(args.testnet);
    let rows: Vec<_> = supported_chains(env)
        .iter()
        .map(|chain| ChainRow {
            id: chain.id,
            name: chain.name,
            kind: chain.kind.as_str(),
            env: env.as_str(),
            cctp_domain: chain.cctp_domain,
            token_messenger: chain.token_messenger.map(|a| a.to_string()),
            message_transmitter: chain.message_transmitter.map(|a| a.to_string()),
            average_standard_time_seconds: chain.standard_time_seconds,
            average_fast_time_seconds: chain.fast_time_seconds,
        })
        .collect();

    if args.json {
        print_json(&rows)?;
    } else {
        for row in rows {
            if let Some(id) = row.id {
                println!("{} ({})", row.name, id);
            } else {
                println!("{}", row.name);
            }
            println!("  kind: {}", row.kind);
            println!("  env: {}", row.env);
            println!("  domain: {}", row.cctp_domain);
            println!(
                "  token messenger: {}",
                row.token_messenger.as_deref().unwrap_or("n/a")
            );
            println!(
                "  message transmitter: {}",
                row.message_transmitter.as_deref().unwrap_or("n/a")
            );
            println!("  est. fast time: {}s", row.average_fast_time_seconds);
            println!(
                "  est. standard time: {}s",
                row.average_standard_time_seconds
            );
            println!();
        }
    }

    Ok(())
}
