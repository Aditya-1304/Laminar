//! Pure mathematical functions for laminar protocol
//! All functions are deterministic and use fixed-point arithmetic
//! No external depedencies, fully testable in isolation


// use anchor_lang::prelude::*;

pub const SOL_PRECISION: u64 = 1_000_000_000;
pub const USD_PRECISION: u64 = 1_000_000;
pub const BPS_PRECISION: u64 = 10_000; // 1e4 basis points (100% = 10000 bps)
pub const MIN_LST_DEPOSIT: u64 = 100_000; // 0.0001 SOL (100k lamports)
pub const MIN_AMUSD_MINT: u64 = 1_000; // 0.001 USD (1k micro-USD)
pub const MIN_ASOL_MINT: u64 = 1_000_000; // 0.001 SOL (1M lamports)


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
pub fn compute_liability_sol(amusd_supply: u64, sol_price_usd: u64) -> Option<u64> {
  if sol_price_usd == 0 {
    return None;
  }

  // Convert amUSD (USD terms) to SOL terms
  // liability_sol = (amusd_supply / sol_price_usd) * SOL_PRECISION
  mul_div_down(amusd_supply, SOL_PRECISION, sol_price_usd)
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
/// Returns u64::MAX if liability is 0 (infinite CR - no debt exists)
pub fn compute_cr_bps(tvl: u64, liability: u64) -> u64 {
  if liability == 0 {
    return u64::MAX; // No debt = undefined CR (treated as infinite)
  }

  // CR = (TVL / Liability) * BPS_PRECISION
  mul_div_down(tvl, BPS_PRECISION, liability).unwrap_or(u64::MAX)
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
/// Returns None if TVL < liability (prevents negative equity propagation)
/// Returns None if aSOL supply is 0 (edge case: first mint)
pub fn nav_asol(tvl: u64, liability: u64, asol_supply: u64) -> Option<u64> {
    if asol_supply == 0 {
        return None; // First mint case - will be handled specially
    }
    
    let equity = compute_equity_sol(tvl, liability);
    
    // nav_asol = equity / asol_supply (both in lamports)
    mul_div_down(equity, SOL_PRECISION, asol_supply)
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mul_div_up_rounding() {
        // 10 * 3 / 4 = 7.5, should round up to 8
        assert_eq!(mul_div_up(10, 3, 4), Some(8));
        
        // Exact division should not add rounding
        assert_eq!(mul_div_up(10, 4, 4), Some(10));
    }

    #[test]
    fn test_mul_div_down_rounding() {
        // 10 * 3 / 4 = 7.5, should round down to 7
        assert_eq!(mul_div_down(10, 3, 4), Some(7));
        
        // Exact division
        assert_eq!(mul_div_down(10, 4, 4), Some(10));
    }

    #[test]
    fn test_mul_div_zero_divisor() {
        // Division by zero should return None
        assert_eq!(mul_div_up(10, 3, 0), None);
        assert_eq!(mul_div_down(10, 3, 0), None);
    }

    #[test]
    fn test_compute_cr_bps_basic() {
        // TVL = 200 SOL, Liability = 100 SOL
        // CR = 200 / 100 = 200% = 20000 bps
        let tvl = 200 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        assert_eq!(compute_cr_bps(tvl, liability), 20_000);
    }

    #[test]
    fn test_compute_cr_bps_exactly_150_percent() {
        // TVL = 150 SOL, Liability = 100 SOL
        // CR = 150% = 15000 bps
        let tvl = 150 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        assert_eq!(compute_cr_bps(tvl, liability), 15_000);
    }

    #[test]
    fn test_compute_cr_bps_undercollateralized() {
        // TVL = 120 SOL, Liability = 100 SOL
        // CR = 120% = 12000 bps
        let tvl = 120 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        assert_eq!(compute_cr_bps(tvl, liability), 12_000);
    }

    #[test]
    fn test_compute_cr_bps_zero_liability() {
        // No debt = CR is undefined, return 0
        let tvl = 100 * SOL_PRECISION;
        assert_eq!(compute_cr_bps(tvl, 0), u64::MAX);
    }

    #[test]
    fn test_compute_equity_sol_positive() {
        // TVL = 200 SOL, Liability = 100 SOL
        // Equity = 100 SOL
        let tvl = 200 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        assert_eq!(compute_equity_sol(tvl, liability), 100 * SOL_PRECISION);
    }

    #[test]
    fn test_compute_equity_sol_zero_when_insolvent() {
        // TVL = 80 SOL, Liability = 100 SOL
        // Equity = 0 (not negative)
        let tvl = 80 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        assert_eq!(compute_equity_sol(tvl, liability), 0);
    }

    #[test]
    fn test_nav_asol_at_various_leverage() {
        // Scenario: TVL = 200 SOL, Liability = 100 SOL, aSOL supply = 100
        let tvl = 200 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        let asol_supply = 100 * SOL_PRECISION;
        
        // Equity = 100 SOL, NAV = 100/100 = 1 SOL per aSOL
        assert_eq!(nav_asol(tvl, liability, asol_supply), Some(SOL_PRECISION));
    }

    #[test]
    fn test_nav_asol_high_leverage() {
        // Scenario: TVL = 200 SOL, Liability = 180 SOL, aSOL supply = 20
        let tvl = 200 * SOL_PRECISION;
        let liability = 180 * SOL_PRECISION;
        let asol_supply = 20 * SOL_PRECISION;
        
        // Equity = 20 SOL, NAV = 20/20 = 1 SOL per aSOL
        assert_eq!(nav_asol(tvl, liability, asol_supply), Some(SOL_PRECISION));
    }

    #[test]
    fn test_nav_asol_zero_when_insolvent() {
        // TVL < Liability should return NAV = 0
        let tvl = 90 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        let asol_supply = 50 * SOL_PRECISION;
        
        assert_eq!(nav_asol(tvl, liability, asol_supply), Some(0));
    }

    #[test]
    fn test_nav_asol_zero_supply_edge_case() {
        // First mint case - no aSOL exists yet
        let tvl = 100 * SOL_PRECISION;
        let liability = 0;
        let asol_supply = 0;
        
        assert_eq!(nav_asol(tvl, liability, asol_supply), None);
    }

    #[test]
    fn test_simulate_40_percent_price_drop() {
        // Initial state: TVL = 200 SOL, Liability = 100 SOL
        let initial_tvl = 200 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        let asol_supply = 100 * SOL_PRECISION;
        
        // Initial CR = 200%
        assert_eq!(compute_cr_bps(initial_tvl, liability), 20_000);
        
        // Initial aSOL NAV = 1.0 SOL
        assert_eq!(nav_asol(initial_tvl, liability, asol_supply), Some(SOL_PRECISION));
        
        // Simulate 40% SOL price drop (TVL drops to 120 SOL)
        let crashed_tvl = 120 * SOL_PRECISION;
        
        // New CR = 120%
        assert_eq!(compute_cr_bps(crashed_tvl, liability), 12_000);
        
        // New aSOL NAV = (120 - 100) / 100 = 0.2 SOL
        // Equity absorbed the entire loss
        let new_nav = nav_asol(crashed_tvl, liability, asol_supply);
        assert_eq!(new_nav, Some(SOL_PRECISION / 5)); // 0.2 SOL
    }

    #[test]
    fn test_simulate_60_percent_price_drop() {
        // Initial state: TVL = 200 SOL, Liability = 100 SOL
        let initial_tvl = 200 * SOL_PRECISION;
        let liability = 100 * SOL_PRECISION;
        let asol_supply = 100 * SOL_PRECISION;
        
        // Simulate 60% SOL price drop (TVL drops to 80 SOL)
        let crashed_tvl = 80 * SOL_PRECISION;
        
        // New CR = 80% (insolvent!)
        assert_eq!(compute_cr_bps(crashed_tvl, liability), 8_000);
        
        // aSOL NAV should be 0 (TVL < Liability)
        assert_eq!(nav_asol(crashed_tvl, liability, asol_supply), Some(0));
    }

    #[test]
    fn test_apply_fee_half_percent() {
        let amount = 1_000_000;
        let fee_bps = 50; // 0.5%
        
        let (net, fee) = apply_fee(amount, fee_bps).unwrap();
        
        assert_eq!(fee, 5_000); // 0.5% of 1M
        assert_eq!(net, 995_000);
        assert_eq!(net + fee, amount); // Conservation check
    }

    #[test]
    fn test_apply_fee_zero() {
        let amount = 1_000_000;
        let fee_bps = 0;
        
        let (net, fee) = apply_fee(amount, fee_bps).unwrap();
        
        assert_eq!(fee, 0);
        assert_eq!(net, amount);
    }

    #[test]
    fn test_compute_liability_sol() {
        // amUSD supply = 100,000 (with USD_PRECISION = 1e6)
        // SOL price = $100 (with USD_PRECISION = 1e6)
        // Expected liability = 100,000 / 100 = 1,000 SOL = 1,000 * SOL_PRECISION lamports
        
        let amusd_supply = 100_000 * USD_PRECISION;
        let sol_price = 100 * USD_PRECISION;
        
        let liability = compute_liability_sol(amusd_supply, sol_price).unwrap();
        assert_eq!(liability, 1_000 * SOL_PRECISION);
    }

    #[test]
    fn test_nav_amusd() {
        // SOL price = $100
        // amUSD NAV should be 1/100 = 0.01 SOL = 0.01 * SOL_PRECISION lamports
        
        let sol_price = 100 * USD_PRECISION;
        let nav = nav_amusd(sol_price).unwrap();
        
        assert_eq!(nav, SOL_PRECISION / 100);
    }
}