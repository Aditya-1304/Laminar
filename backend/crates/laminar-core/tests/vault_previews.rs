use laminar_core::{
    preview_mint_amusd, preview_mint_asol, preview_redeem_amusd, preview_redeem_asol,
    MintAmusdPreviewInput, MintAsolPreviewInput, RedeemAmusdPreviewInput, RedeemAsolPreviewInput,
    VaultQuoteContext, SOL_PRECISION, USD_PRECISION,
};

fn worked_example_context() -> VaultQuoteContext {
    VaultQuoteContext {
        current_lst_amount: 1_000 * SOL_PRECISION,
        current_amusd_supply: 50_000 * USD_PRECISION,
        current_asol_supply: 0,
        current_rounding_reserve_lamports: 0,
        max_rounding_reserve_lamports: 10_000,
        lst_to_sol_rate: 1_050_000_000,
        safe_price_usd: 99_000_000,
        redeem_price_usd: 100_000_000,
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

fn reserve_backed_redeem_context() -> VaultQuoteContext {
    let mut ctx = worked_example_context();
    ctx.current_rounding_reserve_lamports = 10;
    ctx.fee_amusd_redeem_bps = 0;
    ctx
}

fn contract_exact_asol_context() -> VaultQuoteContext {
    let mut ctx = worked_example_context();
    ctx.current_asol_supply = 544_949_494_949;
    ctx.fee_asol_mint_bps = 0;
    ctx.fee_asol_redeem_bps = 0;
    ctx
}

fn reserve_backed_asol_redeem_context() -> VaultQuoteContext {
    let mut ctx = contract_exact_asol_context();
    ctx.current_rounding_reserve_lamports = 10;
    ctx
}

fn haircut_context() -> VaultQuoteContext {
    VaultQuoteContext {
        current_lst_amount: 90 * SOL_PRECISION,
        current_amusd_supply: 1_000 * USD_PRECISION,
        current_asol_supply: 1 * SOL_PRECISION,
        current_rounding_reserve_lamports: 0,
        max_rounding_reserve_lamports: 10_000,
        lst_to_sol_rate: SOL_PRECISION,
        safe_price_usd: 10_000_000,
        redeem_price_usd: 10_000_000,
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

fn drawdown_flag_context() -> VaultQuoteContext {
    VaultQuoteContext {
        current_lst_amount: 120 * SOL_PRECISION,
        current_amusd_supply: 1_000 * USD_PRECISION,
        current_asol_supply: 1 * SOL_PRECISION,
        current_rounding_reserve_lamports: 10,
        max_rounding_reserve_lamports: 10_000,
        lst_to_sol_rate: SOL_PRECISION,
        safe_price_usd: 10_000_000,
        redeem_price_usd: 10_000_000,
        min_cr_bps: 13_000,
        target_cr_bps: 15_000,
        uncertainty_index_bps: 0,
        uncertainty_max_bps: 20_000,
        fee_amusd_mint_bps: 50,
        fee_amusd_redeem_bps: 0,
        fee_asol_mint_bps: 30,
        fee_asol_redeem_bps: 15,
        fee_min_multiplier_bps: 10_000,
        fee_max_multiplier_bps: 40_000,
        mint_paused: false,
        redeem_paused: false,
    }
}

#[test]
fn mint_amusd_preview_matches_worked_example() {
    let preview = preview_mint_amusd(&MintAmusdPreviewInput {
        context: worked_example_context(),
        lst_amount: 10 * SOL_PRECISION,
        min_amusd_out: 1_034_302_500,
    })
    .unwrap();

    assert_eq!(preview.lst_in, 10 * SOL_PRECISION);
    assert_eq!(preview.sol_value_lamports, 10_500_000_000);
    assert_eq!(preview.gross_amusd_out, 1_039_500_000);
    assert_eq!(preview.fee_amusd, 5_197_500);
    assert_eq!(preview.net_amusd_out, 1_034_302_500);
    assert_eq!(preview.reserve_credit_lamports, 0);
    assert_eq!(preview.rounding_bound_lamports, 13);

    assert_eq!(
        preview.post_mint_balance_sheet.lst_amount,
        1_010 * SOL_PRECISION
    );
    assert_eq!(preview.post_mint_balance_sheet.amusd_supply, 51_039_500_000);
    assert_eq!(preview.post_mint_balance_sheet.rounding_reserve_lamports, 0);

    assert!(
        preview
            .post_mint_balance_sheet
            .collateral_ratio_bps
            .unwrap()
            >= 13_000
    );
}

#[test]
fn redeem_amusd_preview_matches_solvent_rounding_path() {
    let preview = preview_redeem_amusd(&RedeemAmusdPreviewInput {
        context: reserve_backed_redeem_context(),
        amusd_amount: 1_000 * USD_PRECISION,
        min_lst_out: 100_000,
        stability_pool_amusd_available: 0,
        effective_lst_amount: None,
        effective_amusd_supply: None,
        effective_asol_supply: None,
        effective_rounding_reserve_lamports: None,
        drawdown_rounds_executed: 0,
    })
    .unwrap();

    assert_eq!(preview.amusd_in, 1_000 * USD_PRECISION);
    assert_eq!(preview.amusd_net_burn, 1_000 * USD_PRECISION);
    assert_eq!(preview.amusd_fee_in, 0);
    assert_eq!(preview.sol_value_gross_lamports, 10_000_000_000);
    assert_eq!(preview.lst_out, 9_523_809_524);
    assert_eq!(preview.reserve_debit_lamports, 2);
    assert_eq!(preview.rounding_bound_lamports, 13);
    assert_eq!(preview.haircut_bps, None);
    assert!(preview.solvent_mode);
    assert!(preview.used_user_favoring_rounding);
    assert!(!preview.drawdown_expected);

    assert_eq!(
        preview.post_redemption_balance_sheet.lst_amount,
        990_476_190_476
    );
    assert_eq!(
        preview.post_redemption_balance_sheet.amusd_supply,
        49_000_000_000
    );
    assert_eq!(
        preview
            .post_redemption_balance_sheet
            .rounding_reserve_lamports,
        8
    );
}

#[test]
fn mint_asol_preview_matches_contract_consistent_vector() {
    let preview = preview_mint_asol(&MintAsolPreviewInput {
        context: contract_exact_asol_context(),
        lst_amount: 10 * SOL_PRECISION,
        min_asol_out: 10_500_000_000,
    })
    .unwrap();

    assert_eq!(preview.lst_in, 10 * SOL_PRECISION);
    assert_eq!(preview.sol_value_lamports, 10_500_000_000);
    assert_eq!(preview.nav_before_lamports, 1_000_000_000);
    assert_eq!(preview.gross_asol_out, 10_500_000_000);
    assert_eq!(preview.net_asol_out, 10_500_000_000);
    assert_eq!(preview.fee_asol, 0);
    assert_eq!(preview.reserve_credit_lamports, 0);
    assert_eq!(preview.rounding_bound_lamports, 2);
    assert!(!preview.bootstrap_mode);
    assert_eq!(preview.orphan_equity_swept_lamports, 0);

    assert_eq!(preview.post_mint_balance_sheet.asol_supply, 555_449_494_949);
}

#[test]
fn redeem_asol_preview_matches_solvent_rounding_path() {
    let preview = preview_redeem_asol(&RedeemAsolPreviewInput {
        context: reserve_backed_asol_redeem_context(),
        asol_amount: 1 * SOL_PRECISION,
        min_lst_out: 100_000,
    })
    .unwrap();

    assert_eq!(preview.asol_in, 1 * SOL_PRECISION);
    assert_eq!(preview.asol_net_burn, 1 * SOL_PRECISION);
    assert_eq!(preview.asol_fee_in, 0);

    assert_eq!(preview.nav_before_lamports, 999_999_999);
    assert_eq!(preview.sol_value_gross_lamports, 999_999_999);
    assert_eq!(preview.lst_out, 952_380_952);

    assert_eq!(preview.reserve_debit_lamports, 2);
    assert_eq!(preview.rounding_bound_lamports, 2);
    assert!(preview.solvent_mode);
    assert!(preview.used_user_favoring_rounding);

    assert_eq!(
        preview.post_redemption_balance_sheet.lst_amount,
        999_047_619_048
    );
    assert_eq!(
        preview.post_redemption_balance_sheet.asol_supply,
        543_949_494_949
    );
    assert_eq!(
        preview
            .post_redemption_balance_sheet
            .rounding_reserve_lamports,
        8
    );
}

#[test]
fn redeem_amusd_preview_uses_haircut_mode_and_zero_fee_below_100_cr() {
    let preview = preview_redeem_amusd(&RedeemAmusdPreviewInput {
        context: haircut_context(),
        amusd_amount: 100 * USD_PRECISION,
        min_lst_out: 100_000,
        stability_pool_amusd_available: 0,
        effective_lst_amount: None,
        effective_amusd_supply: None,
        effective_asol_supply: None,
        effective_rounding_reserve_lamports: None,
        drawdown_rounds_executed: 0,
    })
    .unwrap();

    assert_eq!(preview.amusd_fee_in, 0);
    assert_eq!(preview.haircut_bps, Some(9_000));
    assert_eq!(preview.sol_value_gross_lamports, 9_000_000_000);
    assert_eq!(preview.lst_out, 9_000_000_000);
    assert_eq!(preview.reserve_debit_lamports, 0);
    assert!(!preview.solvent_mode);
    assert!(!preview.used_user_favoring_rounding);

    assert_eq!(
        preview.post_redemption_balance_sheet.collateral_ratio_bps,
        Some(9_000)
    );
}

#[test]
fn redeem_amusd_preview_flags_drawdown_expected_and_uses_effective_snapshot() {
    let preview = preview_redeem_amusd(&RedeemAmusdPreviewInput {
        context: drawdown_flag_context(),
        amusd_amount: 100 * USD_PRECISION,
        min_lst_out: 100_000,
        stability_pool_amusd_available: 50 * USD_PRECISION,
        effective_lst_amount: Some(120 * SOL_PRECISION),
        effective_amusd_supply: Some(900 * USD_PRECISION),
        effective_asol_supply: Some(1 * SOL_PRECISION),
        effective_rounding_reserve_lamports: Some(10),
        drawdown_rounds_executed: 1,
    })
    .unwrap();

    assert!(preview.drawdown_expected);
    assert_eq!(preview.drawdown_rounds_executed, 1);
    assert_eq!(
        preview.pre_redemption_balance_sheet.amusd_supply,
        900 * USD_PRECISION
    );
    assert_eq!(
        preview.post_redemption_balance_sheet.amusd_supply,
        800 * USD_PRECISION
    );
    assert_eq!(preview.lst_out, 10 * SOL_PRECISION);
}
