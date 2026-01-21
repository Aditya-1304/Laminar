//! Pure mathematical functions for laminar protocol
//! All functions are deterministic and use fixed-point arithmetic
//! No external depedencies, fully testable in isolation


use anchor_lang::prelude::*;

pub const SOL_PRECISION: u64 = 1_000_000_000;
pub const USD_PRECISION: u64 = 1_000_000;
pub const BPS_PRECISION: u64 = 10_000; // 1e4 basis points (100% = 10000 bps)


/// Multiply two u64 values and divide by a third, rounding up
/// Used for conservative calculations that favor protocol solvency
/// Returns None in overflow
pub fn mul_div_up(a: u64, b: u64, c: u64) -> Option<u64> {
  if c == 0 {
    return None;
  }

  let result = (a as u128)
    .checked_mul(b as u128)?
    .checked_add((c - 1) as u128)? // we add (c - 1) before division to round up
    .checked_div(c as u128)?;

  u64::try_from(result).ok()
}

/// Multiply two u64 values and divide by a third, rounding DOWN
/// Used for conservative calculations that favor protocol solvency 
/// Returns None on Overflow
pub fn mul_div_down(a: u64, b: u64, c: u64) -> Option<u64> {
  if c == 0 {
    return None;
  }

  let result = (a as u128)
    .checked_mul(b as u128)?
    .checked_div(c as u128)?;

  u64::try_from(result).ok()
}

/// Compute total value locked (TVL) in SOL terms
/// 
/// # Arguments 
/// * `collateral_lamports` - Total collateral held by protocol in lamports
/// * `lst_to_sol_rate` - Exchange rate from LST to SOL (with SOL_PRECISION)
/// 
/// # Returns
/// TVL in lamports (SOL base units)
pub fn compute_tvl_sol(collateral_lamports: u64, lst_to_sol_rate: u64) -> Option<u64> {
  mul_div_down(collateral_lamports, lst_to_sol_rate, SOL_PRECISION)
}

/// Compute SOL-denominated liabilities owed to amUSD holders
/// 
/// # Arguments
/// * `amusd_supply` - Total amUSD supply (with USD_PRECISION)
/// * `sol_price_usd` - SOL price in USD (with USD_PRECISION)
/// 
/// # Returns 
/// Liability in lamports (SOL base units)
pub fn compute_liability_sol(amusd_suuply: u64, sol_price_usd: u64) -> Option<u64> {
  if sol_price_usd == 0 {
    return None;
  }

  // Convert amUSD (USD terms) to SOL terms
  // liability_sol = (amusd_supply / sol_price_usd) * SOL_PRECISION
  mul_div_down(amusd_suuply, SOL_PRECISION, sol_price_usd)
}