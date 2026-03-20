use circle_iris::{Environment, IrisClient};
use serde::Serialize;

use crate::{
    amount::{self, format_usdc, parse_usdc_amount},
    chain::{BridgeEnvironment, find_chain},
    cli::{EstimateArgs, TransferSpeedArg},
    error::{CliError, Result},
    output::print_json,
};

#[derive(Debug, Serialize)]
struct EstimateOutput<'a> {
    source_chain: &'a str,
    destination_chain: &'a str,
    source_domain: u32,
    destination_domain: u32,
    speed: &'a str,
    amount: String,
    protocol_fee: String,
    received_amount: String,
    estimated_time: &'a str,
    fallback_to_standard: bool,
}

pub async fn run(args: EstimateArgs) -> Result<()> {
    let source = find_chain(&args.from_chain)?;
    let destination = find_chain(&args.to_chain)?;

    if source.env != destination.env {
        return Err(CliError::InvalidInput(
            "source and destination chains must be in the same environment".into(),
        ));
    }

    let amount = parse_usdc_amount(&args.amount)?;
    let amount_str = format_usdc(amount);
    let iris = IrisClient::new(iris_environment(source.env));

    let (protocol_fee, received_amount, estimated_time, fallback_to_standard) = match args.speed {
        TransferSpeedArg::Standard => (0u128, amount, destination.standard_time_label(), false),
        TransferSpeedArg::Fast => match iris
            .fast_fee_bps(source.cctp_domain, destination.cctp_domain)
            .await
            .map_err(|e| CliError::Iris(e.to_string()))?
        {
            Some(fee_bps_scaled) => {
                let fee = amount::calculate_fee(amount, fee_bps_scaled)?;
                let received = amount
                    .checked_sub(fee)
                    .ok_or_else(|| CliError::InvalidInput("amount too small for fee".into()))?;
                (fee, received, destination.fast_time_label(), false)
            }
            None => (0u128, amount, destination.standard_time_label(), true),
        },
    };

    let out = EstimateOutput {
        source_chain: source.name,
        destination_chain: destination.name,
        source_domain: source.cctp_domain,
        destination_domain: destination.cctp_domain,
        speed: effective_speed(args.speed, fallback_to_standard),
        amount: amount_str,
        protocol_fee: format_usdc(protocol_fee),
        received_amount: format_usdc(received_amount),
        estimated_time,
        fallback_to_standard,
    };

    if args.json {
        print_json(&out)?;
    } else {
        println!(
            "Transfer: {} -> {}",
            out.source_chain, out.destination_chain
        );
        println!("Amount: {} USDC", out.amount);
        println!("Speed: {}", out.speed);
        println!("Protocol fee: {} USDC", out.protocol_fee);
        println!("Recipient receives: {} USDC", out.received_amount);
        println!("Estimated time: {}", out.estimated_time);
        if out.fallback_to_standard {
            println!("Note: fast tier unavailable, estimate fell back to standard.");
        }
    }

    Ok(())
}

fn effective_speed(speed: TransferSpeedArg, fallback_to_standard: bool) -> &'static str {
    if fallback_to_standard {
        TransferSpeedArg::Standard.as_str()
    } else {
        speed.as_str()
    }
}

fn iris_environment(env: BridgeEnvironment) -> Environment {
    match env {
        BridgeEnvironment::Mainnet => Environment::Mainnet,
        BridgeEnvironment::Testnet => Environment::Testnet,
    }
}
