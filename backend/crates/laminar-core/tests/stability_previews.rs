use laminar_core::{
    preview_deposit_amusd_to_stability_pool, preview_drawdown_sequence,
    preview_execute_debt_equity_swap, preview_harvest_yield, preview_withdraw_underlying,
    DebtEquitySwapPreviewInput, DepositAmusdToStabilityPoolPreviewInput, DrawdownPreviewInput,
    HarvestYieldPreviewInput, StabilityPoolQuoteContext, VaultQuoteContext,
    WithdrawUnderlyingPreviewInput, SOL_PRECISION, USD_PRECISION,
};

fn harvest_vault_context() -> VaultQuoteContext {
    VaultQuoteContext {
        current_lst_amount: 100 * SOL_PRECISION,
        current_amusd_supply: 1_000 * USD_PRECISION,
        current_asol_supply: 10 * SOL_PRECISION,
        current_rounding_reserve_lamports: 0,
        max_rounding_reserve_lamports: 10_000,
        lst_to_sol_rate: SOL_PRECISION,
        safe_price_usd: 100 * USD_PRECISION,
        redeem_price_usd: 100 * USD_PRECISION,
        min_cr_bps: 13_000,
        target_cr_bps: 15_000,
        uncertainty_index_bps: 0,
        uncertainty_max_bps: 20_000,
        fee_amusd_mint_bps: 50,
        fee_amusd_redeem_bps: 25,
        fee_asol_mint_bps: 30,
        fee_asol_redeem_bps: 15,
        fee_min_multiplier_bps: 10_000,
        fee_max_multiplier_bps: 40_000,
        mint_paused: false,
        redeem_paused: false,
    }
}

fn empty_pool_context() -> StabilityPoolQuoteContext {
    StabilityPoolQuoteContext {
        total_amusd: 0,
        total_asol: 0,
        total_samusd: 0,
        stability_withdrawals_paused: false,
        last_harvest_lst_to_sol_rate: SOL_PRECISION,
        price_safe_usd: 100 * USD_PRECISION,
        lst_to_sol_rate: SOL_PRECISION,
        nav_asol_lamports: 0,
        current_lst_amount: 100 * SOL_PRECISION,
        current_amusd_supply: 1_000 * USD_PRECISION,
        current_asol_supply: 10 * SOL_PRECISION,
        current_rounding_reserve_lamports: 0,
        min_cr_bps: 13_000,
        nav_floor_lamports: 1_000_000,
        max_asol_mint_per_round: 50_000 * SOL_PRECISION,
    }
}

fn populated_pool_context() -> StabilityPoolQuoteContext {
    StabilityPoolQuoteContext {
        total_amusd: 500 * USD_PRECISION,
        total_asol: 5 * SOL_PRECISION,
        total_samusd: 1_000 * USD_PRECISION,
        stability_withdrawals_paused: false,
        last_harvest_lst_to_sol_rate: SOL_PRECISION,
        price_safe_usd: 100 * USD_PRECISION,
        lst_to_sol_rate: SOL_PRECISION,
        nav_asol_lamports: SOL_PRECISION,
        current_lst_amount: 100 * SOL_PRECISION,
        current_amusd_supply: 1_000 * USD_PRECISION,
        current_asol_supply: 10 * SOL_PRECISION,
        current_rounding_reserve_lamports: 0,
        min_cr_bps: 13_000,
        nav_floor_lamports: 1_000_000,
        max_asol_mint_per_round: 50_000 * SOL_PRECISION,
    }
}

fn drawdown_vault_context() -> VaultQuoteContext {
    VaultQuoteContext {
        current_lst_amount: 120 * SOL_PRECISION,
        current_amusd_supply: 1_000 * USD_PRECISION,
        current_asol_supply: 20 * SOL_PRECISION,
        current_rounding_reserve_lamports: 0,
        max_rounding_reserve_lamports: 10_000,
        lst_to_sol_rate: SOL_PRECISION,
        safe_price_usd: 10 * USD_PRECISION,
        redeem_price_usd: 10 * USD_PRECISION,
        min_cr_bps: 13_000,
        target_cr_bps: 15_000,
        uncertainty_index_bps: 0,
        uncertainty_max_bps: 20_000,
        fee_amusd_mint_bps: 50,
        fee_amusd_redeem_bps: 25,
        fee_asol_mint_bps: 30,
        fee_asol_redeem_bps: 15,
        fee_min_multiplier_bps: 10_000,
        fee_max_multiplier_bps: 40_000,
        mint_paused: false,
        redeem_paused: false,
    }
}

fn drawdown_pool_context() -> StabilityPoolQuoteContext {
    StabilityPoolQuoteContext {
        total_amusd: 100 * USD_PRECISION,
        total_asol: 0,
        total_samusd: 100 * USD_PRECISION,
        stability_withdrawals_paused: false,
        last_harvest_lst_to_sol_rate: SOL_PRECISION,
        price_safe_usd: 10 * USD_PRECISION,
        lst_to_sol_rate: SOL_PRECISION,
        nav_asol_lamports: SOL_PRECISION,
        current_lst_amount: 120 * SOL_PRECISION,
        current_amusd_supply: 1_000 * USD_PRECISION,
        current_asol_supply: 20 * SOL_PRECISION,
        current_rounding_reserve_lamports: 0,
        min_cr_bps: 13_000,
        nav_floor_lamports: 1_000_000,
        max_asol_mint_per_round: 50_000 * SOL_PRECISION,
    }
}

#[test]
fn deposit_amusd_to_stability_pool_preview_matches_empty_pool_bootstrap() {
    let preview =
        preview_deposit_amusd_to_stability_pool(&DepositAmusdToStabilityPoolPreviewInput {
            context: empty_pool_context(),
            amusd_amount: 100 * USD_PRECISION,
            min_samusd_out: 100 * USD_PRECISION,
        })
        .unwrap();

    assert_eq!(preview.deposit_value_lamports, SOL_PRECISION);
    assert_eq!(preview.samusd_out, 100 * USD_PRECISION);
    assert_eq!(preview.pool_value_before_lamports, 0);
    assert_eq!(preview.post_pool_inventory.total_amusd, 100 * USD_PRECISION);
    assert_eq!(
        preview.post_pool_inventory.total_samusd,
        100 * USD_PRECISION
    );
}

#[test]
fn withdraw_underlying_preview_matches_pro_rata_split() {
    let preview = preview_withdraw_underlying(&WithdrawUnderlyingPreviewInput {
        context: populated_pool_context(),
        samusd_amount: 250 * USD_PRECISION,
        min_amusd_out: 1,
        min_asol_out: 1,
    })
    .unwrap();

    assert_eq!(preview.amusd_out, 125 * USD_PRECISION);
    assert_eq!(preview.asol_out, 1_250_000_000);
    assert_eq!(preview.post_pool_inventory.total_amusd, 375 * USD_PRECISION);
    assert_eq!(preview.post_pool_inventory.total_asol, 3_750_000_000);
    assert_eq!(
        preview.post_pool_inventory.total_samusd,
        750 * USD_PRECISION
    );
}

#[test]
fn harvest_yield_preview_mints_amusd_into_pool_on_positive_rate_delta() {
    let preview = preview_harvest_yield(&HarvestYieldPreviewInput {
        vault_context: harvest_vault_context(),
        context: empty_pool_context(),
        new_lst_to_sol_rate: 1_050_000_000,
    })
    .unwrap();

    assert_eq!(preview.old_rate, SOL_PRECISION);
    assert_eq!(preview.new_rate, 1_050_000_000);
    assert_eq!(preview.yield_delta_sol_lamports, 5 * SOL_PRECISION);
    assert_eq!(preview.amusd_minted, 500 * USD_PRECISION);
    assert!(!preview.negative_yield);
    assert_eq!(
        preview.post_vault_balance_sheet.amusd_supply,
        1_500 * USD_PRECISION
    );
    assert_eq!(preview.post_pool_inventory.total_amusd, 500 * USD_PRECISION);
}

#[test]
fn debt_equity_swap_preview_improves_cr_and_updates_pool() {
    let preview = preview_execute_debt_equity_swap(&DebtEquitySwapPreviewInput {
        vault_context: drawdown_vault_context(),
        context: drawdown_pool_context(),
    })
    .unwrap();

    assert_eq!(preview.burn_amount_amusd, 76_923_077);
    assert_eq!(preview.asol_minted, 7_692_307_700);
    assert_eq!(preview.nav_pre_lamports, SOL_PRECISION);
    assert_eq!(preview.nav_conv_lamports, SOL_PRECISION);
    assert_eq!(preview.cr_before_bps, Some(12_000));
    assert_eq!(preview.cr_after_bps, Some(13_000));

    assert_eq!(preview.post_vault_balance_sheet.amusd_supply, 923_076_923);
    assert_eq!(preview.post_pool_inventory.total_amusd, 23_076_923);
    assert_eq!(preview.post_pool_inventory.total_asol, 7_692_307_700);
}

#[test]
fn drawdown_preview_runs_until_target_is_reached() {
    let preview = preview_drawdown_sequence(&DrawdownPreviewInput {
        vault_context: drawdown_vault_context(),
        context: drawdown_pool_context(),
        max_rounds: 8,
    })
    .unwrap();

    assert_eq!(preview.rounds.len(), 1);
    assert_eq!(preview.cr_before_bps, Some(12_000));
    assert_eq!(preview.cr_after_bps, Some(13_000));
    assert!(preview.reached_target);
    assert!(!preview.pool_exhausted);
    assert!(!preview.hit_max_rounds);
}
