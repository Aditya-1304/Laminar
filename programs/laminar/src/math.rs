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

/// Compute SOL-denominated equity owned by aSOL holders
/// 
/// # Arguments 
/// * `tvl` - Total value locked in lamports
/// * `liability` - Total liabilities in lamports
/// 
/// # Returns
/// Equity in lamports (returns 0 if TVL < liabilty to prevent negative equity) 
pub fn compute_equity_sol(tvl: u64, liability: u64) -> u64 {
  // Prevent negative equity - if insolvent, equity is zero
  tvl.saturating_sub(liability)
}


///Compute collateral ratio in basis points 
/// 
/// # Arguments 
/// * `tvl` - Total value locked in lamports
/// * `liability` - Total liabilities in lamports 
/// 
/// # Returns 
/// CR in basis points (e.g., 15000 = 150%)
/// Returns 0 if liability is 0 (edge case: no debt exists)
pub fn compute_cr_bps(tvl: u64, liability: u64) -> u64 {
  if liability == 0 {
    return 0; // No debt = undefined CR (treated as 0 for safety)
  }

  // CR = (TVL / Liability) * BPS_PRECISION
  mul_div_down(tvl, BPS_PRECISION, liability).unwrap_or(0)
}

/// Compute Net Asset Value (NAV) of amUSD in SOL terms
/// amUSD is always worth $1, so NAV = 1 / SOL_price
/// 
/// # Arguments 
/// * `sol_price_usd` - SOL price in USD (with USD_PRECISION)
/// 
/// # Returns 
/// NAV in lamports per amUSD unit (with USD_PRECISION)
pub fn nav_amusd(sol_price_usd: u64) -> Option<u64> {
  if sol_price_usd == 0 {
    return None;
  }

  // nav = (1 USD * SOL_PRECISION) / sol_price_usd
  // Since 1 USD = USD_PRECISION, we get: 
  mul_div_down(USD_PRECISION, SOL_PRECISION, sol_price_usd)
}

/// Compute Net Asset Value (NAV) of aSOL
/// aSOL represents residual equity after amUSD debt is satisfied
/// 
/// # Arguments
/// * `tvl` - Total value locked in lamports
/// * `liability` - Total liabilities in lamports
/// * `asol_supply` - Total aSOL supply (with SOL_PRECISION)
/// 
/// # Returns
/// NAV in lamports per aSOL unit
/// Returns 0 if TVL < liability (prevents negative equity propagation)
/// Returns 0 if aSOL supply is 0 (edge case: first mint)
pub fn nav_asol(tvl: u64, liability: u64, asol_supply: u64) -> u64 {
    if asol_supply == 0 {
        return 0; // First mint case - will be handled specially
    }
    
    let equity = compute_equity_sol(tvl, liability);
    
    if equity == 0 {
        return 0; // Protocol is insolvent - xSOL worthless
    }
    
    // nav_asol = equity / asol_supply (both in lamports)
    mul_div_down(equity, SOL_PRECISION, asol_supply).unwrap_or(0)
}

/// Apply a fee to an amount and return net amount + fee
/// 
/// Arguments
/// * `amount` - Gross amount before fee
/// * `fee_bps` - Fee in basis points (e.g., 50 = 0.5%)
/// 
/// # Returns 
/// (net_amount, fee_amount)
pub fn apply_fee(amount: u64, fee_bps: u64) -> Option<(u64, u64)> {
  let fee_amount = mul_div_down(amount, fee_bps, BPS_PRECISION)?;
  let net_amount = amount.checked_sub(fee_amount)?;
  Some((net_amount, fee_amount))
}