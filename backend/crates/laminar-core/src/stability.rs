use thiserror::Error;

use crate::{
    math::{
        build_vault_balance_sheet, compute_cr_bps, compute_liability_sol, mul_div_down,
        nav_asol_with_reserve, MathError, MathResult, MIN_AMUSD_MINT, SOL_PRECISION,
    },
    normalization::normalize_collateral_ratio_bps,
    quote::{
        DebtEquitySwapPreview, DebtEquitySwapPreviewInput, DepositAmusdToStabilityPoolPreview,
        DepositAmusdToStabilityPoolPreviewInput, DrawdownPreview, DrawdownPreviewInput,
        HarvestYieldPreview, HarvestYieldPreviewInput, StabilityPoolInventory,
        StabilityPoolQuoteContext, VaultQuoteContext, WithdrawUnderlyingPreview,
        WithdrawUnderlyingPreviewInput,
    },
};

#[derive(Debug, Error)]
pub enum StabilityPreviewError {
    #[error(transparent)]
    Math(#[from] MathError),
    #[error("stability pool withdrawals are paused")]
    StabilityWithdrawalsPaused,
    #[error("stability pool is empty")]
    StabilityPoolEmpty,
    #[error("no conversion needed")]
    NoConversionNeeded,
    #[error("conversion output too small")]
    ConversionOutputTooSmall,
    #[error("conversion cap exceeded")]
    ConversionCapExceeded,
}

pub type StabilityPreviewResult<T> = Result<T, StabilityPreviewError>;

pub fn compute_stability_pool_value_sol(
    total_amusd: u64,
    total_asol: u64,
    price_safe_usd: u64,
    nav_asol_lamports: u64,
) -> MathResult<u64> {
    let amusd_component = mul_div_down(total_amusd, SOL_PRECISION, price_safe_usd)?;

    let asol_component = if total_asol == 0 || nav_asol_lamports == 0 {
        0
    } else {
        mul_div_down(total_asol, nav_asol_lamports, SOL_PRECISION)?
    };

    amusd_component
        .checked_add(asol_component)
        .ok_or(MathError::Overflow)
}

pub fn build_stability_pool_inventory(
    context: &StabilityPoolQuoteContext,
) -> StabilityPreviewResult<StabilityPoolInventory> {
    let pool_value_lamports = compute_stability_pool_value_sol(
        context.total_amusd,
        context.total_asol,
        context.price_safe_usd,
        context.nav_asol_lamports,
    )?;

    Ok(StabilityPoolInventory {
        total_amusd: context.total_amusd,
        total_asol: context.total_asol,
        total_samusd: context.total_samusd,
        pool_value_lamports,
        nav_asol_lamports: context.nav_asol_lamports,
    })
}

fn updated_vault_context(
    context: &VaultQuoteContext,
    current_lst_amount: u64,
    current_amusd_supply: u64,
    current_asol_supply: u64,
    current_rounding_reserve_lamports: u64,
    lst_to_sol_rate: u64,
) -> VaultQuoteContext {
    let mut next = context.clone();
    next.current_lst_amount = current_lst_amount;
    next.current_amusd_supply = current_amusd_supply;
    next.current_asol_supply = current_asol_supply;
    next.current_rounding_reserve_lamports = current_rounding_reserve_lamports;
    next.lst_to_sol_rate = lst_to_sol_rate;
    next
}

fn updated_stability_context(
    context: &StabilityPoolQuoteContext,
    total_amusd: u64,
    total_asol: u64,
    total_samusd: u64,
    last_harvest_lst_to_sol_rate: u64,
    lst_to_sol_rate: u64,
    nav_asol_lamports: u64,
    current_amusd_supply: u64,
    current_asol_supply: u64,
) -> StabilityPoolQuoteContext {
    let mut next = context.clone();
    next.total_amusd = total_amusd;
    next.total_asol = total_asol;
    next.total_samusd = total_samusd;
    next.last_harvest_lst_to_sol_rate = last_harvest_lst_to_sol_rate;
    next.lst_to_sol_rate = lst_to_sol_rate;
    next.nav_asol_lamports = nav_asol_lamports;
    next.current_amusd_supply = current_amusd_supply;
    next.current_asol_supply = current_asol_supply;
    next
}

pub fn preview_deposit_amusd_to_stability_pool(
    input: &DepositAmusdToStabilityPoolPreviewInput,
) -> StabilityPreviewResult<DepositAmusdToStabilityPoolPreview> {
    let context = &input.context;

    if input.amusd_amount == 0 || input.min_samusd_out == 0 {
        return Err(MathError::ZeroAmount.into());
    }

    if input.amusd_amount < MIN_AMUSD_MINT {
        return Err(MathError::AmountTooSmall("amusd_amount").into());
    }

    let pre_pool_inventory = build_stability_pool_inventory(context)?;
    let deposit_value_lamports =
        mul_div_down(input.amusd_amount, SOL_PRECISION, context.price_safe_usd)?;

    if deposit_value_lamports == 0 {
        return Err(MathError::AmountTooSmall("deposit_value_lamports").into());
    }

    let samusd_out = if context.total_samusd == 0 || pre_pool_inventory.pool_value_lamports == 0 {
        input.amusd_amount
    } else {
        mul_div_down(
            deposit_value_lamports,
            context.total_samusd,
            pre_pool_inventory.pool_value_lamports,
        )?
    };

    if samusd_out == 0 {
        return Err(MathError::AmountTooSmall("samusd_out").into());
    }

    if samusd_out < input.min_samusd_out {
        return Err(MathError::SlippageExceeded.into());
    }

    let new_total_amusd = context
        .total_amusd
        .checked_add(input.amusd_amount)
        .ok_or(MathError::Overflow)?;

    let new_total_samusd = context
        .total_samusd
        .checked_add(samusd_out)
        .ok_or(MathError::Overflow)?;

    let next_stability_pool_context = updated_stability_context(
        context,
        new_total_amusd,
        context.total_asol,
        new_total_samusd,
        context.last_harvest_lst_to_sol_rate,
        context.lst_to_sol_rate,
        context.nav_asol_lamports,
        context.current_amusd_supply,
        context.current_asol_supply,
    );

    let post_pool_inventory = build_stability_pool_inventory(&next_stability_pool_context)?;

    Ok(DepositAmusdToStabilityPoolPreview {
        amusd_in: input.amusd_amount,
        deposit_value_lamports,
        pool_value_before_lamports: pre_pool_inventory.pool_value_lamports,
        samusd_out,
        pre_pool_inventory,
        post_pool_inventory,
        next_stability_pool_context,
    })
}

pub fn preview_withdraw_underlying(
    input: &WithdrawUnderlyingPreviewInput,
) -> StabilityPreviewResult<WithdrawUnderlyingPreview> {
    let context = &input.context;

    if input.samusd_amount == 0 {
        return Err(MathError::ZeroAmount.into());
    }

    if context.stability_withdrawals_paused {
        return Err(StabilityPreviewError::StabilityWithdrawalsPaused);
    }

    if context.total_samusd == 0 {
        return Err(StabilityPreviewError::StabilityPoolEmpty);
    }

    if input.samusd_amount > context.total_samusd {
        return Err(MathError::InsufficientSupply.into());
    }

    let pre_pool_inventory = build_stability_pool_inventory(context)?;

    let amusd_out = mul_div_down(
        context.total_amusd,
        input.samusd_amount,
        context.total_samusd,
    )?;
    let asol_out = mul_div_down(
        context.total_asol,
        input.samusd_amount,
        context.total_samusd,
    )?;

    if amusd_out < input.min_amusd_out || asol_out < input.min_asol_out {
        return Err(MathError::SlippageExceeded.into());
    }

    if amusd_out == 0 && asol_out == 0 {
        return Err(MathError::AmountTooSmall("withdrawal_output").into());
    }

    let new_total_amusd = context
        .total_amusd
        .checked_sub(amusd_out)
        .ok_or(MathError::Overflow)?;
    let new_total_asol = context
        .total_asol
        .checked_sub(asol_out)
        .ok_or(MathError::Overflow)?;
    let new_total_samusd = context
        .total_samusd
        .checked_sub(input.samusd_amount)
        .ok_or(MathError::Overflow)?;

    let next_stability_pool_context = updated_stability_context(
        context,
        new_total_amusd,
        new_total_asol,
        new_total_samusd,
        context.last_harvest_lst_to_sol_rate,
        context.lst_to_sol_rate,
        context.nav_asol_lamports,
        context.current_amusd_supply,
        context.current_asol_supply,
    );

    let post_pool_inventory = build_stability_pool_inventory(&next_stability_pool_context)?;

    Ok(WithdrawUnderlyingPreview {
        samusd_in: input.samusd_amount,
        amusd_out,
        asol_out,
        pre_pool_inventory,
        post_pool_inventory,
        next_stability_pool_context,
    })
}

pub fn preview_harvest_yield(
    input: &HarvestYieldPreviewInput,
) -> StabilityPreviewResult<HarvestYieldPreview> {
    let vault_context = &input.vault_context;
    let context = &input.context;

    let pre_vault_balance_sheet = build_vault_balance_sheet(
        vault_context.current_lst_amount,
        vault_context.current_amusd_supply,
        vault_context.current_asol_supply,
        vault_context.current_rounding_reserve_lamports,
        vault_context.lst_to_sol_rate,
        vault_context.safe_price_usd,
    )?;
    let pre_pool_inventory = build_stability_pool_inventory(context)?;

    let old_rate = context.last_harvest_lst_to_sol_rate;
    let new_rate = input.new_lst_to_sol_rate;

    let mut next_vault_context = updated_vault_context(
        vault_context,
        vault_context.current_lst_amount,
        vault_context.current_amusd_supply,
        vault_context.current_asol_supply,
        vault_context.current_rounding_reserve_lamports,
        new_rate,
    );

    let mut next_stability_pool_context = updated_stability_context(
        context,
        context.total_amusd,
        context.total_asol,
        context.total_samusd,
        new_rate,
        new_rate,
        context.nav_asol_lamports,
        context.current_amusd_supply,
        context.current_asol_supply,
    );

    let mut yield_delta_sol_lamports = 0;
    let mut amusd_minted = 0;
    let mut negative_yield = false;

    if old_rate == 0 {
        next_stability_pool_context.last_harvest_lst_to_sol_rate = new_rate;
    } else if new_rate <= old_rate {
        negative_yield = new_rate < old_rate;
        next_stability_pool_context.last_harvest_lst_to_sol_rate = new_rate;
    } else {
        let rate_delta = new_rate.checked_sub(old_rate).ok_or(MathError::Overflow)?;
        yield_delta_sol_lamports =
            mul_div_down(rate_delta, vault_context.current_lst_amount, SOL_PRECISION)?;

        amusd_minted = mul_div_down(
            yield_delta_sol_lamports,
            context.price_safe_usd,
            SOL_PRECISION,
        )?;

        if amusd_minted > 0 {
            next_stability_pool_context.total_amusd = next_stability_pool_context
                .total_amusd
                .checked_add(amusd_minted)
                .ok_or(MathError::Overflow)?;
            next_vault_context.current_amusd_supply = next_vault_context
                .current_amusd_supply
                .checked_add(amusd_minted)
                .ok_or(MathError::Overflow)?;
            next_stability_pool_context.current_amusd_supply =
                next_vault_context.current_amusd_supply;
        }
    }

    let post_vault_balance_sheet = build_vault_balance_sheet(
        next_vault_context.current_lst_amount,
        next_vault_context.current_amusd_supply,
        next_vault_context.current_asol_supply,
        next_vault_context.current_rounding_reserve_lamports,
        next_vault_context.lst_to_sol_rate,
        next_vault_context.safe_price_usd,
    )?;

    next_stability_pool_context.nav_asol_lamports =
        post_vault_balance_sheet.nav_asol_lamports.unwrap_or(0);

    let post_pool_inventory = build_stability_pool_inventory(&next_stability_pool_context)?;

    Ok(HarvestYieldPreview {
        old_rate,
        new_rate,
        yield_delta_sol_lamports,
        amusd_minted,
        negative_yield,
        pre_vault_balance_sheet,
        post_vault_balance_sheet,
        pre_pool_inventory,
        post_pool_inventory,
        next_vault_context,
        next_stability_pool_context,
    })
}

pub fn preview_execute_debt_equity_swap(
    input: &DebtEquitySwapPreviewInput,
) -> StabilityPreviewResult<DebtEquitySwapPreview> {
    let vault_context = &input.vault_context;
    let context = &input.context;

    let pre_vault_balance_sheet = build_vault_balance_sheet(
        vault_context.current_lst_amount,
        vault_context.current_amusd_supply,
        vault_context.current_asol_supply,
        vault_context.current_rounding_reserve_lamports,
        vault_context.lst_to_sol_rate,
        vault_context.safe_price_usd,
    )?;
    let pre_pool_inventory = build_stability_pool_inventory(context)?;

    let old_cr_raw = compute_cr_bps(
        pre_vault_balance_sheet.tvl_lamports,
        pre_vault_balance_sheet.liability_lamports,
    );

    if old_cr_raw >= vault_context.min_cr_bps {
        return Err(StabilityPreviewError::NoConversionNeeded);
    }

    if context.total_amusd == 0 {
        return Err(StabilityPreviewError::StabilityPoolEmpty);
    }

    if context.nav_floor_lamports == 0 {
        return Err(MathError::InvalidParameter("nav_floor_lamports").into());
    }

    if context.max_asol_mint_per_round == 0 {
        return Err(MathError::InvalidParameter("max_asol_mint_per_round").into());
    }

    let l_sol_max = mul_div_down(
        pre_vault_balance_sheet.tvl_lamports,
        10_000,
        vault_context.min_cr_bps,
    )?;

    let mut amusd_supply_max = mul_div_down(l_sol_max, context.price_safe_usd, SOL_PRECISION)?;

    for _ in 0..4 {
        let liab = compute_liability_sol(amusd_supply_max, context.price_safe_usd)?;
        if liab <= l_sol_max || amusd_supply_max == 0 {
            break;
        }
        amusd_supply_max = amusd_supply_max.saturating_sub(1);
    }

    let burn_needed = vault_context
        .current_amusd_supply
        .saturating_sub(amusd_supply_max);

    if burn_needed == 0 {
        return Err(StabilityPreviewError::NoConversionNeeded);
    }

    let burn_target = burn_needed.min(context.total_amusd);

    let nav_pre_lamports = nav_asol_with_reserve(
        pre_vault_balance_sheet.tvl_lamports,
        pre_vault_balance_sheet.liability_lamports,
        vault_context.current_rounding_reserve_lamports,
        vault_context.current_asol_supply,
    )?
    .unwrap_or(0);

    let nav_conv_lamports = nav_pre_lamports.max(context.nav_floor_lamports);

    let max_sol_by_cap = mul_div_down(
        context.max_asol_mint_per_round,
        nav_conv_lamports,
        SOL_PRECISION,
    )?;
    let max_burn_by_cap = mul_div_down(max_sol_by_cap, context.price_safe_usd, SOL_PRECISION)?;

    let burn_amount_amusd = burn_target.min(max_burn_by_cap);
    if burn_amount_amusd == 0 {
        return Err(StabilityPreviewError::ConversionOutputTooSmall);
    }

    let sol_value = mul_div_down(burn_amount_amusd, SOL_PRECISION, context.price_safe_usd)?;
    let asol_minted = mul_div_down(sol_value, SOL_PRECISION, nav_conv_lamports)?;

    if asol_minted == 0 {
        return Err(StabilityPreviewError::ConversionOutputTooSmall);
    }

    if asol_minted > context.max_asol_mint_per_round {
        return Err(StabilityPreviewError::ConversionCapExceeded);
    }

    let next_vault_context = updated_vault_context(
        vault_context,
        vault_context.current_lst_amount,
        vault_context
            .current_amusd_supply
            .checked_sub(burn_amount_amusd)
            .ok_or(MathError::Overflow)?,
        vault_context
            .current_asol_supply
            .checked_add(asol_minted)
            .ok_or(MathError::Overflow)?,
        vault_context.current_rounding_reserve_lamports,
        vault_context.lst_to_sol_rate,
    );

    let post_vault_balance_sheet = build_vault_balance_sheet(
        next_vault_context.current_lst_amount,
        next_vault_context.current_amusd_supply,
        next_vault_context.current_asol_supply,
        next_vault_context.current_rounding_reserve_lamports,
        next_vault_context.lst_to_sol_rate,
        next_vault_context.safe_price_usd,
    )?;

    let new_cr_raw = compute_cr_bps(
        post_vault_balance_sheet.tvl_lamports,
        post_vault_balance_sheet.liability_lamports,
    );

    if new_cr_raw < old_cr_raw {
        return Err(StabilityPreviewError::NoConversionNeeded);
    }

    let next_stability_pool_context = updated_stability_context(
        context,
        context
            .total_amusd
            .checked_sub(burn_amount_amusd)
            .ok_or(MathError::Overflow)?,
        context
            .total_asol
            .checked_add(asol_minted)
            .ok_or(MathError::Overflow)?,
        context.total_samusd,
        context.last_harvest_lst_to_sol_rate,
        context.lst_to_sol_rate,
        post_vault_balance_sheet.nav_asol_lamports.unwrap_or(0),
        next_vault_context.current_amusd_supply,
        next_vault_context.current_asol_supply,
    );

    let post_pool_inventory = build_stability_pool_inventory(&next_stability_pool_context)?;

    Ok(DebtEquitySwapPreview {
        burn_amount_amusd,
        asol_minted,
        nav_pre_lamports,
        nav_conv_lamports,
        cr_before_bps: normalize_collateral_ratio_bps(old_cr_raw),
        cr_after_bps: normalize_collateral_ratio_bps(new_cr_raw),
        pre_vault_balance_sheet,
        post_vault_balance_sheet,
        pre_pool_inventory,
        post_pool_inventory,
        next_vault_context,
        next_stability_pool_context,
    })
}

pub fn preview_drawdown_sequence(
    input: &DrawdownPreviewInput,
) -> StabilityPreviewResult<DrawdownPreview> {
    let pre_vault_balance_sheet = build_vault_balance_sheet(
        input.vault_context.current_lst_amount,
        input.vault_context.current_amusd_supply,
        input.vault_context.current_asol_supply,
        input.vault_context.current_rounding_reserve_lamports,
        input.vault_context.lst_to_sol_rate,
        input.vault_context.safe_price_usd,
    )?;
    let pre_pool_inventory = build_stability_pool_inventory(&input.context)?;

    let cr_before_bps = pre_vault_balance_sheet.collateral_ratio_bps;

    let mut next_vault_context = input.vault_context.clone();
    let mut next_stability_pool_context = input.context.clone();
    let mut rounds = Vec::new();

    for _ in 0..input.max_rounds {
        let round = match preview_execute_debt_equity_swap(&DebtEquitySwapPreviewInput {
            vault_context: next_vault_context.clone(),
            context: next_stability_pool_context.clone(),
        }) {
            Ok(round) => round,
            Err(StabilityPreviewError::NoConversionNeeded)
            | Err(StabilityPreviewError::StabilityPoolEmpty) => break,
            Err(other) => return Err(other),
        };

        next_vault_context = round.next_vault_context.clone();
        next_stability_pool_context = round.next_stability_pool_context.clone();

        let reached_target = round
            .cr_after_bps
            .map(|cr| cr >= next_vault_context.min_cr_bps)
            .unwrap_or(false);

        rounds.push(round);

        if reached_target || next_stability_pool_context.total_amusd == 0 {
            break;
        }
    }

    let post_vault_balance_sheet = build_vault_balance_sheet(
        next_vault_context.current_lst_amount,
        next_vault_context.current_amusd_supply,
        next_vault_context.current_asol_supply,
        next_vault_context.current_rounding_reserve_lamports,
        next_vault_context.lst_to_sol_rate,
        next_vault_context.safe_price_usd,
    )?;
    let post_pool_inventory = build_stability_pool_inventory(&next_stability_pool_context)?;

    let cr_after_bps = post_vault_balance_sheet.collateral_ratio_bps;
    let reached_target = cr_after_bps
        .map(|cr| cr >= next_vault_context.min_cr_bps)
        .unwrap_or(false);
    let pool_exhausted = next_stability_pool_context.total_amusd == 0;
    let hit_max_rounds = input.max_rounds > 0
        && rounds.len() == input.max_rounds as usize
        && !reached_target
        && !pool_exhausted;

    Ok(DrawdownPreview {
        rounds,
        cr_before_bps,
        cr_after_bps,
        pre_vault_balance_sheet,
        post_vault_balance_sheet,
        pre_pool_inventory,
        post_pool_inventory,
        reached_target,
        pool_exhausted,
        hit_max_rounds,
        next_vault_context,
        next_stability_pool_context,
    })
}
