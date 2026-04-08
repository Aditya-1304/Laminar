use laminar_core::{
    preview_drawdown_sequence, preview_execute_debt_equity_swap, preview_harvest_yield,
    preview_withdraw_underlying, DebtEquitySwapPreviewInput, DrawdownPreviewInput,
    HarvestYieldPreviewInput, StabilityPoolQuoteContext, StabilityPreviewError, VaultQuoteContext,
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
fn debt_equity_swap_respects_nav_floor_when_floor_is_higher_than_nav_pre() {
    let mut pool = drawdown_pool_context();
    pool.nav_floor_lamports = 2 * SOL_PRECISION;

    let preview = preview_execute_debt_equity_swap(&DebtEquitySwapPreviewInput {
        vault_context: drawdown_vault_context(),
        context: pool,
    })
    .unwrap();

    assert_eq!(preview.nav_pre_lamports, SOL_PRECISION);
    assert_eq!(preview.nav_conv_lamports, 2 * SOL_PRECISION);
    assert_eq!(preview.burn_amount_amusd, 76_923_077);
    assert_eq!(preview.asol_minted, 3_846_153_850);
    assert_eq!(preview.cr_after_bps, Some(13_000));
    assert_eq!(preview.post_pool_inventory.total_asol, 3_846_153_850);
}

#[test]
fn drawdown_sequence_hits_max_rounds_when_conversion_cap_is_tight() {
    let mut pool = drawdown_pool_context();
    pool.total_amusd = 500 * USD_PRECISION;
    pool.total_samusd = 500 * USD_PRECISION;
    pool.max_asol_mint_per_round = SOL_PRECISION;

    let preview = preview_drawdown_sequence(&DrawdownPreviewInput {
        vault_context: drawdown_vault_context(),
        context: pool,
        max_rounds: 2,
    })
    .unwrap();

    assert_eq!(preview.rounds.len(), 2);
    assert_eq!(preview.rounds[0].burn_amount_amusd, 10 * USD_PRECISION);
    assert_eq!(preview.rounds[1].burn_amount_amusd, 10 * USD_PRECISION);
    assert_eq!(preview.rounds[0].asol_minted, SOL_PRECISION);
    assert_eq!(preview.rounds[1].asol_minted, SOL_PRECISION);
    assert_eq!(preview.cr_before_bps, Some(12_000));
    assert_eq!(preview.cr_after_bps, Some(12_244));
    assert!(!preview.reached_target);
    assert!(!preview.pool_exhausted);
    assert!(preview.hit_max_rounds);
}

#[test]
fn drawdown_sequence_marks_pool_exhausted_when_pool_cannot_restore_cr() {
    let mut pool = drawdown_pool_context();
    pool.total_amusd = 10 * USD_PRECISION;
    pool.total_samusd = 10 * USD_PRECISION;

    let preview = preview_drawdown_sequence(&DrawdownPreviewInput {
        vault_context: drawdown_vault_context(),
        context: pool,
        max_rounds: 8,
    })
    .unwrap();

    assert_eq!(preview.rounds.len(), 1);
    assert_eq!(preview.rounds[0].burn_amount_amusd, 10 * USD_PRECISION);
    assert_eq!(preview.rounds[0].asol_minted, SOL_PRECISION);
    assert_eq!(preview.cr_before_bps, Some(12_000));
    assert_eq!(preview.cr_after_bps, Some(12_121));
    assert!(!preview.reached_target);
    assert!(preview.pool_exhausted);
    assert!(!preview.hit_max_rounds);
}

#[test]
fn harvest_yield_negative_rate_does_not_mint_amusd() {
    let mut context = populated_pool_context();
    context.last_harvest_lst_to_sol_rate = 1_050_000_000;

    let preview = preview_harvest_yield(&HarvestYieldPreviewInput {
        vault_context: harvest_vault_context(),
        context: context.clone(),
        new_lst_to_sol_rate: SOL_PRECISION,
    })
    .unwrap();

    assert!(preview.negative_yield);
    assert_eq!(preview.yield_delta_sol_lamports, 0);
    assert_eq!(preview.amusd_minted, 0);
    assert_eq!(
        preview.next_stability_pool_context.total_amusd,
        context.total_amusd
    );
    assert_eq!(
        preview
            .next_stability_pool_context
            .last_harvest_lst_to_sol_rate,
        SOL_PRECISION
    );
}

#[test]
fn withdraw_underlying_preview_errors_when_withdrawals_are_paused() {
    let mut context = populated_pool_context();
    context.stability_withdrawals_paused = true;

    let result = preview_withdraw_underlying(&WithdrawUnderlyingPreviewInput {
        context,
        samusd_amount: 250 * USD_PRECISION,
        min_amusd_out: 1,
        min_asol_out: 1,
    });

    assert!(matches!(
        result,
        Err(StabilityPreviewError::StabilityWithdrawalsPaused)
    ));
}
