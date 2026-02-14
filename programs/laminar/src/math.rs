//! Pure mathematical functions for laminar protocol
//! All functions are deterministic and use fixed-point arithmetic
//! No external depedencies, fully testable in isolation


// use anchor_lang::prelude::*;

pub use crate::constants::{
    SOL_PRECISION,
    USD_PRECISION,
    BPS_PRECISION,
    MIN_LST_DEPOSIT,
    MIN_AMUSD_MINT,
    MIN_ASOL_MINT,
    MIN_NAV_LAMPORTS,
    MAX_FEE_MULTIPLIER_BPS,
};


/// Multiply two u64 values and divide by a third, rounding up
/// Used for conservative calculations that favor protocol solvency
/// Returns None in overflow
#[inline]
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
#[inline]
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
#[inline]
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
/// Liability in lamports (SOL base units), rounded up for conservative solvency accounting.
pub fn compute_liability_sol(amusd_supply: u64, sol_price_usd: u64) -> Option<u64> {
  if sol_price_usd == 0 {
    return None;
  }

  // Convert amUSD (USD terms) to SOL terms
  // Conservative: liabilities must round up, never down
  // liability_sol = (amusd_supply / sol_price_usd) * SOL_PRECISION
  mul_div_up(amusd_supply, SOL_PRECISION, sol_price_usd)
}

/// Compute determisnistic rounding delta between conservative and user outputs
/// 
/// # Arguments
/// * `conservative_output` - output from conservative rounding and user (down)
/// * `user_favoring_output` - output from user-favoring path (up)
/// 
/// # Returns
/// Delta in output units (`user_favoring_output` - `conservative output`)
pub fn compute_rounding_delta_units(
  conservative_output: u64,
  use_favouring_output: u64,
) -> Option<u64> {
  use_favouring_output.checked_sub(conservative_output)
}


/// Convert micro-USD dust to lamports with conservative round-up.
///
/// # Arguments
/// * `usd_dust_micro` - Dust in micro-USD units
/// * `sol_price_usd` - SOL price in micro-USD
///
/// # Returns
/// Lamports equivalent, rounded up.
pub fn usd_dust_to_lamports_up(usd_dust_micro: u64, sol_price_usd: u64) -> Option<u64> {
  if usd_dust_micro == 0 {
    return Some(0);
  }

  mul_div_up(usd_dust_micro, SOL_PRECISION, sol_price_usd)
}

/// Convert LST-unit dust to lamports with conservative round-up.
///
/// # Arguments
/// * `lst_dust_units` - Dust in LST base units (9 decimals)
/// * `lst_to_sol_rate` - LST->SOL rate (SOL_PRECISION scale)
///
/// # Returns
/// Lamports equivalent, rounded up.
pub fn lst_dust_to_lamports_up(lst_dust_units: u64, lst_to_sol_rate: u64) -> Option<u64> {
  if lst_dust_units == 0 {
    return Some(0);
  }
  mul_div_up(lst_dust_units, lst_to_sol_rate, SOL_PRECISION)
}

/// Convert aSOL-unit dust to lamports with conservative round-up.
///
/// # Arguments
/// * `asol_dust_units` - Dust in aSOL base units (9 decimals)
/// * `nav_lamports` - aSOL NAV in lamports per aSOL (SOL_PRECISION scale)
///
/// # Returns
/// Lamports equivalent, rounded up.
pub fn asol_dust_to_lamports_up(asol_dust_units: u64, nav_lamports: u64) -> Option<u64> {
  if asol_dust_units == 0 {
    return Some(0);
  }
  mul_div_up(asol_dust_units, nav_lamports, SOL_PRECISION)
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

/// Compute accounting equity in SOL lamports, including rounding reserve
/// 
/// Accounting identity: 
/// E = TVL - Liability - RoundingReserve
/// 
/// # Arguments
/// * `tvl` - Total collateral value in lamports
/// * `liability` - Total liabilities in lamports
/// * `rounding_reserve` - Non-claimable rounding reserve in lamports 
/// 
/// # Returns 
/// Signed accounting equity in lamports (can be negative during insolvency)
pub fn compute_accounting_equity_sol(
  tvl: u64,
  liability: u64,
  rounding_reserve: u64,
) -> Option<i128> {
  (tvl as i128)
    .checked_sub(liability as i128)?
    .checked_sub(rounding_reserve as i128)
}

/// Compute claimable equity in SOL lamports 
/// 
/// This clamps negative accounting equity to zero for user-claim purposes 
/// 
/// # Arguments 
/// * `tvl` - Total collateral value in lamports
/// * `liability` - Total liabilities in lamports
/// * `rounding_reserve` - Non-claimable rounding reserve in lamports 
/// 
/// # Returns 
/// Claimable equity in lamports (`max(accounting_equity, 0`).
pub fn compute_claimable_equity_sol(
  tvl: u64,
  liability: u64,
  rounding_reserve: u64,
) -> Option<u64> {
  let equity = compute_accounting_equity_sol(tvl, liability, rounding_reserve)?;
  if equity <= 0 {
    Some(0)
  } else {
    u64::try_from(equity).ok()
  }
}

/// Compute Net Asset Value (NAV) of amUSD in SOL terms
/// amUSD is always worth $1, so NAV = SOL_PRECISION / SOL_price
/// 
/// # Arguments 
/// * `sol_price_usd` - SOL price in USD (with USD_PRECISION, e.g., 100_000_000 = $100)
/// 
/// # Returns 
/// NAV in lamports per amUSD unit (1e6 amUSD = $1)
/// Example: If SOL = $100, 1 amUSD (1e6 units) = 10_000_000 lamports (0.01 SOL)
pub fn nav_amusd(sol_price_usd: u64) -> Option<u64> {
  if sol_price_usd == 0 {
    return None;
  }

  // nav = (1 USD * SOL_PRECISION) / sol_price_usd
  // Since 1 USD = USD_PRECISION, we get: 
  mul_div_down(USD_PRECISION, SOL_PRECISION, sol_price_usd)
}

/// Compute reserve-aware NAV of aSOL using claimable equity.
/// 
/// # Arguments
/// * `tvl` - Total value locked in lamports
/// * `liability` - Total liabilties in lamports
/// * `asol_supply` - Total aSOL supply (SOL_PRECISION units)
/// 
/// # Returns
/// NAV in lamports per aSOL unit.
/// Returns `None` if `asol_supply == 0` 
pub fn nav_asol_with_reserve(
  tvl: u64,
  liability: u64,
  rounding_reserve: u64,
  asol_supply: u64,
) -> Option<u64> {
  if asol_supply == 0 {
    return None;
  }

  let claimable_equity = compute_claimable_equity_sol(tvl, liability, rounding_reserve)?;
  mul_div_down(claimable_equity, SOL_PRECISION, asol_supply)
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
/// Returns Some(0) if TVL < liability (prevents negative equity propagation)
/// Returns None if aSOL supply is 0 (edge case: first mint)
pub fn nav_asol(tvl: u64, liability: u64, asol_supply: u64) -> Option<u64> {
    if asol_supply == 0 {
        return None; // First mint case - will be handled specially
    }
    
    let equity = compute_equity_sol(tvl, liability);
    
    // nav_asol = equity / asol_supply (both in lamports)
    mul_div_down(equity, SOL_PRECISION, asol_supply)
}

/// fee action categories used by the dynamic fee engine
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeeAction {
  /// Debt creation (risk-increasing when CR is low)
  AmusdMint,
  /// Debt repayment (risk-reducing when CR is low)
  AmUSDRedeem,
  /// Equity injection (risk-reducing when CR is low)
  AsolMint,
  /// Equity exit (risk-increasing when CR is low)
  AsolRedeem,
}

impl FeeAction {
  /// True for actions that should become more expensive when CR deteriorates.
  #[inline]
  pub fn is_risk_increasing(self) -> bool {
    matches!(self, FeeAction::AmusdMint | FeeAction::AsolRedeem)
  }
}

/// uncertainty_up_bps = BPS + (uncertainity_intex_bps * BPS / k)
pub const UNCERTAINTY_K_BPS: u64 = 1_000;

/// Clamp helper for u64.
#[inline]
pub fn clamp_u64(value: u64, min_value: u64, max_value:u64) -> u64 {
  value.max(min_value).min(max_value)
} 

/// Derive CR-based multiplier
/// 
/// Returns multiplier in bps (10_000 = 1.0x)
pub fn derive_cr_multiplier_bps(
  action: FeeAction,
  cr_bps: u64,
  min_cr_bps: u64,
  target_cr_bps: u64,
  fee_min_multiplier_bps: u64,
  fee_max_multiplier_bps: u64,
) -> Option<u64> {
  // Enforce parameter bounds required by spec.
  if min_cr_bps >= target_cr_bps {
    return None;
  }
  if fee_min_multiplier_bps > BPS_PRECISION || fee_max_multiplier_bps < BPS_PRECISION {
    return None;
  }
  if fee_min_multiplier_bps > fee_max_multiplier_bps {
    return None;
  }

  if cr_bps == u64::MAX {
    return Some(BPS_PRECISION);
  }

  let cr_mult = if action.is_risk_increasing() {
    // expensive when CR falls.
    if cr_bps >= target_cr_bps {
      BPS_PRECISION
    } else if cr_bps <= min_cr_bps {
      fee_max_multiplier_bps
    } else {
      let distance = target_cr_bps.checked_sub(cr_bps)?;
      let range = target_cr_bps.checked_sub(min_cr_bps)?;
      let delta = fee_max_multiplier_bps.checked_sub(BPS_PRECISION)?;

      let step = mul_div_down(distance, delta, range)?;
      BPS_PRECISION.checked_add(step)?
    }
  } else {
    // cheap when CR falls.
    if cr_bps >= target_cr_bps {
      BPS_PRECISION
    } else if cr_bps <= min_cr_bps {
      fee_min_multiplier_bps
    } else {
      let distance = target_cr_bps.checked_sub(cr_bps)?;
      let range = target_cr_bps.checked_sub(min_cr_bps)?;
      let delta = BPS_PRECISION.checked_sub(fee_min_multiplier_bps)?;

      let step = mul_div_down(distance, delta, range)?;
      BPS_PRECISION.checked_sub(step)?
    }
  };

  Some(clamp_u64(cr_mult, fee_min_multiplier_bps, fee_max_multiplier_bps))
}

/// Derive uncertainity multiplier
/// 
/// for risk increasing actions: applies capped uncertainity uplift
/// for risk reducing actions: neutral
pub fn derive_uncertainty_multiplier_bps(
  action: FeeAction,
  uncertainty_index_bps: u64,
  uncertainty_max_bps: u64,
) -> Option<u64> {
  if uncertainty_max_bps < BPS_PRECISION {
    return None;
  }

  if !action.is_risk_increasing(){
    return Some(BPS_PRECISION);
  }

  let uncertainity_delta = mul_div_down(uncertainty_index_bps, BPS_PRECISION, UNCERTAINTY_K_BPS)?;
  let unc_up = BPS_PRECISION.checked_add(uncertainity_delta)?;

  Some(clamp_u64(unc_up, BPS_PRECISION, uncertainty_max_bps))
}

/// CR and uncertainity multiplier wuth precedence and clamps
pub fn compose_fee_multiplier_bps(
  action: FeeAction,
  cr_multiplier_bps: u64,
  unc_multipier_bps: u64,
  fee_min_multiplier_bps: u64,
  fee_max_multiplier_bps: u64,
) -> Option<u64> {
  if fee_min_multiplier_bps > BPS_PRECISION || fee_max_multiplier_bps < BPS_PRECISION {
    return None;
  }

  if fee_min_multiplier_bps > fee_max_multiplier_bps {
    return None;
  }

  let mut total = mul_div_down(cr_multiplier_bps, unc_multipier_bps, BPS_PRECISION)?;

  if action.is_risk_increasing() {
    total = total.max(BPS_PRECISION)
  } else {
    total = total.min(BPS_PRECISION);
  }

  total = clamp_u64(total, fee_min_multiplier_bps, fee_max_multiplier_bps);
  Some(total)
}

/// final dynamic fee in bps for a canonical action
/// 
/// Effective fee = floor(base_fee_bps * multiplier_total_bps / BPS)
pub fn compute_dynamic_fee_bps(
  base_fee_bps: u64,
  action: FeeAction,
  cr_bps: u64,
  min_cr_bps: u64,
  target_cr_bps: u64,
  fee_min_multiplier_bps: u64,
  fee_max_multiplier_bps: u64,
  uncertainty_index_bps: u64,
  uncertainty_max_bps: u64,
) -> Option<u64> {
  if base_fee_bps == 0 {
    return Some(0);
  }

  let cr_multiplier = derive_cr_multiplier_bps(action, cr_bps, min_cr_bps, target_cr_bps, fee_min_multiplier_bps, fee_max_multiplier_bps)?;

  let unc_multiplier = derive_uncertainty_multiplier_bps(action, uncertainty_index_bps, uncertainty_max_bps)?;

  let total_multplier = compose_fee_multiplier_bps(action, cr_multiplier, unc_multiplier, fee_min_multiplier_bps, fee_max_multiplier_bps)?;

  mul_div_down(base_fee_bps, total_multplier, BPS_PRECISION)
}

/// Dynamic fee adjustment when CR deteriorates (CR < target)
/// - For actions that should become MORE expensive when CR is low
/// - Returns base fee when CR >= target or if CR is infinite (no debt)
pub fn fee_bps_increase_when_low(
  base_fee_bps: u64,
  cr_bps: u64,
  target_cr_bps: u64,
) -> u64 {
  if base_fee_bps == 0 {
    return 0;
  }
  if cr_bps == u64::MAX || cr_bps >= target_cr_bps {
    return base_fee_bps;
  }

  // Scale up: fee = base * (target / cr)
  let scaled = mul_div_up(base_fee_bps, target_cr_bps, cr_bps).unwrap_or(base_fee_bps);
  let max_fee = mul_div_down(base_fee_bps, MAX_FEE_MULTIPLIER_BPS, BPS_PRECISION)
    .unwrap_or(u64::MAX);

  scaled.min(max_fee)
}

/// Dynamic fee adjustment when CR deteriorates (CR < target)
/// - For actions that should become CHEAPER when CR is low
/// - Returns base fee when CR >= target or if CR is infinite (no debt)
pub fn fee_bps_decrease_when_low(
  base_fee_bps: u64,
  cr_bps: u64,
  target_cr_bps: u64,
) -> u64 {
  if base_fee_bps == 0 {
    return 0;
  }
  if cr_bps == u64::MAX || cr_bps >= target_cr_bps {
    return base_fee_bps;
  }

  // Scale down: fee = base * (cr / target)
  mul_div_down(base_fee_bps, cr_bps, target_cr_bps).unwrap_or(0)
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

    #[test]
    fn test_fee_bps_increase_when_low() {
        let base = 100u64;
        let target = 15_000u64;

        // At or above target, fee stays base
        assert_eq!(fee_bps_increase_when_low(base, 15_000, target), base);
        assert_eq!(fee_bps_increase_when_low(base, 20_000, target), base);

        // Below target, fee scales up: base * (target / cr)
        assert_eq!(fee_bps_increase_when_low(base, 10_000, target), 150);

        // Extreme low CR should be capped by MAX_FEE_MULTIPLIER_BPS (4x)
        assert_eq!(fee_bps_increase_when_low(base, 1_000, target), 400);
    }

    #[test]
    fn test_fee_bps_decrease_when_low() {
        let base = 100u64;
        let target = 15_000u64;

        // At or above target, fee stays base
        assert_eq!(fee_bps_decrease_when_low(base, 15_000, target), base);
        assert_eq!(fee_bps_decrease_when_low(base, 20_000, target), base);

        // Below target, fee scales down: base * (cr / target)
        assert_eq!(fee_bps_decrease_when_low(base, 10_000, target), 66);

        // Very low CR can reduce fee to zero
        assert_eq!(fee_bps_decrease_when_low(base, 0, target), 0);
    }

    #[test]
    fn test_compute_liability_sol_rounds_up_fractional_case() {
        // $1 / $3 => 333_333_333.333... lamports, must ceil.
        let amusd_supply = USD_PRECISION;
        let sol_price = 3 * USD_PRECISION;

        let liability = compute_liability_sol(amusd_supply, sol_price).unwrap();
        assert_eq!(liability, 333_333_334);
    }

    #[test]
    fn test_compute_rounding_delta_units() {
        assert_eq!(compute_rounding_delta_units(100, 100), Some(0));
        assert_eq!(compute_rounding_delta_units(100, 101), Some(1));
    }

    #[test]
    fn test_usd_dust_to_lamports_up() {
        // 1 micro-USD at $100/SOL => 10 lamports (ceil)
        let lamports = usd_dust_to_lamports_up(1, 100 * USD_PRECISION).unwrap();
        assert_eq!(lamports, 10);
    }

        #[test]
    fn test_dynamic_fee_curve_all_actions_green_yellow_red() {
        let base = 100u64;
        let min_cr = 13_000u64;
        let target_cr = 15_000u64;
        let mmin = 5_000u64;  // 0.5x
        let mmax = 20_000u64; // 2.0x
        let unc_idx = 0u64;
        let unc_max = 20_000u64;

        // Green (CR >= target): base fee
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmusdMint, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(100)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolRedeem, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(100)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmUSDRedeem, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(100)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolMint, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(100)
        );

        // Yellow midpoint CR=14_000 (halfway): 1.5x for risk-increasing, 0.75x for risk-reducing.
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmusdMint, 14_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(150)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolRedeem, 14_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(150)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmUSDRedeem, 14_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(75)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolMint, 14_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(75)
        );

        // Red (CR <= min): clamp to extreme multipliers.
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmusdMint, 12_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(200)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolRedeem, 12_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(200)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmUSDRedeem, 12_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(50)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolMint, 12_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(50)
        );
    }

    #[test]
    fn test_uncertainty_applies_only_to_risk_increasing_actions() {
        let base = 100u64;
        let min_cr = 13_000u64;
        let target_cr = 15_000u64;
        let mmin = 10_000u64;
        let mmax = 40_000u64;

        // uncertainty_up = 10_000 + floor(500 * 10_000 / 1000) = 15_000 (1.5x)
        let unc_idx = 500u64;
        let unc_max = 20_000u64;

        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmusdMint, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(150)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolRedeem, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(150)
        );

        // Risk reducing should stay neutral under uncertainty.
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AmUSDRedeem, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(100)
        );
        assert_eq!(
            compute_dynamic_fee_bps(base, FeeAction::AsolMint, 16_000, min_cr, target_cr, mmin, mmax, unc_idx, unc_max),
            Some(100)
        );
    }

    #[test]
    fn test_invalid_multiplier_bounds_fail() {
        let result = compute_dynamic_fee_bps(
            100,
            FeeAction::AmusdMint,
            14_000,
            13_000,
            15_000,
            12_000, 
            9_000,  
            0,
            20_000,
        );
        assert!(result.is_none());
    }


}
