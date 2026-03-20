use crate::error::{CliError, Result};

const USDC_DECIMALS: usize = 6;
const BPS_PRECISION: u128 = 10_000;
const BPS_DIVISOR: u128 = 10_000 * BPS_PRECISION;

pub fn parse_usdc_amount(value: &str) -> Result<u128> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliError::InvalidInput("amount is required".into()));
    }

    let mut parts = trimmed.split('.');
    let int = parts
        .next()
        .ok_or_else(|| CliError::InvalidInput("invalid amount".into()))?;
    let frac = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(CliError::InvalidInput("invalid decimal amount".into()));
    }
    if !int.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err(CliError::InvalidInput("amount must be numeric".into()));
    }

    if frac.len() > USDC_DECIMALS {
        return Err(CliError::InvalidInput(format!(
            "amount supports at most {USDC_DECIMALS} decimal places"
        )));
    }

    let mut frac_buf = frac.to_string();
    while frac_buf.len() < USDC_DECIMALS {
        frac_buf.push('0');
    }

    let int_part: u128 = int
        .parse()
        .map_err(|_| CliError::InvalidInput("amount too large".into()))?;
    let frac_part: u128 = frac_buf
        .parse()
        .map_err(|_| CliError::InvalidInput("amount too large".into()))?;

    Ok(int_part * 1_000_000 + frac_part)
}

pub fn format_usdc(amount: u128) -> String {
    let int = amount / 1_000_000;
    let frac = amount % 1_000_000;
    format!("{int}.{frac:06}")
}

pub fn calculate_fee(amount: u128, scaled_fee_bps: u128) -> Result<u128> {
    if scaled_fee_bps > BPS_DIVISOR {
        return Err(CliError::InvalidInput("fee rate exceeds 100%".into()));
    }
    Ok(amount.saturating_mul(scaled_fee_bps).div_ceil(BPS_DIVISOR))
}
