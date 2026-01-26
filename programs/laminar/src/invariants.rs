//! Invariant assertions for Laminar protocol
//! These are the non-negotiable rules that protect protocol solvency
//! Every state-changing instruction MUST call these before committing 

use anchor_lang::prelude::*;

/// Assert that the balance sheet equation holds: TVL = Liability + Equity
/// This is the foundational invariant of the entire protocol
/// 
/// # Arguments 
/// * `tvl` - Total value locked in lamports
/// * `liability` - Total liabilities in lamports 
/// * `equity` - Total equity in lamports
pub fn assert_balance_sheet_holds(tvl: u64, liability: u64, equity: u64) -> Result<()> {
  const MAX_ROUNDING_ERROR: u64 = 10;
   // lamports
  let total = liability.checked_add(equity)
    .ok_or(ProtocolError::ArithmeticOverflow)?;

  let diff = tvl.abs_diff(total);
  require!(
    diff <= MAX_ROUNDING_ERROR,
    ProtocolError::BalanceSheetViolation
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
    ProtocolError::CollateralRatioTooLow
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
    ProtocolError::NegativeEquity
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
    ProtocolError::ZeroSupply
  );
  msg!("Supply check passed for action: {}", action_name);
  Ok(())
}

/// Protocol specific error codes 
#[error_code]
pub enum ProtocolError {
    #[msg("Balance sheet invariant violated: TVL != Liability + Equity")]
    BalanceSheetViolation,
    
    #[msg("Collateral ratio below minimum threshold")]
    CollateralRatioTooLow,
    
    #[msg("Negative equity detected: TVL < Liability")]
    NegativeEquity,
    
    #[msg("Supply is zero - cannot perform this operation")]
    ZeroSupply,

    #[msg("Arithmetic overflow in invariant check")]
    ArithmeticOverflow,
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_sheet_holds_valid() {
        // TVL = 200, Liability = 100, Equity = 100
        // 200 = 100 + 100 ✓
        let result = assert_balance_sheet_holds(200, 100, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_balance_sheet_violation() {
        // TVL = 200, Liability = 100, Equity = 50
        // 200 ≠ 150 ✗
        let result = assert_balance_sheet_holds(200, 100, 50);
        assert!(result.is_err());
    }

    #[test]
    fn test_cr_above_minimum_valid() {
        // CR = 15000 (150%), min = 13000 (130%)
        let result = assert_cr_above_minimum(15_000, 13_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cr_above_minimum_exact() {
        // CR = 13000 (130%), min = 13000 (130%)
        // Exact match should pass
        let result = assert_cr_above_minimum(13_000, 13_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cr_below_minimum() {
        // CR = 12000 (120%), min = 13000 (130%)
        let result = assert_cr_above_minimum(12_000, 13_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_negative_equity_valid() {
        // TVL = 200, Liability = 100
        let result = assert_no_negative_equity(200, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_negative_equity_exact() {
        // TVL = 100, Liability = 100
        // Zero equity is valid
        let result = assert_no_negative_equity(100, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_negative_equity_fails() {
        // TVL = 80, Liability = 100
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
}