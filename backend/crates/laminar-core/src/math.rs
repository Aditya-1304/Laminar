use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::{
    AmusdBaseUnits, AsolBaseUnits, BasisPoints, Lamports, LstBaseUnits, LstToSolRate, MicroUsd,
    NavLamports,
};

pub const SOL_PRECISION: u64 = 1_000_000_000;
pub const USD_PRECISION: u64 = 1_000_000;
pub const BPS_PRECISION: u64 = 10_000;

pub const MIN_LST_DEPOSIT: u64 = 100_000;
pub const MIN_AMUSD_MINT: u64 = 1_000;
pub const MIN_ASOL_MINT: u64 = 1_000_000;
pub const MIN_NAV_LAMPORTS: u64 = 1_000;
pub const MAX_FEE_MULTIPLIER_BPS: u64 = 40_000;
pub const UNCERTAINTY_K_BPS: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MathError {
    #[error("division by zero")]
    DivisionByZero,
    #[error("arithmetic overflow")]
    Overflow,
    #[error("invalid parameter: {0}")]
    InvalidParameter(&'static str),
    #[error("rounding reserve cap exceeded")]
    RoundingReserveExceeded,
    #[error("rounding reserve underflow")]
    RoundingReserveUnderflow,
}

pub type MathResult<T> = Result<T, MathError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeeAction {
    AmusdMint,
    AmusdRedeem,
    AsolMint,
    AsolRedeem,
}

impl FeeAction {
    pub fn is_risk_increasing(self) -> bool {
        matches!(self, Self::AmusdMint | Self::AsolRedeem)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HaircutRedemptionPreview {
    pub haircut_bps: BasisPoints,
    pub sol_out_lamports: Lamports,
    pub lst_out_amount: LstBaseUnits,
    pub fee_in_amusd: AmusdBaseUnits,
}

#[inline]
pub fn mul_div_up(a: u64, b: u64, c: u64) -> MathResult<u64> {
    if c == 0 {
        return Err(MathError::DivisionByZero);
    }

    let result = (a as u128)
        .checked_mul(b as u128)
        .ok_or(MathError::Overflow)?
        .checked_add((c - 1) as u128)
        .ok_or(MathError::Overflow)?
        .checked_div(c as u128)
        .ok_or(MathError::Overflow)?;

    u64::try_from(result).map_err(|_| MathError::Overflow)
}

#[inline]
pub fn mul_div_down(a: u64, b: u64, c: u64) -> MathResult<u64> {
    if c == 0 {
        return Err(MathError::DivisionByZero);
    }

    let result = (a as u128)
        .checked_mul(b as u128)
        .ok_or(MathError::Overflow)?
        .checked_div(c as u128)
        .ok_or(MathError::Overflow)?;

    u64::try_from(result).map_err(|_| MathError::Overflow)
}

#[inline]
pub fn clamp_u64(value: u64, min_value: u64, max_value: u64) -> u64 {
    value.max(min_value).min(max_value)
}

pub fn compute_tvl_sol(
    lst_amount: LstBaseUnits,
    lst_to_sol_rate: LstToSolRate,
) -> MathResult<Lamports> {
    mul_div_down(lst_amount, lst_to_sol_rate, SOL_PRECISION)
}

pub fn compute_liability_sol(
    amusd_supply: AmusdBaseUnits,
    sol_price_usd: MicroUsd,
) -> MathResult<Lamports> {
    mul_div_up(amusd_supply, SOL_PRECISION, sol_price_usd)
}

pub fn compute_equity_sol(tvl: Lamports, liability: Lamports) -> Lamports {
    tvl.saturating_sub(liability)
}

pub fn compute_accounting_equity_sol(
    tvl: Lamports,
    liability: Lamports,
    rounding_reserve: Lamports,
) -> MathResult<i128> {
    (tvl as i128)
        .checked_sub(liability as i128)
        .and_then(|value| value.checked_sub(rounding_reserve as i128))
        .ok_or(MathError::Overflow)
}

pub fn compute_claimable_equity_sol(
    tvl: Lamports,
    liability: Lamports,
    rounding_reserve: Lamports,
) -> MathResult<Lamports> {
    let accounting_equity = compute_accounting_equity_sol(tvl, liability, rounding_reserve)?;
    if accounting_equity <= 0 {
        Ok(0)
    } else {
        u64::try_from(accounting_equity).map_err(|_| MathError::Overflow)
    }
}

pub fn compute_cr_bps(tvl: Lamports, liability: Lamports) -> BasisPoints {
    if liability == 0 {
        return u64::MAX;
    }

    mul_div_down(tvl, BPS_PRECISION, liability).unwrap_or(u64::MAX)
}

pub fn compute_rounding_delta_units(
    conservative_output: u64,
    user_favoring_output: u64,
) -> MathResult<u64> {
    user_favoring_output
        .checked_sub(conservative_output)
        .ok_or(MathError::InvalidParameter(
            "user_favoring_output must be >= conservative_output",
        ))
}

pub fn usd_dust_to_lamports_up(
    usd_dust_micro: MicroUsd,
    sol_price_usd: MicroUsd,
) -> MathResult<Lamports> {
    if usd_dust_micro == 0 {
        return Ok(0);
    }

    mul_div_up(usd_dust_micro, SOL_PRECISION, sol_price_usd)
}

pub fn lst_dust_to_lamports_up(
    lst_dust_units: LstBaseUnits,
    lst_to_sol_rate: LstToSolRate,
) -> MathResult<Lamports> {
    if lst_dust_units == 0 {
        return Ok(0);
    }

    mul_div_up(lst_dust_units, lst_to_sol_rate, SOL_PRECISION)
}

pub fn asol_dust_to_lamports_up(
    asol_dust_units: AsolBaseUnits,
    nav_lamports: NavLamports,
) -> MathResult<Lamports> {
    if asol_dust_units == 0 {
        return Ok(0);
    }

    mul_div_up(asol_dust_units, nav_lamports, SOL_PRECISION)
}

pub fn nav_amusd(sol_price_usd: MicroUsd) -> MathResult<NavLamports> {
    mul_div_down(USD_PRECISION, SOL_PRECISION, sol_price_usd)
}

pub fn nav_asol_without_reserve(
    tvl: Lamports,
    liability: Lamports,
    asol_supply: AsolBaseUnits,
) -> MathResult<Option<NavLamports>> {
    if asol_supply == 0 {
        return Ok(None);
    }

    let equity = compute_equity_sol(tvl, liability);
    Ok(Some(mul_div_down(equity, SOL_PRECISION, asol_supply)?))
}

pub fn nav_asol_with_reserve(
    tvl: Lamports,
    liability: Lamports,
    rounding_reserve: Lamports,
    asol_supply: AsolBaseUnits,
) -> MathResult<Option<NavLamports>> {
    if asol_supply == 0 {
        return Ok(None);
    }

    let claimable_equity = compute_claimable_equity_sol(tvl, liability, rounding_reserve)?;
    Ok(Some(mul_div_down(
        claimable_equity,
        SOL_PRECISION,
        asol_supply,
    )?))
}

pub fn derive_rounding_bound_lamports(
    k_lamports: u64,
    k_usd: u64,
    sol_price_usd: MicroUsd,
) -> MathResult<Lamports> {
    if sol_price_usd == 0 {
        return Err(MathError::InvalidParameter("sol_price_usd must be > 0"));
    }

    let lamports_per_micro_usd = mul_div_up(SOL_PRECISION, 1, sol_price_usd)?;
    let usd_component = (k_usd as u128)
        .checked_mul(lamports_per_micro_usd as u128)
        .ok_or(MathError::Overflow)?;

    let bound = (k_lamports as u128)
        .checked_add(usd_component)
        .ok_or(MathError::Overflow)?;

    u64::try_from(bound).map_err(|_| MathError::Overflow)
}

pub fn balance_sheet_difference_lamports(
    tvl: Lamports,
    liability: Lamports,
    accounting_equity: i128,
    rounding_reserve: Lamports,
) -> MathResult<u128> {
    let lhs = tvl as i128;
    let rhs = (liability as i128)
        .checked_add(accounting_equity)
        .and_then(|value| value.checked_add(rounding_reserve as i128))
        .ok_or(MathError::Overflow)?;

    Ok(if lhs >= rhs {
        (lhs - rhs) as u128
    } else {
        (rhs - lhs) as u128
    })
}

pub fn balance_sheet_holds(
    tvl: Lamports,
    liability: Lamports,
    accounting_equity: i128,
    rounding_reserve: Lamports,
    rounding_bound_lamports: Lamports,
) -> MathResult<bool> {
    let diff =
        balance_sheet_difference_lamports(tvl, liability, accounting_equity, rounding_reserve)?;

    Ok(diff <= rounding_bound_lamports as u128)
}

pub fn credit_rounding_reserve(
    current_rounding_reserve: Lamports,
    credit_lamports: Lamports,
    max_rounding_reserve: Lamports,
) -> MathResult<Lamports> {
    let next = current_rounding_reserve
        .checked_add(credit_lamports)
        .ok_or(MathError::Overflow)?;

    if next > max_rounding_reserve {
        return Err(MathError::RoundingReserveExceeded);
    }

    Ok(next)
}

pub fn debit_rounding_reserve(
    current_rounding_reserve: Lamports,
    debit_lamports: Lamports,
) -> MathResult<Lamports> {
    current_rounding_reserve
        .checked_sub(debit_lamports)
        .ok_or(MathError::RoundingReserveUnderflow)
}

pub fn derive_cr_multiplier_bps(
    action: FeeAction,
    cr_bps: BasisPoints,
    min_cr_bps: BasisPoints,
    target_cr_bps: BasisPoints,
    fee_min_multiplier_bps: BasisPoints,
    fee_max_multiplier_bps: BasisPoints,
) -> MathResult<BasisPoints> {
    if min_cr_bps >= target_cr_bps {
        return Err(MathError::InvalidParameter(
            "min_cr_bps must be < target_cr_bps",
        ));
    }

    if fee_min_multiplier_bps > BPS_PRECISION {
        return Err(MathError::InvalidParameter(
            "fee_min_multiplier_bps must be <= 10_000",
        ));
    }

    if fee_max_multiplier_bps < BPS_PRECISION {
        return Err(MathError::InvalidParameter(
            "fee_max_multiplier_bps must be >= 10_000",
        ));
    }

    if fee_min_multiplier_bps > fee_max_multiplier_bps {
        return Err(MathError::InvalidParameter(
            "fee_min_multiplier_bps must be <= fee_max_multiplier_bps",
        ));
    }

    if cr_bps == u64::MAX {
        return Ok(BPS_PRECISION);
    }

    let raw = if action.is_risk_increasing() {
        if cr_bps >= target_cr_bps {
            BPS_PRECISION
        } else if cr_bps <= min_cr_bps {
            fee_max_multiplier_bps
        } else {
            let distance = target_cr_bps
                .checked_sub(cr_bps)
                .ok_or(MathError::Overflow)?;
            let range = target_cr_bps
                .checked_sub(min_cr_bps)
                .ok_or(MathError::Overflow)?;
            let delta = fee_max_multiplier_bps
                .checked_sub(BPS_PRECISION)
                .ok_or(MathError::Overflow)?;

            BPS_PRECISION
                .checked_add(mul_div_down(distance, delta, range)?)
                .ok_or(MathError::Overflow)?
        }
    } else if cr_bps >= target_cr_bps {
        BPS_PRECISION
    } else if cr_bps <= min_cr_bps {
        fee_min_multiplier_bps
    } else {
        let distance = target_cr_bps
            .checked_sub(cr_bps)
            .ok_or(MathError::Overflow)?;
        let range = target_cr_bps
            .checked_sub(min_cr_bps)
            .ok_or(MathError::Overflow)?;
        let delta = BPS_PRECISION
            .checked_sub(fee_min_multiplier_bps)
            .ok_or(MathError::Overflow)?;

        BPS_PRECISION
            .checked_sub(mul_div_down(distance, delta, range)?)
            .ok_or(MathError::Overflow)?
    };

    Ok(clamp_u64(
        raw,
        fee_min_multiplier_bps,
        fee_max_multiplier_bps,
    ))
}

pub fn derive_uncertainty_multiplier_bps(
    action: FeeAction,
    uncertainty_index_bps: BasisPoints,
    uncertainty_max_bps: BasisPoints,
) -> MathResult<BasisPoints> {
    if uncertainty_max_bps < BPS_PRECISION {
        return Err(MathError::InvalidParameter(
            "uncertainty_max_bps must be >= 10_000",
        ));
    }

    if !action.is_risk_increasing() {
        return Ok(BPS_PRECISION);
    }

    let uncertainty_delta = mul_div_down(uncertainty_index_bps, BPS_PRECISION, UNCERTAINTY_K_BPS)?;
    let raw = BPS_PRECISION
        .checked_add(uncertainty_delta)
        .ok_or(MathError::Overflow)?;

    Ok(clamp_u64(raw, BPS_PRECISION, uncertainty_max_bps))
}

pub fn compose_fee_multiplier_bps(
    action: FeeAction,
    cr_multiplier_bps: BasisPoints,
    uncertainty_multiplier_bps: BasisPoints,
    fee_min_multiplier_bps: BasisPoints,
    fee_max_multiplier_bps: BasisPoints,
) -> MathResult<BasisPoints> {
    if fee_min_multiplier_bps > BPS_PRECISION {
        return Err(MathError::InvalidParameter(
            "fee_min_multiplier_bps must be <= 10_000",
        ));
    }

    if fee_max_multiplier_bps < BPS_PRECISION {
        return Err(MathError::InvalidParameter(
            "fee_max_multiplier_bps must be >= 10_000",
        ));
    }

    if fee_min_multiplier_bps > fee_max_multiplier_bps {
        return Err(MathError::InvalidParameter(
            "fee_min_multiplier_bps must be <= fee_max_multiplier_bps",
        ));
    }

    let raw = mul_div_down(cr_multiplier_bps, uncertainty_multiplier_bps, BPS_PRECISION)?;

    let directional = if action.is_risk_increasing() {
        raw.max(BPS_PRECISION)
    } else {
        raw.min(BPS_PRECISION)
    };

    Ok(clamp_u64(
        directional,
        fee_min_multiplier_bps,
        fee_max_multiplier_bps,
    ))
}

pub fn compute_dynamic_fee_bps(
    base_fee_bps: BasisPoints,
    action: FeeAction,
    cr_bps: BasisPoints,
    min_cr_bps: BasisPoints,
    target_cr_bps: BasisPoints,
    fee_min_multiplier_bps: BasisPoints,
    fee_max_multiplier_bps: BasisPoints,
    uncertainty_index_bps: BasisPoints,
    uncertainty_max_bps: BasisPoints,
) -> MathResult<BasisPoints> {
    if base_fee_bps == 0 {
        return Ok(0);
    }

    let cr_multiplier = derive_cr_multiplier_bps(
        action,
        cr_bps,
        min_cr_bps,
        target_cr_bps,
        fee_min_multiplier_bps,
        fee_max_multiplier_bps,
    )?;

    let uncertainty_multiplier =
        derive_uncertainty_multiplier_bps(action, uncertainty_index_bps, uncertainty_max_bps)?;

    let total_multiplier = compose_fee_multiplier_bps(
        action,
        cr_multiplier,
        uncertainty_multiplier,
        fee_min_multiplier_bps,
        fee_max_multiplier_bps,
    )?;

    mul_div_down(base_fee_bps, total_multiplier, BPS_PRECISION)
}

pub fn apply_fee(amount: u64, fee_bps: BasisPoints) -> MathResult<(u64, u64)> {
    let fee_amount = mul_div_down(amount, fee_bps, BPS_PRECISION)?;
    let net_amount = amount.checked_sub(fee_amount).ok_or(MathError::Overflow)?;
    Ok((net_amount, fee_amount))
}

pub fn drawdown_expected(
    collateral_ratio_bps: BasisPoints,
    min_cr_bps: BasisPoints,
    stability_pool_amusd: AmusdBaseUnits,
) -> bool {
    collateral_ratio_bps < min_cr_bps && stability_pool_amusd > 0
}

pub fn insolvency_mode(collateral_ratio_bps: BasisPoints) -> bool {
    collateral_ratio_bps < BPS_PRECISION
}

pub fn haircut_bps_from_cr(collateral_ratio_bps: BasisPoints) -> BasisPoints {
    collateral_ratio_bps.min(BPS_PRECISION)
}

pub fn preview_haircut_redemption(
    amusd_in: AmusdBaseUnits,
    sol_price_redeem_usd: MicroUsd,
    lst_to_sol_rate: LstToSolRate,
    post_drawdown_cr_bps: BasisPoints,
) -> MathResult<HaircutRedemptionPreview> {
    let haircut_bps = haircut_bps_from_cr(post_drawdown_cr_bps);

    let sol_out_par = mul_div_down(amusd_in, SOL_PRECISION, sol_price_redeem_usd)?;
    let sol_out_lamports = mul_div_down(sol_out_par, haircut_bps, BPS_PRECISION)?;
    let lst_out_amount = mul_div_down(sol_out_lamports, SOL_PRECISION, lst_to_sol_rate)?;

    Ok(HaircutRedemptionPreview {
        haircut_bps,
        sol_out_lamports,
        lst_out_amount,
        fee_in_amusd: 0,
    })
}
