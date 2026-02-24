use anchor_lang::prelude::*;
use pyth_sdk_solana::state::SolanaPriceAccount;

use crate::{
  constants::{
    MAX_ORACLE_EXPONENT_ABS, ORACLE_BACKEND_MOCK, ORACLE_BACKEND_PYTH_PUSH, ORACLE_PRICE_TARGET_DECIMALS, SLOT_TIME_MS_ESTIMATE
  }, error::LaminarError, invariants::assert_oracle_freshness_and_confidence, math::{BPS_PRECISION, mul_div_up}, state::GlobalState
};

/// Canonical pricing tuple consumed by vault logic.
///
/// - `price_safe_usd` is conservative (EMA - conf) and is used for solvency/CR-sensitive checks.
/// - `price_redeem_usd` is EMA and is used for user-facing redeem conversion.
/// - `price_ema_usd` and `confidence_usd` are emitted for observability.
#[derive(Clone, Copy, Debug, Default)]
pub struct OraclePricing {
  pub price_safe_usd: u64,
  pub price_redeem_usd: u64,
  pub price_ema_usd: u64,
  pub confidence_usd: u64,
  pub uncertainty_index_bps: u64,
}

/// Load oracle pricing and update cached snapshot in `GlobalState` when needed.
///
/// Backends:
/// - MOCK: validates cached snapshot staleness/confidence and derives safe price.
/// - PYTH_PUSH: reads configured feed account from remaining accounts, derives EMA/conf/safe,
///   then refreshes cached fields + uncertainty index
pub fn load_oracle_pricing_in_place<'info>(
  global_state: &mut GlobalState,
  clock: &Clock,
  remaining_accounts: &[AccountInfo<'info>],
) -> Result<OraclePricing> {
  match global_state.oracle_backend {
    ORACLE_BACKEND_MOCK => pricing_from_cached_snapshot(global_state, clock),
    ORACLE_BACKEND_PYTH_PUSH => {
      let pricing = load_pyth_pricing(
        global_state.pyth_sol_usd_price_account,
        global_state.max_oracle_staleness_slots,
        global_state.max_conf_bps,
        clock,
        remaining_accounts,
      )?;

      global_state.mock_sol_price_usd = pricing.price_ema_usd;
      global_state.mock_oracle_confidence_usd = pricing.confidence_usd;
      global_state.last_oracle_update_slot = clock.slot;
      global_state.uncertainty_index_bps = pricing.uncertainty_index_bps;

      Ok(pricing)
    }

    _ => err!(LaminarError::UnsupportedOracleBackend),
  }
}

/// Read-only safe-price quote path for `get_safe_price`.
/// 
/// This never mutates `GlobalState`, but it still enforces freshness/confidence checks.
pub fn quote_safe_price<'info> (
  global_state: &mut GlobalState,
  clock: &Clock,
  remaining_accounts: &[AccountInfo<'info>],
) -> Result<OraclePricing> {
  match global_state.oracle_backend {
    ORACLE_BACKEND_MOCK => pricing_from_cached_snapshot(global_state, clock),
    ORACLE_BACKEND_PYTH_PUSH => load_pyth_pricing(
      global_state.pyth_sol_usd_price_account,
      global_state.max_oracle_staleness_slots,
      global_state.max_conf_bps,
      clock,
      remaining_accounts,
    ),
    _ => err!(LaminarError::UnsupportedOracleBackend),
  }
}

fn pricing_from_cached_snapshot(global_state: &mut GlobalState, clock: &Clock) -> Result<OraclePricing> {
  assert_oracle_freshness_and_confidence(
    clock.slot, 
    global_state.last_oracle_update_slot, 
    global_state.max_oracle_staleness_slots, 
    global_state.mock_sol_price_usd, 
    global_state.mock_oracle_confidence_usd, 
    global_state.max_conf_bps,
  )?;

  let safe = global_state
    .mock_sol_price_usd
    .checked_sub(global_state.mock_oracle_confidence_usd)
    .ok_or(LaminarError::SafePriceInvalid)?;
  require!(safe > 0, LaminarError::SafePriceInvalid);

  let uncertainty_index_bps = mul_div_up(
    global_state.mock_oracle_confidence_usd,
    BPS_PRECISION,
    global_state.mock_sol_price_usd,
  )
  .ok_or(LaminarError::ArithmeticOverflow)?;

  Ok(OraclePricing {
    price_safe_usd: safe,
    price_redeem_usd: global_state.mock_sol_price_usd,
    price_ema_usd: global_state.mock_sol_price_usd,
    confidence_usd: global_state.mock_oracle_confidence_usd,
    uncertainty_index_bps,
  })

}

fn load_pyth_pricing<'info>(
  configured_feed: Pubkey,
  max_oracle_staleness_slots: u64,
  max_conf_bps: u64,
  clock: &Clock,
  remaining_accounts: &[AccountInfo<'info>],
) -> Result<OraclePricing> {
  require!(configured_feed != Pubkey::default(), LaminarError::OracleFeedNotSet);

  let feed_ai = find_remaining_account(remaining_accounts, &configured_feed)
    .ok_or(LaminarError::OracleFeedAccountMissing)?;
  require!(feed_ai.key == &configured_feed, LaminarError::OracleFeedMismatch);

  let price_feed = SolanaPriceAccount::account_info_to_feed(&feed_ai)
    .map_err(|_| LaminarError::OracleFeedLoadFailed)?;

  let max_age_secs = slots_to_seconds(max_oracle_staleness_slots)?;
  let ema_price = price_feed
    .get_ema_price_no_older_than(clock.unix_timestamp, max_age_secs)
    .ok_or(LaminarError::OraclePriceStale)?;

  pricing_from_ema_components(ema_price.price, ema_price.conf, ema_price.expo, max_conf_bps)
}

fn pricing_from_ema_components(
  ema_price: i64,
  ema_confidence: u64,
  ema_expo: i32,
  max_conf_bps: u64,
) -> Result<OraclePricing> {
  let price_ema_usd = scale_signed_to_target_down(ema_price, ema_expo, ORACLE_PRICE_TARGET_DECIMALS)?;
  let confidence_usd =
    scale_unsigned_to_target_up(ema_confidence, ema_expo, ORACLE_PRICE_TARGET_DECIMALS)?;

  require!(confidence_usd < price_ema_usd, LaminarError::SafePriceInvalid);

  let uncertainty_index_bps = mul_div_up(confidence_usd, BPS_PRECISION, price_ema_usd)
    .ok_or(LaminarError::ArithmeticOverflow)?;
  require!(
    uncertainty_index_bps <= max_conf_bps,
    LaminarError::OracleConfidenceTooHigh
  );

  let price_safe_usd = price_ema_usd
    .checked_sub(confidence_usd)
    .ok_or(LaminarError::SafePriceInvalid)?;
  require!(price_safe_usd > 0, LaminarError::SafePriceInvalid);

  Ok(OraclePricing {
    price_safe_usd,
    price_redeem_usd: price_ema_usd,
    price_ema_usd,
    confidence_usd,
    uncertainty_index_bps,
  })
}

fn find_remaining_account<'info>(
  remaining_accounts: &[AccountInfo<'info>],
  expected_key: &Pubkey,
) -> Option<AccountInfo<'info>> {
  remaining_accounts
    .iter()
    .find(|acc| acc.key == expected_key)
    .cloned()
}

fn slots_to_seconds(max_slots: u64) -> Result<u64> {
  require!(max_slots > 0, LaminarError::InvalidParameter);
  let ms = max_slots
    .checked_mul(SLOT_TIME_MS_ESTIMATE)
    .ok_or(LaminarError::ArithmeticOverflow)?;
  let secs = ms
    .checked_add(999)
    .ok_or(LaminarError::ArithmeticOverflow)?
    / 1000;
  Ok(secs.max(1))
}

fn scale_signed_to_target_down(value: i64, expo: i32, target_decimals: i32) -> Result<u64> {
  require!(value > 0, LaminarError::OraclePriceInvalid);
  validate_exponent(expo)?;

  let unsigned = value as u128;
  let shift = expo
    .checked_add(target_decimals)
    .ok_or(LaminarError::ArithmeticOverflow)?;

  let scaled = if shift >= 0 {
    unsigned
      .checked_mul(pow10_u128(shift as u32)?)
      .ok_or(LaminarError::ArithmeticOverflow)?
  } else {
    let divisor = pow10_u128((-shift) as u32)?;
    unsigned
      .checked_div(divisor)
      .ok_or(LaminarError::ArithmeticOverflow)?
  };

  let out = u64::try_from(scaled).map_err(|_| LaminarError::ArithmeticOverflow)?;
  require!(out > 0, LaminarError::OraclePriceInvalid);
  Ok(out)
}

fn scale_unsigned_to_target_up(value: u64, expo: i32, target_decimals: i32) -> Result<u64> {
  validate_exponent(expo)?;

  let unsigned = value as u128;
  let shift = expo
    .checked_add(target_decimals)
    .ok_or(LaminarError::ArithmeticOverflow)?;

  let scaled = if shift >= 0 {
    unsigned
      .checked_mul(pow10_u128(shift as u32)?)
      .ok_or(LaminarError::ArithmeticOverflow)?
  } else {
    let divisor = pow10_u128((-shift) as u32)?;
    unsigned
      .checked_add(divisor.checked_sub(1).ok_or(LaminarError::ArithmeticOverflow)?)
      .ok_or(LaminarError::ArithmeticOverflow)?
      .checked_div(divisor)
      .ok_or(LaminarError::ArithmeticOverflow)?
  };

  u64::try_from(scaled).map_err(|_| LaminarError::ArithmeticOverflow.into())
}

fn validate_exponent(expo: i32) -> Result<()> {
  let abs = expo
    .checked_abs()
    .ok_or(LaminarError::OracleExponentOutOfRange)?;
  require!(abs <= MAX_ORACLE_EXPONENT_ABS, LaminarError::OracleExponentOutOfRange);
  Ok(())
}

fn pow10_u128(exp: u32) -> Result<u128> {
  let mut out = 1u128;
  for _ in 0..exp {
    out = out
      .checked_mul(10)
      .ok_or(LaminarError::ArithmeticOverflow)?;
  }
  Ok(out)
}