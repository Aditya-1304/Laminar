use laminar_core::{
    apply_fee, asol_dust_to_lamports_up, balance_sheet_holds, compute_accounting_equity_sol,
    compute_claimable_equity_sol, compute_cr_bps, compute_dynamic_fee_bps, compute_liability_sol,
    compute_rounding_delta_units, compute_tvl_sol, credit_rounding_reserve, debit_rounding_reserve,
    derive_rounding_bound_lamports, lst_dust_to_lamports_up, mul_div_down, mul_div_up,
    nav_asol_with_reserve, usd_dust_to_lamports_up, FeeAction, MathError, BPS_PRECISION,
    SOL_PRECISION, USD_PRECISION,
};

#[test]
fn parity_vector_63_5_1_mint_amusd_matches_contract_numbers() {
    let total_lst_amount = 1_000 * SOL_PRECISION;
    let lst_to_sol_rate = 1_050_000_000u64;
    let amusd_supply = 50_000 * USD_PRECISION;
    let rounding_reserve = 0u64;

    let p_safe = 99_000_000u64;
    let q_lst = 10 * SOL_PRECISION;
    let fee_base_bps = 50u64;

    let sol_value = mul_div_down(q_lst, lst_to_sol_rate, SOL_PRECISION).unwrap();
    assert_eq!(sol_value, 10_500_000_000);

    let amusd_gross = mul_div_down(sol_value, p_safe, SOL_PRECISION).unwrap();
    assert_eq!(amusd_gross, 1_039_500_000);

    let (amusd_net, fee) = apply_fee(amusd_gross, fee_base_bps).unwrap();
    assert_eq!(fee, 5_197_500);
    assert_eq!(amusd_net, 1_034_302_500);

    let sol_value_up = mul_div_up(q_lst, lst_to_sol_rate, SOL_PRECISION).unwrap();
    let amusd_gross_up = mul_div_up(sol_value_up, p_safe, SOL_PRECISION).unwrap();
    let mint_rounding_delta_usd =
        compute_rounding_delta_units(amusd_gross, amusd_gross_up).unwrap();
    let reserve_credit = usd_dust_to_lamports_up(mint_rounding_delta_usd, p_safe).unwrap();
    assert_eq!(reserve_credit, 0);

    let new_lst = total_lst_amount + q_lst;
    let new_amusd_supply = amusd_supply + amusd_gross;
    let new_tvl = compute_tvl_sol(new_lst, lst_to_sol_rate).unwrap();
    let new_liability = compute_liability_sol(new_amusd_supply, p_safe).unwrap();
    let new_reserve =
        credit_rounding_reserve(rounding_reserve, reserve_credit, 1_000_000_000).unwrap();
    let accounting_equity =
        compute_accounting_equity_sol(new_tvl, new_liability, new_reserve).unwrap();
    let bound = derive_rounding_bound_lamports(2, 1, p_safe).unwrap();

    assert_eq!(bound, 13);
    assert!(compute_cr_bps(new_tvl, new_liability) >= 13_000);
    assert!(balance_sheet_holds(
        new_tvl,
        new_liability,
        accounting_equity,
        new_reserve,
        bound,
    )
    .unwrap());
}

#[test]
fn parity_vector_63_5_2_redeem_amusd_matches_contract_numbers() {
    let p_redeem = 100_000_000u64;
    let lst_to_sol_rate = 1_050_000_000u64;
    let amusd_in = 1_000 * USD_PRECISION;

    let sol_out = mul_div_up(amusd_in, SOL_PRECISION, p_redeem).unwrap();
    assert_eq!(sol_out, 10_000_000_000);

    let lst_out = mul_div_up(sol_out, SOL_PRECISION, lst_to_sol_rate).unwrap();
    assert_eq!(lst_out, 9_523_809_524);
}

#[test]
fn parity_vector_63_5_3_mint_asol_matches_contract_numbers() {
    let total_lst_amount = 1_000 * SOL_PRECISION;
    let lst_to_sol_rate = 1_050_000_000u64;
    let amusd_supply = 50_000 * USD_PRECISION;
    let p_safe = 99_000_000u64;
    let rounding_reserve = 0u64;

    let tvl_pre = compute_tvl_sol(total_lst_amount, lst_to_sol_rate).unwrap();
    assert_eq!(tvl_pre, 1_050_000_000_000);

    let liability_pre = compute_liability_sol(amusd_supply, p_safe).unwrap();
    assert_eq!(liability_pre, 505_050_505_051);

    let equity_pre =
        compute_claimable_equity_sol(tvl_pre, liability_pre, rounding_reserve).unwrap();
    assert_eq!(equity_pre, 544_949_494_949);

    let asol_supply = equity_pre;
    let nav_pre = nav_asol_with_reserve(tvl_pre, liability_pre, rounding_reserve, asol_supply)
        .unwrap()
        .unwrap();
    assert_eq!(nav_pre, SOL_PRECISION);

    let q_lst = 10 * SOL_PRECISION;
    let sol_value = mul_div_down(q_lst, lst_to_sol_rate, SOL_PRECISION).unwrap();
    assert_eq!(sol_value, 10_500_000_000);

    let asol_minted = mul_div_down(sol_value, SOL_PRECISION, nav_pre).unwrap();
    assert_eq!(asol_minted, 10_500_000_000);
}

#[test]
fn parity_vector_63_5_4_redeem_asol_matches_contract_numbers() {
    let asol_in = SOL_PRECISION;
    let nav_pre = SOL_PRECISION;
    let lst_to_sol_rate = 1_050_000_000u64;

    let sol_out = mul_div_down(asol_in, nav_pre, SOL_PRECISION).unwrap();
    assert_eq!(sol_out, 1_000_000_000);

    let lst_out = mul_div_up(sol_out, SOL_PRECISION, lst_to_sol_rate).unwrap();
    assert_eq!(lst_out, 952_380_953);
}

#[test]
fn parity_rounding_reserve_math_matches_contract() {
    let lst_delta_units = 1u64;
    let lst_to_sol_rate = 1_050_000_000u64;
    let lamport_delta = lst_dust_to_lamports_up(lst_delta_units, lst_to_sol_rate).unwrap();
    assert_eq!(lamport_delta, 2);

    let credited = credit_rounding_reserve(100, lamport_delta, 1_000).unwrap();
    let debited = debit_rounding_reserve(credited, lamport_delta).unwrap();
    assert_eq!(debited, 100);

    let asol_delta_units = 1u64;
    let nav = SOL_PRECISION;
    let asol_lamport_delta = asol_dust_to_lamports_up(asol_delta_units, nav).unwrap();
    assert_eq!(asol_lamport_delta, 1);
}

#[test]
fn parity_fee_curve_interpolation_and_clamps_hold() {
    let base = 100u64;
    let min_cr = 13_000u64;
    let target_cr = 15_000u64;
    let min_mult = 5_000u64;
    let max_mult = 20_000u64;

    assert_eq!(
        compute_dynamic_fee_bps(
            base,
            FeeAction::AmusdMint,
            14_000,
            min_cr,
            target_cr,
            min_mult,
            max_mult,
            0,
            20_000,
        )
        .unwrap(),
        150
    );

    assert_eq!(
        compute_dynamic_fee_bps(
            base,
            FeeAction::AmusdRedeem,
            14_000,
            min_cr,
            target_cr,
            min_mult,
            max_mult,
            0,
            20_000,
        )
        .unwrap(),
        75
    );

    let risk_inc = compute_dynamic_fee_bps(
        base,
        FeeAction::AsolRedeem,
        u64::MAX,
        min_cr,
        target_cr,
        min_mult,
        max_mult,
        10_000,
        12_000,
    )
    .unwrap();

    let risk_red = compute_dynamic_fee_bps(
        base,
        FeeAction::AsolMint,
        u64::MAX,
        min_cr,
        target_cr,
        min_mult,
        max_mult,
        10_000,
        12_000,
    )
    .unwrap();

    assert!(risk_inc >= base);
    assert!(risk_red <= base);

    assert!(matches!(
        compute_dynamic_fee_bps(
            base,
            FeeAction::AmusdMint,
            14_000,
            min_cr,
            target_cr,
            12_000,
            9_000,
            0,
            20_000,
        ),
        Err(MathError::InvalidParameter(_))
    ));

    assert_eq!(BPS_PRECISION, 10_000);
}
