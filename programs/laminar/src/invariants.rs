//! Invariant assertions for Laminar protocol
//! These are the non-negotiable rules that protect protocol solvency
//! Every state-changing instruction MUST call these before committing 

use anchor_lang::prelude::*;

use crate::{error::LaminarError, math::{SOL_PRECISION, mul_div_up}};


/// Derive deterministic rounding bound in lamports for a given instruction path.
/// 
/// Bound formula: 
/// rounding_bound_lamports = k_lamports + ceil(k_usd * lamports_per_microUDSD)
/// where lamports_per_microUSD = ceil(SOL_PRECISION / sol_price_usd)
/// 
/// # Arguments
/// * `k_lamports` - Number of fixed-point divisions with lamports output units
/// * `k_usd` = Number of fixed-point divisons with microUsd output units
/// * `sol_price_usd` - Conservative SOL price in microUSD
/// 
/// # Returns
///  Deterministic per-instruction rounding bound in lamports.
pub fn derive_rounding_bound_lamports(
  k_lamports: u64,
  k_usd: u64,
  sol_price_usd: u64,
) -> Result<u64> {
  require!(sol_price_usd > 0, LaminarError::InvalidParameter);

  let lamports_per_micro_usd = mul_div_up(SOL_PRECISION, 1, sol_price_usd).ok_or(LaminarError::ArithmeticOverflow)?;

  let usd_component_u128 = (k_usd as u128)
    .checked_mul(lamports_per_micro_usd as u128)
    .ok_or(LaminarError::ArithmeticOverflow)?;

  let usd_component = u64::try_from(usd_component_u128)
    .map_err(|_| LaminarError::ArithmeticOverflow)?;

  let bound = k_lamports
    .checked_add(usd_component)
    .ok_or(LaminarError::ArithmeticOverflow)?;

  Ok(bound)
}

/// Assert reserve cap is not exceeded.
/// 
/// # Arguments 
/// * `current` - Current rounding reserve in lamports
/// * `max` - configured reserve cap in lamports
pub fn assert_rounding_reserve_within_cap(current: u64, max: u64) -> Result<()> {
  require!(current <= max, LaminarError::RoundingReserveExceeded);
  Ok(())
}


/// Assert that the balance sheet equation holds: TVL = Liability + Equity
/// This is the foundational invariant of the entire protocol
/// 
/// # Arguments 
/// * `tvl` - Total value locked in lamports
/// * `liability` - Total liabilities in lamports 
/// * `equity` - Total equity in lamports
pub fn assert_balance_sheet_holds(tvl: u64, liability: u64, accounting_equity: i128, rounding_reserve: u64, rounding_bound_lamports: u64) -> Result<()> { 
  let lhs = tvl as i128;

  let rhs = (liability as i128)
    .checked_add(accounting_equity)
    .and_then(|v| v.checked_add(rounding_reserve as i128))
    .ok_or(LaminarError::ArithmeticOverflow)?;

  let diff: u128 = if lhs >= rhs {
    (lhs - rhs) as u128
  } else {
      (rhs - lhs) as u128
  };

  require!(
    diff <= rounding_bound_lamports as u128,
    LaminarError::BalanceSheetViolation
  );
  Ok(())
}

/// Assert that collateral ratio is above minimum threshold
/// prevents the protocol from becoming undercollateralized
/// 
/// # Arguments 
/// * `cr_bps` - Current collateral ratio is above the minimum threshold 
/// * `min_cr_bps` - Minimum allowed CR in basis points
pub fn assert_cr_above_minimum(cr_bps: u64, min_cr_bps: u64) -> Result<()> {

  if cr_bps == u64::MAX {
    return Ok(());
  }
  require!(
    cr_bps >= min_cr_bps,
    LaminarError::CollateralRatioTooLow
  );
  Ok(())
}


/// Assert that TVL is always >= liablilty (no negative equity)
/// Prevents bad debt propagtion
/// 
/// # Arguments 
/// * `tvl` - Total value locked in lamports
/// * `liability` - Total liabilities in lamports
pub fn assert_no_negative_equity(tvl: u64, liability: u64) -> Result<()> {
  require!(
    tvl >= liability,
    LaminarError::NegativeEquity
  );
  Ok(())
}

/// Assert that supply is non-zero before operations that require division 
/// Prevents division by zero panics 
/// 
/// # Arguments 
/// * `supply` - The supply value to check
/// * `action_name` - Name of the action for error context
pub fn assert_supply_nonzero(supply: u64, action_name: &str) -> Result<()> {
  require!(
    supply > 0,
    LaminarError::ZeroSupply
  );
  msg!("Supply check passed for action: {}", action_name);
  Ok(())
}

/// Credit rounding reserve by deterministic dust amount.
///
/// # Arguments
/// * `current_rounding_reserve` - Current reserve in lamports
/// * `credit_lamports` - Lamports to add
/// * `max_rounding_reserve` - Hard cap for reserve growth
///
/// # Returns
/// Updated reserve value.
pub fn credit_rounding_reserve(
  current_rounding_reserve: u64,
  credit_lamports: u64,
  max_rounding_reserve: u64,
) -> Result<u64> {
  let next = current_rounding_reserve
    .checked_add(credit_lamports)
    .ok_or(LaminarError::ArithmeticOverflow)?;

  require!(next <= max_rounding_reserve, LaminarError::RoundingReserveExceeded);
  Ok(next)
}

/// Debit rounding reserve when user-favoring rounding is applied.
///
/// # Arguments
/// * `current_rounding_reserve` - Current reserve in lamports
/// * `debit_lamports` - Lamports to subtract
///
/// # Returns
/// Updated reserve value.
pub fn debit_rounding_reserve(
  current_rounding_reserve: u64,
  debit_lamports: u64,
) -> Result<u64> {
  let next = current_rounding_reserve
    .checked_sub(debit_lamports)
    .ok_or(LaminarError::RoundingReserveUnderflow)?;

  Ok(next)
}

/// Uses stack height instead of instruction index. so normal setup 
/// instructions in the same tnx is allowed
pub fn assert_not_cpi_context()-> Result<()> {
  let stack_height = anchor_lang::solana_program::instruction::get_stack_height();

  require!(
    stack_height <= anchor_lang::solana_program::instruction::TRANSACTION_LEVEL_STACK_HEIGHT, LaminarError::InvalidCPIContext
  );

  Ok(())
}

/// Protocol specific error codes 
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_sheet_holds_exact() {
        // TVL = 10 SOL, L = 5 SOL, E = 5 SOL, R = 0
        let tvl = 10_000_000_000u64;
        let liability = 5_000_000_000u64;
        let accounting_equity = 5_000_000_000i128;
        let rounding_reserve = 0u64;
        let rounding_bound_lamports = 0u64;

        let result = assert_balance_sheet_holds(
            tvl,
            liability,
            accounting_equity,
            rounding_reserve,
            rounding_bound_lamports,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_balance_sheet_violation() {
        // TVL = 10 SOL, RHS = 9 SOL, diff = 1 SOL, bound = 0 => fail
        let tvl = 10_000_000_000u64;
        let liability = 5_000_000_000u64;
        let accounting_equity = 4_000_000_000i128;
        let rounding_reserve = 0u64;
        let rounding_bound_lamports = 0u64;

        let result = assert_balance_sheet_holds(
            tvl,
            liability,
            accounting_equity,
            rounding_reserve,
            rounding_bound_lamports,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_balance_sheet_within_explicit_bound() {
        // TVL = 10 SOL, RHS = TVL - 100 lamports, bound = 100 => pass
        let tvl = 10_000_000_000u64;
        let liability = 5_000_000_000u64;
        let accounting_equity = 4_999_999_900i128;
        let rounding_reserve = 0u64;
        let rounding_bound_lamports = 100u64;

        let result = assert_balance_sheet_holds(
            tvl,
            liability,
            accounting_equity,
            rounding_reserve,
            rounding_bound_lamports,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_rounding_reserve_within_cap_valid() {
        let result = assert_rounding_reserve_within_cap(5_000, 10_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rounding_reserve_within_cap_fails() {
        let result = assert_rounding_reserve_within_cap(10_001, 10_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_rounding_bound_lamports_basic() {
        // price = 100 USD = 100_000_000 microUSD
        // lamports_per_microUSD = ceil(1_000_000_000 / 100_000_000) = 10
        // bound = k_lamports + k_usd * lamports_per_microUSD = 2 + 1*10 = 12
        let bound = derive_rounding_bound_lamports(2, 1, 100_000_000).unwrap();
        assert_eq!(bound, 12);
    }

    #[test]
    fn test_cr_above_minimum_valid() {
        let result = assert_cr_above_minimum(15_000, 13_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cr_above_minimum_exact() {
        let result = assert_cr_above_minimum(13_000, 13_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cr_below_minimum() {
        let result = assert_cr_above_minimum(12_000, 13_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_negative_equity_valid() {
        let result = assert_no_negative_equity(200, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_negative_equity_exact() {
        let result = assert_no_negative_equity(100, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_negative_equity_fails() {
        let result = assert_no_negative_equity(80, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_supply_nonzero_valid() {
        let result = assert_supply_nonzero(1000, "test_action");
        assert!(result.is_ok());
    }

    #[test]
    fn test_supply_zero_fails() {
        let result = assert_supply_nonzero(0, "test_action");
        assert!(result.is_err());
    }

    #[test]
    fn test_credit_rounding_reserve_valid() {
        let result = credit_rounding_reserve(100, 25, 200).unwrap();
        assert_eq!(result, 125);
    }

    #[test]
    fn test_credit_rounding_reserve_cap_violation() {
        let result = credit_rounding_reserve(180, 30, 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_debit_rounding_reserve_valid() {
        let result = debit_rounding_reserve(100, 25).unwrap();
        assert_eq!(result, 75);
    }

    #[test]
    fn test_debit_rounding_reserve_underflow() {
        let result = debit_rounding_reserve(10, 11);
        assert!(result.is_err());
    }

}
