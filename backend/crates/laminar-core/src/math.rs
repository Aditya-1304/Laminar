use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    models::{
        AmusdBaseUnits, AsolBaseUnits, BasisPoints, Lamports, LstBaseUnits, LstToSolRate, MicroUsd,
        NavLamports,
    },
    normalization::normalize_collateral_ratio_bps,
    quote::{
        MintAmusdPreview, MintAmusdPreviewInput, MintAsolPreview, MintAsolPreviewInput,
        RedeemAmusdPreview, RedeemAmusdPreviewInput, RedeemAsolPreview, RedeemAsolPreviewInput,
        VaultBalanceSheet,
    },
};

pub const SOL_PRECISION: u64 = 1_000_000_000;
pub const USD_PRECISION: u64 = 1_000_000;
pub const BPS_PRECISION: u64 = 10_000;

pub const MIN_LST_DEPOSIT: u64 = 100_000;
pub const MIN_AMUSD_MINT: u64 = 1_000;
pub const MIN_ASOL_MINT: u64 = 1_000_000;
pub const MIN_PROTOCOL_TVL: u64 = 1_000_000;
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
    #[error("minting is paused")]
    MintPaused,
    #[error("redemptions are paused")]
    RedeemPaused,
    #[error("zero amount")]
    ZeroAmount,
    #[error("amount too small: {0}")]
    AmountTooSmall(&'static str),
    #[error("slippage exceeded")]
    SlippageExceeded,
    #[error("collateral ratio too low")]
    CollateralRatioTooLow,
    #[error("insolvent protocol")]
    InsolventProtocol,
    #[error("balance sheet violation")]
    BalanceSheetViolation,
    #[error("equity exists while aSOL supply is zero")]
    EquityWithoutAsolSupply,
    #[error("below minimum protocol tvl")]
    BelowMinimumTvl,
    #[error("insufficient protocol supply")]
    InsufficientSupply,
    #[error("insufficient protocol collateral")]
    InsufficientCollateral,
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

pub fn build_vault_balance_sheet(
    lst_amount: LstBaseUnits,
    amusd_supply: AmusdBaseUnits,
    asol_supply: AsolBaseUnits,
    rounding_reserve_lamports: Lamports,
    lst_to_sol_rate: LstToSolRate,
    safe_price_usd: MicroUsd,
) -> MathResult<VaultBalanceSheet> {
    let tvl_lamports = compute_tvl_sol(lst_amount, lst_to_sol_rate)?;
    let liability_lamports = if amusd_supply > 0 {
        compute_liability_sol(amusd_supply, safe_price_usd)?
    } else {
        0
    };

    let accounting_equity_lamports =
        compute_accounting_equity_sol(tvl_lamports, liability_lamports, rounding_reserve_lamports)?;

    let claimable_equity_lamports =
        compute_claimable_equity_sol(tvl_lamports, liability_lamports, rounding_reserve_lamports)?;

    let raw_cr_bps = compute_cr_bps(tvl_lamports, liability_lamports);
    let collateral_ratio_bps = normalize_collateral_ratio_bps(raw_cr_bps);

    let nav_amusd_lamports = nav_amusd(safe_price_usd)?;
    let nav_asol_lamports = nav_asol_with_reserve(
        tvl_lamports,
        liability_lamports,
        rounding_reserve_lamports,
        asol_supply,
    )?;

    Ok(VaultBalanceSheet {
        lst_amount,
        amusd_supply,
        asol_supply,
        tvl_lamports,
        liability_lamports,
        accounting_equity_lamports,
        claimable_equity_lamports,
        collateral_ratio_bps,
        nav_amusd_lamports,
        nav_asol_lamports,
        rounding_reserve_lamports,
    })
}

fn ensure_nonzero_amount(amount: u64) -> MathResult<()> {
    if amount == 0 {
        Err(MathError::ZeroAmount)
    } else {
        Ok(())
    }
}

fn ensure_min_amount(amount: u64, min_amount: u64, label: &'static str) -> MathResult<()> {
    if amount < min_amount {
        Err(MathError::AmountTooSmall(label))
    } else {
        Ok(())
    }
}

fn ensure_collateral_ratio_at_least(
    collateral_ratio_bps: Option<BasisPoints>,
    min_cr_bps: BasisPoints,
) -> MathResult<()> {
    if let Some(value) = collateral_ratio_bps {
        if value < min_cr_bps {
            return Err(MathError::CollateralRatioTooLow);
        }
    }

    Ok(())
}

fn ensure_balance_sheet_is_valid(
    sheet: &VaultBalanceSheet,
    rounding_bound_lamports: Lamports,
) -> MathResult<()> {
    let valid = balance_sheet_holds(
        sheet.tvl_lamports,
        sheet.liability_lamports,
        sheet.accounting_equity_lamports,
        sheet.rounding_reserve_lamports,
        rounding_bound_lamports,
    )?;

    if valid {
        Ok(())
    } else {
        Err(MathError::BalanceSheetViolation)
    }
}

fn ensure_min_protocol_tvl(new_lst_amount: LstBaseUnits) -> MathResult<()> {
    if new_lst_amount >= MIN_PROTOCOL_TVL || new_lst_amount == 0 {
        Ok(())
    } else {
        Err(MathError::BelowMinimumTvl)
    }
}

pub fn preview_mint_amusd(input: &MintAmusdPreviewInput) -> MathResult<MintAmusdPreview> {
    let ctx = &input.context;

    if ctx.mint_paused {
        return Err(MathError::MintPaused);
    }

    ensure_nonzero_amount(input.lst_amount)?;
    ensure_min_amount(input.lst_amount, MIN_LST_DEPOSIT, "lst_amount")?;

    let pre_mint_balance_sheet = build_vault_balance_sheet(
        ctx.current_lst_amount,
        ctx.current_amusd_supply,
        ctx.current_asol_supply,
        ctx.current_rounding_reserve_lamports,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    let old_cr_bps = compute_cr_bps(
        pre_mint_balance_sheet.tvl_lamports,
        pre_mint_balance_sheet.liability_lamports,
    );

    let sol_value_lamports = compute_tvl_sol(input.lst_amount, ctx.lst_to_sol_rate)?;
    let sol_value_up = mul_div_up(input.lst_amount, ctx.lst_to_sol_rate, SOL_PRECISION)?;

    let gross_amusd_out = mul_div_down(sol_value_lamports, ctx.safe_price_usd, SOL_PRECISION)?;
    let gross_amusd_out_up = mul_div_up(sol_value_up, ctx.safe_price_usd, SOL_PRECISION)?;

    let mint_rounding_delta_usd =
        compute_rounding_delta_units(gross_amusd_out, gross_amusd_out_up)?;
    let reserve_credit_lamports =
        usd_dust_to_lamports_up(mint_rounding_delta_usd, ctx.safe_price_usd)?;

    let fee_bps = compute_dynamic_fee_bps(
        ctx.fee_amusd_mint_bps,
        FeeAction::AmusdMint,
        old_cr_bps,
        ctx.min_cr_bps,
        ctx.target_cr_bps,
        ctx.fee_min_multiplier_bps,
        ctx.fee_max_multiplier_bps,
        ctx.uncertainty_index_bps,
        ctx.uncertainty_max_bps,
    )?;

    let (net_amusd_out, fee_amusd) = apply_fee(gross_amusd_out, fee_bps)?;
    ensure_min_amount(net_amusd_out, MIN_AMUSD_MINT, "net_amusd_out")?;

    if net_amusd_out < input.min_amusd_out {
        return Err(MathError::SlippageExceeded);
    }

    let new_lst_amount = ctx
        .current_lst_amount
        .checked_add(input.lst_amount)
        .ok_or(MathError::Overflow)?;

    let new_amusd_supply = ctx
        .current_amusd_supply
        .checked_add(gross_amusd_out)
        .ok_or(MathError::Overflow)?;

    let new_rounding_reserve = credit_rounding_reserve(
        ctx.current_rounding_reserve_lamports,
        reserve_credit_lamports,
        ctx.max_rounding_reserve_lamports,
    )?;

    let post_mint_balance_sheet = build_vault_balance_sheet(
        new_lst_amount,
        new_amusd_supply,
        ctx.current_asol_supply,
        new_rounding_reserve,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    ensure_collateral_ratio_at_least(post_mint_balance_sheet.collateral_ratio_bps, ctx.min_cr_bps)?;

    let rounding_bound_lamports = derive_rounding_bound_lamports(2, 1, ctx.safe_price_usd)?;
    ensure_balance_sheet_is_valid(&post_mint_balance_sheet, rounding_bound_lamports)?;

    Ok(MintAmusdPreview {
        lst_in: input.lst_amount,
        sol_value_lamports,
        gross_amusd_out,
        net_amusd_out,
        fee_amusd,
        fee_bps,
        reserve_credit_lamports,
        rounding_bound_lamports,
        pre_mint_balance_sheet,
        post_mint_balance_sheet,
    })
}

pub fn preview_redeem_amusd(input: &RedeemAmusdPreviewInput) -> MathResult<RedeemAmusdPreview> {
    let ctx = &input.context;

    if ctx.redeem_paused {
        return Err(MathError::RedeemPaused);
    }

    ensure_nonzero_amount(input.amusd_amount)?;
    ensure_nonzero_amount(input.min_lst_out)?;
    ensure_min_amount(input.min_lst_out, MIN_LST_DEPOSIT, "min_lst_out")?;

    let current_pre_drawdown_sheet = build_vault_balance_sheet(
        ctx.current_lst_amount,
        ctx.current_amusd_supply,
        ctx.current_asol_supply,
        ctx.current_rounding_reserve_lamports,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;
    let current_pre_drawdown_cr_bps = compute_cr_bps(
        current_pre_drawdown_sheet.tvl_lamports,
        current_pre_drawdown_sheet.liability_lamports,
    );

    let effective_lst_amount = input.effective_lst_amount.unwrap_or(ctx.current_lst_amount);
    let effective_amusd_supply = input
        .effective_amusd_supply
        .unwrap_or(ctx.current_amusd_supply);
    let effective_asol_supply = input
        .effective_asol_supply
        .unwrap_or(ctx.current_asol_supply);
    let effective_rounding_reserve = input
        .effective_rounding_reserve_lamports
        .unwrap_or(ctx.current_rounding_reserve_lamports);

    if effective_amusd_supply < input.amusd_amount {
        return Err(MathError::InsufficientSupply);
    }

    let pre_redemption_balance_sheet = build_vault_balance_sheet(
        effective_lst_amount,
        effective_amusd_supply,
        effective_asol_supply,
        effective_rounding_reserve,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    let effective_cr_bps = compute_cr_bps(
        pre_redemption_balance_sheet.tvl_lamports,
        pre_redemption_balance_sheet.liability_lamports,
    );
    let solvent_mode = !insolvency_mode(effective_cr_bps);

    let (amusd_net_burn, amusd_fee_in) = if solvent_mode {
        let fee_bps = compute_dynamic_fee_bps(
            ctx.fee_amusd_redeem_bps,
            FeeAction::AmusdRedeem,
            effective_cr_bps,
            ctx.min_cr_bps,
            ctx.target_cr_bps,
            ctx.fee_min_multiplier_bps,
            ctx.fee_max_multiplier_bps,
            ctx.uncertainty_index_bps,
            ctx.uncertainty_max_bps,
        )?;

        let (net_in, fee_in) = apply_fee(input.amusd_amount, fee_bps)?;
        ensure_nonzero_amount(net_in)?;
        (net_in, fee_in)
    } else {
        (input.amusd_amount, 0)
    };

    let sol_value_par_down = mul_div_down(amusd_net_burn, SOL_PRECISION, ctx.redeem_price_usd)?;
    let lst_par_down = mul_div_down(sol_value_par_down, SOL_PRECISION, ctx.lst_to_sol_rate)?;

    let (sol_value_gross_lamports, lst_out, reserve_debit_lamports, rounding_k_lamports, haircut) =
        if solvent_mode {
            let sol_value_up = mul_div_up(amusd_net_burn, SOL_PRECISION, ctx.redeem_price_usd)?;
            let lst_gross_up = mul_div_up(sol_value_up, SOL_PRECISION, ctx.lst_to_sol_rate)?;

            let redeem_rounding_delta_lst =
                compute_rounding_delta_units(lst_par_down, lst_gross_up)?;
            let lamport_debit =
                lst_dust_to_lamports_up(redeem_rounding_delta_lst, ctx.lst_to_sol_rate)?;

            if lamport_debit <= effective_rounding_reserve {
                (sol_value_up, lst_gross_up, lamport_debit, 2, None)
            } else {
                (sol_value_par_down, lst_par_down, 0, 2, None)
            }
        } else {
            let haircut_preview = preview_haircut_redemption(
                amusd_net_burn,
                ctx.redeem_price_usd,
                ctx.lst_to_sol_rate,
                effective_cr_bps,
            )?;

            (
                haircut_preview.sol_out_lamports,
                haircut_preview.lst_out_amount,
                0,
                3,
                Some(haircut_preview.haircut_bps),
            )
        };

    if lst_out < input.min_lst_out {
        return Err(MathError::SlippageExceeded);
    }

    let used_user_favoring_rounding = solvent_mode && reserve_debit_lamports > 0;

    let new_lst_amount = effective_lst_amount
        .checked_sub(lst_out)
        .ok_or(MathError::InsufficientCollateral)?;
    ensure_min_protocol_tvl(new_lst_amount)?;

    let new_amusd_supply = effective_amusd_supply
        .checked_sub(amusd_net_burn)
        .ok_or(MathError::InsufficientSupply)?;

    let new_rounding_reserve =
        debit_rounding_reserve(effective_rounding_reserve, reserve_debit_lamports)?;

    let post_redemption_balance_sheet = build_vault_balance_sheet(
        new_lst_amount,
        new_amusd_supply,
        effective_asol_supply,
        new_rounding_reserve,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    let rounding_bound_lamports =
        derive_rounding_bound_lamports(rounding_k_lamports, 1, ctx.safe_price_usd)?;
    ensure_balance_sheet_is_valid(&post_redemption_balance_sheet, rounding_bound_lamports)?;

    Ok(RedeemAmusdPreview {
        amusd_in: input.amusd_amount,
        amusd_net_burn,
        amusd_fee_in,
        sol_value_gross_lamports,
        lst_out,
        reserve_debit_lamports,
        rounding_bound_lamports,
        haircut_bps: haircut,
        solvent_mode,
        used_user_favoring_rounding,
        drawdown_expected: drawdown_expected(
            current_pre_drawdown_cr_bps,
            ctx.min_cr_bps,
            input.stability_pool_amusd_available,
        ),
        drawdown_rounds_executed: input.drawdown_rounds_executed,
        pre_redemption_balance_sheet,
        post_redemption_balance_sheet,
    })
}

pub fn preview_mint_asol(input: &MintAsolPreviewInput) -> MathResult<MintAsolPreview> {
    let ctx = &input.context;

    if ctx.mint_paused {
        return Err(MathError::MintPaused);
    }

    ensure_nonzero_amount(input.lst_amount)?;
    ensure_min_amount(input.lst_amount, MIN_LST_DEPOSIT, "lst_amount")?;

    let pre_mint_balance_sheet = build_vault_balance_sheet(
        ctx.current_lst_amount,
        ctx.current_amusd_supply,
        ctx.current_asol_supply,
        ctx.current_rounding_reserve_lamports,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    let old_tvl = pre_mint_balance_sheet.tvl_lamports;
    let old_liability = pre_mint_balance_sheet.liability_lamports;
    let old_cr_bps = compute_cr_bps(old_tvl, old_liability);
    let old_claimable_equity = pre_mint_balance_sheet.claimable_equity_lamports;

    let rounding_bound_lamports = derive_rounding_bound_lamports(2, 0, ctx.safe_price_usd)?;

    let bootstrap_mode = ctx.current_asol_supply == 0;
    let mut effective_rounding_reserve_before_mint_lamports = ctx.current_rounding_reserve_lamports;
    let mut orphan_equity_swept_lamports = 0;

    if bootstrap_mode {
        if old_tvl < old_liability {
            return Err(MathError::InsolventProtocol);
        }

        let lhs = old_tvl as i128;
        let rhs = (old_liability as i128)
            .checked_add(effective_rounding_reserve_before_mint_lamports as i128)
            .ok_or(MathError::Overflow)?;

        let bootstrap_diff = if lhs >= rhs {
            (lhs - rhs) as u128
        } else {
            (rhs - lhs) as u128
        };

        if bootstrap_diff > rounding_bound_lamports as u128 {
            return Err(MathError::EquityWithoutAsolSupply);
        }

        if old_claimable_equity > 0 {
            effective_rounding_reserve_before_mint_lamports =
                effective_rounding_reserve_before_mint_lamports
                    .checked_add(old_claimable_equity)
                    .ok_or(MathError::Overflow)?;

            if effective_rounding_reserve_before_mint_lamports > ctx.max_rounding_reserve_lamports {
                return Err(MathError::EquityWithoutAsolSupply);
            }

            orphan_equity_swept_lamports = old_claimable_equity;
        }
    }

    let sol_value_lamports = compute_tvl_sol(input.lst_amount, ctx.lst_to_sol_rate)?;
    let sol_value_up = mul_div_up(input.lst_amount, ctx.lst_to_sol_rate, SOL_PRECISION)?;

    let nav_before_lamports = if bootstrap_mode {
        SOL_PRECISION
    } else {
        nav_asol_with_reserve(
            old_tvl,
            old_liability,
            effective_rounding_reserve_before_mint_lamports,
            ctx.current_asol_supply,
        )?
        .ok_or(MathError::InsolventProtocol)?
    };

    if !bootstrap_mode && nav_before_lamports == 0 {
        return Err(MathError::InsolventProtocol);
    }

    let gross_asol_out = if bootstrap_mode {
        sol_value_lamports
    } else {
        mul_div_down(sol_value_lamports, SOL_PRECISION, nav_before_lamports)?
    };

    let asol_reference_up = if bootstrap_mode {
        sol_value_up
    } else {
        mul_div_up(sol_value_up, SOL_PRECISION, nav_before_lamports)?
    };

    let mint_rounding_delta_asol = compute_rounding_delta_units(gross_asol_out, asol_reference_up)?;

    let reserve_credit_lamports = if bootstrap_mode {
        mint_rounding_delta_asol
    } else {
        asol_dust_to_lamports_up(mint_rounding_delta_asol, nav_before_lamports)?
    };

    let fee_bps = compute_dynamic_fee_bps(
        ctx.fee_asol_mint_bps,
        FeeAction::AsolMint,
        old_cr_bps,
        ctx.min_cr_bps,
        ctx.target_cr_bps,
        ctx.fee_min_multiplier_bps,
        ctx.fee_max_multiplier_bps,
        ctx.uncertainty_index_bps,
        ctx.uncertainty_max_bps,
    )?;

    let (net_asol_out, fee_asol) = apply_fee(gross_asol_out, fee_bps)?;
    ensure_min_amount(net_asol_out, MIN_ASOL_MINT, "net_asol_out")?;

    if net_asol_out < input.min_asol_out {
        return Err(MathError::SlippageExceeded);
    }

    let new_lst_amount = ctx
        .current_lst_amount
        .checked_add(input.lst_amount)
        .ok_or(MathError::Overflow)?;
    let new_asol_supply = ctx
        .current_asol_supply
        .checked_add(gross_asol_out)
        .ok_or(MathError::Overflow)?;
    let new_rounding_reserve = credit_rounding_reserve(
        effective_rounding_reserve_before_mint_lamports,
        reserve_credit_lamports,
        ctx.max_rounding_reserve_lamports,
    )?;

    let post_mint_balance_sheet = build_vault_balance_sheet(
        new_lst_amount,
        ctx.current_amusd_supply,
        new_asol_supply,
        new_rounding_reserve,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    ensure_balance_sheet_is_valid(&post_mint_balance_sheet, rounding_bound_lamports)?;

    Ok(MintAsolPreview {
        lst_in: input.lst_amount,
        sol_value_lamports,
        gross_asol_out,
        net_asol_out,
        fee_asol,
        fee_bps,
        nav_before_lamports,
        bootstrap_mode,
        orphan_equity_swept_lamports,
        effective_rounding_reserve_before_mint_lamports,
        reserve_credit_lamports,
        rounding_bound_lamports,
        pre_mint_balance_sheet,
        post_mint_balance_sheet,
    })
}

pub fn preview_redeem_asol(input: &RedeemAsolPreviewInput) -> MathResult<RedeemAsolPreview> {
    let ctx = &input.context;

    if ctx.redeem_paused {
        return Err(MathError::RedeemPaused);
    }

    ensure_nonzero_amount(input.asol_amount)?;
    ensure_nonzero_amount(input.min_lst_out)?;
    ensure_min_amount(input.min_lst_out, MIN_LST_DEPOSIT, "min_lst_out")?;

    if ctx.current_asol_supply < input.asol_amount {
        return Err(MathError::InsufficientSupply);
    }

    let pre_redemption_balance_sheet = build_vault_balance_sheet(
        ctx.current_lst_amount,
        ctx.current_amusd_supply,
        ctx.current_asol_supply,
        ctx.current_rounding_reserve_lamports,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    let old_cr_bps = compute_cr_bps(
        pre_redemption_balance_sheet.tvl_lamports,
        pre_redemption_balance_sheet.liability_lamports,
    );

    let fee_bps = compute_dynamic_fee_bps(
        ctx.fee_asol_redeem_bps,
        FeeAction::AsolRedeem,
        old_cr_bps,
        ctx.min_cr_bps,
        ctx.target_cr_bps,
        ctx.fee_min_multiplier_bps,
        ctx.fee_max_multiplier_bps,
        ctx.uncertainty_index_bps,
        ctx.uncertainty_max_bps,
    )?;

    let (asol_net_burn, asol_fee_in) = apply_fee(input.asol_amount, fee_bps)?;
    ensure_nonzero_amount(asol_net_burn)?;

    let nav_before_lamports = pre_redemption_balance_sheet
        .nav_asol_lamports
        .ok_or(MathError::InsolventProtocol)?;

    if nav_before_lamports == 0 {
        return Err(MathError::InsolventProtocol);
    }

    let solvent_mode = !insolvency_mode(old_cr_bps);

    let sol_value_down = mul_div_down(asol_net_burn, nav_before_lamports, SOL_PRECISION)?;
    let lst_gross_down = mul_div_down(sol_value_down, SOL_PRECISION, ctx.lst_to_sol_rate)?;

    let (sol_value_gross_lamports, lst_out, reserve_debit_lamports) = if solvent_mode {
        let sol_value_up = mul_div_up(asol_net_burn, nav_before_lamports, SOL_PRECISION)?;
        let lst_gross_up = mul_div_up(sol_value_up, SOL_PRECISION, ctx.lst_to_sol_rate)?;

        let redeem_rounding_delta_lst = compute_rounding_delta_units(lst_gross_down, lst_gross_up)?;
        let lamport_debit =
            lst_dust_to_lamports_up(redeem_rounding_delta_lst, ctx.lst_to_sol_rate)?;

        if lamport_debit <= ctx.current_rounding_reserve_lamports {
            (sol_value_up, lst_gross_up, lamport_debit)
        } else {
            (sol_value_down, lst_gross_down, 0)
        }
    } else {
        (sol_value_down, lst_gross_down, 0)
    };

    if lst_out < input.min_lst_out {
        return Err(MathError::SlippageExceeded);
    }

    let used_user_favoring_rounding = solvent_mode && reserve_debit_lamports > 0;

    let new_lst_amount = ctx
        .current_lst_amount
        .checked_sub(lst_out)
        .ok_or(MathError::InsufficientCollateral)?;
    ensure_min_protocol_tvl(new_lst_amount)?;

    let new_asol_supply = ctx
        .current_asol_supply
        .checked_sub(asol_net_burn)
        .ok_or(MathError::InsufficientSupply)?;

    let new_rounding_reserve = debit_rounding_reserve(
        ctx.current_rounding_reserve_lamports,
        reserve_debit_lamports,
    )?;

    let post_redemption_balance_sheet = build_vault_balance_sheet(
        new_lst_amount,
        ctx.current_amusd_supply,
        new_asol_supply,
        new_rounding_reserve,
        ctx.lst_to_sol_rate,
        ctx.safe_price_usd,
    )?;

    ensure_collateral_ratio_at_least(
        post_redemption_balance_sheet.collateral_ratio_bps,
        ctx.min_cr_bps,
    )?;

    let rounding_bound_lamports = derive_rounding_bound_lamports(2, 0, ctx.safe_price_usd)?;
    ensure_balance_sheet_is_valid(&post_redemption_balance_sheet, rounding_bound_lamports)?;

    Ok(RedeemAsolPreview {
        asol_in: input.asol_amount,
        asol_net_burn,
        asol_fee_in,
        sol_value_gross_lamports,
        lst_out,
        nav_before_lamports,
        solvent_mode,
        used_user_favoring_rounding,
        reserve_debit_lamports,
        rounding_bound_lamports,
        pre_redemption_balance_sheet,
        post_redemption_balance_sheet,
    })
}
