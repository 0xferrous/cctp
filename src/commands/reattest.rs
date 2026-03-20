use serde::Serialize;

use crate::{
    chain::find_chain,
    cli::ReattestArgs,
    error::{CliError, Result},
    output::print_json,
};

#[derive(Debug, Serialize)]
struct ReattestOutput {
    source_chain: String,
    source_domain: u32,
    nonce: String,
    message: String,
}

pub async fn run(args: ReattestArgs) -> Result<()> {
    let source = find_chain(&args.from_chain)?;
    let iris = crate::commands::attestation::iris_client(source.env);
    let response = iris
        .reattest_message(&args.nonce)
        .await
        .map_err(|e| CliError::Iris(e.to_string()))?;

    let out = ReattestOutput {
        source_chain: source.name.to_owned(),
        source_domain: source.cctp_domain,
        nonce: response.nonce.unwrap_or(args.nonce),
        message: response.message,
    };

    if args.json {
        print_json(&out)?;
    } else {
        println!("Source chain: {}", out.source_chain);
        println!("Source domain: {}", out.source_domain);
        println!("Nonce: {}", out.nonce);
        println!("Iris response: {}", out.message);
    }

    Ok(())
}
