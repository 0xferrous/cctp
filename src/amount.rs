use alloy_primitives::{
    U256,
    utils::{format_units, parse_units},
};

use crate::error::{CliError, Result};

const USDC_DECIMALS: u8 = 6;
const BPS_PRECISION: u128 = 10_000;
const BPS_DIVISOR: u128 = 10_000 * BPS_PRECISION;

pub fn parse_usdc_amount(value: &str) -> Result<U256> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliError::InvalidInput("amount is required".into()));
    }

    parse_units(trimmed, USDC_DECIMALS)
        .map(Into::into)
        .map_err(|e| CliError::InvalidInput(format!("invalid USDC amount: {e}")))
}

pub fn format_usdc(amount: U256) -> String {
    format_units(amount, USDC_DECIMALS).expect("USDC amount formatting should not fail")
}

pub fn calculate_fee(amount: U256, scaled_fee_bps: u128) -> Result<U256> {
    if scaled_fee_bps > BPS_DIVISOR {
        return Err(CliError::InvalidInput("fee rate exceeds 100%".into()));
    }

    let divisor = U256::from(BPS_DIVISOR);
    let numerator = amount
        .saturating_mul(U256::from(scaled_fee_bps))
        .saturating_add(divisor - U256::from(1));
    Ok(numerator / divisor)
}
