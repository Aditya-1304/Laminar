use laminar::constants::MIN_PROTOCOL_TVL;
use laminar::invariants::{
    assert_balance_sheet_holds, assert_cr_above_minimum, assert_rounding_reserve_within_cap,
    credit_rounding_reserve, debit_rounding_reserve, derive_rounding_bound_lamports,
};
use laminar::math::{
    apply_fee, asol_dust_to_lamports_up, compute_accounting_equity_sol, compute_claimable_equity_sol,
    compute_cr_bps, compute_dynamic_fee_bps, compute_liability_sol, compute_rounding_delta_units,
    compute_tvl_sol, lst_dust_to_lamports_up, mul_div_down, mul_div_up, nav_asol_with_reserve,
    usd_dust_to_lamports_up, FeeAction, BPS_PRECISION, MIN_AMUSD_MINT, MIN_ASOL_MINT,
    MIN_LST_DEPOSIT, SOL_PRECISION, USD_PRECISION,
};

#[test]
fn vector_63_5_1_mint_amusd_matches_spec_numbers() {
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
    let mint_rounding_delta_usd = compute_rounding_delta_units(amusd_gross, amusd_gross_up).unwrap();
    let reserve_credit = usd_dust_to_lamports_up(mint_rounding_delta_usd, p_safe).unwrap();

    let new_lst = total_lst_amount + q_lst;
    let new_amusd_supply = amusd_supply + amusd_gross;
    let new_tvl = compute_tvl_sol(new_lst, lst_to_sol_rate).unwrap();
    let new_liability = compute_liability_sol(new_amusd_supply, p_safe).unwrap();
    let new_reserve = credit_rounding_reserve(rounding_reserve, reserve_credit, 1_000_000_000).unwrap();
    let accounting_equity = compute_accounting_equity_sol(new_tvl, new_liability, new_reserve).unwrap();
    let bound = derive_rounding_bound_lamports(2, 1, p_safe).unwrap();

    assert_cr_above_minimum(compute_cr_bps(new_tvl, new_liability), 13_000).unwrap();
    assert_balance_sheet_holds(new_tvl, new_liability, accounting_equity, new_reserve, bound).unwrap();
}

#[test]
fn vector_63_5_2_redeem_amusd_matches_spec_numbers() {
    let p_redeem = 100_000_000u64;
    let lst_to_sol_rate = 1_050_000_000u64;
    let amusd_in = 1_000 * USD_PRECISION;

    let sol_out = mul_div_up(amusd_in, SOL_PRECISION, p_redeem).unwrap();
    assert_eq!(sol_out, 10_000_000_000);

    let lst_out = mul_div_up(sol_out, SOL_PRECISION, lst_to_sol_rate).unwrap();
    assert_eq!(lst_out, 9_523_809_524);
}

#[test]
fn vector_63_5_3_mint_asol_matches_conservative_rounding() {
    let total_lst_amount = 1_000 * SOL_PRECISION;
    let lst_to_sol_rate = 1_050_000_000u64;
    let amusd_supply = 50_000 * USD_PRECISION;
    let p_safe = 99_000_000u64;
    let rounding_reserve = 0u64;

    let tvl_pre = compute_tvl_sol(total_lst_amount, lst_to_sol_rate).unwrap();
    assert_eq!(tvl_pre, 1_050_000_000_000);

    // Conservative liability rounding (A1/A4 behavior).
    let liability_pre = compute_liability_sol(amusd_supply, p_safe).unwrap();
    assert_eq!(liability_pre, 505_050_505_051);

    let equity_pre = compute_claimable_equity_sol(tvl_pre, liability_pre, rounding_reserve).unwrap();
    assert_eq!(equity_pre, 544_949_494_949);

    let asol_supply = equity_pre;
    let nav_pre = nav_asol_with_reserve(tvl_pre, liability_pre, rounding_reserve, asol_supply).unwrap();
    assert_eq!(nav_pre, SOL_PRECISION);

    let q_lst = 10 * SOL_PRECISION;
    let sol_value = mul_div_down(q_lst, lst_to_sol_rate, SOL_PRECISION).unwrap();
    assert_eq!(sol_value, 10_500_000_000);

    let asol_minted = mul_div_down(sol_value, SOL_PRECISION, nav_pre).unwrap();
    assert_eq!(asol_minted, 10_500_000_000);
}

#[test]
fn vector_63_5_4_redeem_asol_matches_spec_numbers() {
    let asol_in = SOL_PRECISION;
    let nav_pre = SOL_PRECISION;
    let lst_to_sol_rate = 1_050_000_000u64;

    let sol_out = mul_div_down(asol_in, nav_pre, SOL_PRECISION).unwrap();
    assert_eq!(sol_out, 1_000_000_000);

    let lst_out = mul_div_up(sol_out, SOL_PRECISION, lst_to_sol_rate).unwrap();
    assert_eq!(lst_out, 952_380_953);
}

#[test]
fn rounding_reserve_math_credit_and_debit_is_consistent() {
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
fn fee_curve_interpolation_and_clamps_hold() {
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
        ),
        Some(150)
    );

    assert_eq!(
        compute_dynamic_fee_bps(
            base,
            FeeAction::AmUSDRedeem,
            14_000,
            min_cr,
            target_cr,
            min_mult,
            max_mult,
            0,
            20_000,
        ),
        Some(75)
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

    assert_eq!(
        compute_dynamic_fee_bps(
            base,
            FeeAction::AmusdMint,
            14_000,
            min_cr,
            target_cr,
            12_000,
            9_000,
            0,
            20_000
        ),
        None
    );
}

#[derive(Clone, Copy)]
struct ModelState {
    total_lst_amount: u64,
    amusd_supply: u64,
    asol_supply: u64,
    rounding_reserve_lamports: u64,
    max_rounding_reserve_lamports: u64,
    sol_price_usd: u64,
    lst_to_sol_rate: u64,
    min_cr_bps: u64,
    target_cr_bps: u64,
    fee_amusd_mint_bps: u64,
    fee_amusd_redeem_bps: u64,
    fee_asol_mint_bps: u64,
    fee_asol_redeem_bps: u64,
    fee_min_multiplier_bps: u64,
    fee_max_multiplier_bps: u64,
    uncertainty_index_bps: u64,
    uncertainty_max_bps: u64,
}

impl ModelState {
    fn seeded() -> Self {
        let total_lst_amount = 1_500 * SOL_PRECISION;
        let lst_to_sol_rate = 1_050_000_000u64;
        let sol_price_usd = 100 * USD_PRECISION;
        let amusd_supply = 80_000 * USD_PRECISION;

        let tvl = compute_tvl_sol(total_lst_amount, lst_to_sol_rate).unwrap();
        let liability = compute_liability_sol(amusd_supply, sol_price_usd).unwrap();
        let asol_supply = compute_claimable_equity_sol(tvl, liability, 0).unwrap();

        Self {
            total_lst_amount,
            amusd_supply,
            asol_supply,
            rounding_reserve_lamports: 0,
            max_rounding_reserve_lamports: 1_000_000_000,
            sol_price_usd,
            lst_to_sol_rate,
            min_cr_bps: 13_000,
            target_cr_bps: 15_000,
            fee_amusd_mint_bps: 50,
            fee_amusd_redeem_bps: 25,
            fee_asol_mint_bps: 30,
            fee_asol_redeem_bps: 15,
            fee_min_multiplier_bps: BPS_PRECISION,
            fee_max_multiplier_bps: 40_000,
            uncertainty_index_bps: 0,
            uncertainty_max_bps: 20_000,
        }
    }

    fn tvl(self) -> u64 {
        compute_tvl_sol(self.total_lst_amount, self.lst_to_sol_rate).unwrap()
    }

    fn liability(self) -> u64 {
        if self.amusd_supply == 0 {
            0
        } else {
            compute_liability_sol(self.amusd_supply, self.sol_price_usd).unwrap()
        }
    }
}

fn xorshift64(seed: &mut u64) -> u64 {
    let mut x = *seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *seed = x;
    x
}

fn rand_range(seed: &mut u64, lo: u64, hi: u64) -> u64 {
    if hi <= lo {
        return lo;
    }
    lo + (xorshift64(seed) % (hi - lo + 1))
}

fn assert_model_invariants(state: &ModelState, rounding_bound_lamports: u64) {
    let tvl = state.tvl();
    let liability = state.liability();
    let accounting_equity =
        compute_accounting_equity_sol(tvl, liability, state.rounding_reserve_lamports).unwrap();

    assert_rounding_reserve_within_cap(
        state.rounding_reserve_lamports,
        state.max_rounding_reserve_lamports,
    )
    .unwrap();

    assert_balance_sheet_holds(
        tvl,
        liability,
        accounting_equity,
        state.rounding_reserve_lamports,
        rounding_bound_lamports,
    )
    .unwrap();
}

fn model_mint_amusd(state: &mut ModelState, lst_amount: u64) -> Option<u64> {
    if lst_amount < MIN_LST_DEPOSIT {
        return None;
    }

    let old_tvl = state.tvl();
    let old_liability = state.liability();
    let old_cr = compute_cr_bps(old_tvl, old_liability);

    let sol_value = compute_tvl_sol(lst_amount, state.lst_to_sol_rate)?;
    let sol_value_up = mul_div_up(lst_amount, state.lst_to_sol_rate, SOL_PRECISION)?;

    let amusd_gross = mul_div_down(sol_value, state.sol_price_usd, SOL_PRECISION)?;
    if amusd_gross < MIN_AMUSD_MINT {
        return None;
    }

    let amusd_gross_up = mul_div_up(sol_value_up, state.sol_price_usd, SOL_PRECISION)?;
    let delta_usd = compute_rounding_delta_units(amusd_gross, amusd_gross_up)?;
    let reserve_credit = usd_dust_to_lamports_up(delta_usd, state.sol_price_usd)?;

    let fee_bps = compute_dynamic_fee_bps(
        state.fee_amusd_mint_bps,
        FeeAction::AmusdMint,
        old_cr,
        state.min_cr_bps,
        state.target_cr_bps,
        state.fee_min_multiplier_bps,
        state.fee_max_multiplier_bps,
        state.uncertainty_index_bps,
        state.uncertainty_max_bps,
    )?;

    let (amusd_to_user, _) = apply_fee(amusd_gross, fee_bps)?;
    if amusd_to_user < MIN_AMUSD_MINT {
        return None;
    }

    let new_lst = state.total_lst_amount.checked_add(lst_amount)?;
    let new_amusd_supply = state.amusd_supply.checked_add(amusd_gross)?;
    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate)?;
    let new_liability = compute_liability_sol(new_amusd_supply, state.sol_price_usd)?;
    let new_reserve = credit_rounding_reserve(
        state.rounding_reserve_lamports,
        reserve_credit,
        state.max_rounding_reserve_lamports,
    )
    .ok()?;

    let new_cr = compute_cr_bps(new_tvl, new_liability);
    if assert_cr_above_minimum(new_cr, state.min_cr_bps).is_err() {
        return None;
    }

    let equity = compute_accounting_equity_sol(new_tvl, new_liability, new_reserve)?;
    let bound = derive_rounding_bound_lamports(2, 1, state.sol_price_usd).ok()?;
    if assert_balance_sheet_holds(new_tvl, new_liability, equity, new_reserve, bound).is_err() {
        return None;
    }

    state.total_lst_amount = new_lst;
    state.amusd_supply = new_amusd_supply;
    state.rounding_reserve_lamports = new_reserve;

    Some(bound)
}

fn model_redeem_amusd(state: &mut ModelState, amusd_amount: u64) -> Option<u64> {
    if amusd_amount == 0 || state.amusd_supply == 0 {
        return None;
    }

    let amount = amusd_amount.min(state.amusd_supply);

    let old_tvl = state.tvl();
    let old_liability = state.liability();
    let old_cr = compute_cr_bps(old_tvl, old_liability);
    let insolvency_mode = old_cr < BPS_PRECISION;

    let (amusd_net_in, _) = if insolvency_mode {
        (amount, 0u64)
    } else {
        let fee_bps = compute_dynamic_fee_bps(
            state.fee_amusd_redeem_bps,
            FeeAction::AmUSDRedeem,
            old_cr,
            state.min_cr_bps,
            state.target_cr_bps,
            state.fee_min_multiplier_bps,
            state.fee_max_multiplier_bps,
            state.uncertainty_index_bps,
            state.uncertainty_max_bps,
        )?;
        let (net, fee) = apply_fee(amount, fee_bps)?;
        if net == 0 {
            return None;
        }
        (net, fee)
    };

    let sol_par_down = mul_div_down(amusd_net_in, SOL_PRECISION, state.sol_price_usd)?;
    let lst_par_down = mul_div_down(sol_par_down, SOL_PRECISION, state.lst_to_sol_rate)?;

    let (lst_out, reserve_debit, rounding_k_lamports) = if insolvency_mode {
        let haircut_bps = old_cr.min(BPS_PRECISION);
        let sol_haircut = mul_div_down(sol_par_down, haircut_bps, BPS_PRECISION)?;
        let lst_haircut = mul_div_down(sol_haircut, SOL_PRECISION, state.lst_to_sol_rate)?;
        (lst_haircut, 0u64, 3u64)
    } else {
        let sol_up = mul_div_up(amusd_net_in, SOL_PRECISION, state.sol_price_usd)?;
        let lst_up = mul_div_up(sol_up, SOL_PRECISION, state.lst_to_sol_rate)?;
        let delta_lst = compute_rounding_delta_units(lst_par_down, lst_up)?;
        let lamport_debit = lst_dust_to_lamports_up(delta_lst, state.lst_to_sol_rate)?;

        if lamport_debit <= state.rounding_reserve_lamports {
            (lst_up, lamport_debit, 2u64)
        } else {
            (lst_par_down, 0u64, 2u64)
        }
    };

    if lst_out < MIN_LST_DEPOSIT {
        return None;
    }

    let new_lst = state.total_lst_amount.checked_sub(lst_out)?;
    if !(new_lst >= MIN_PROTOCOL_TVL || new_lst == 0) {
        return None;
    }

    let new_amusd_supply = state.amusd_supply.checked_sub(amusd_net_in)?;
    let new_liability = if new_amusd_supply == 0 {
        0
    } else {
        compute_liability_sol(new_amusd_supply, state.sol_price_usd)?
    };

    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate)?;
    let new_reserve = debit_rounding_reserve(state.rounding_reserve_lamports, reserve_debit).ok()?;
    let new_equity = compute_accounting_equity_sol(new_tvl, new_liability, new_reserve)?;
    let bound = derive_rounding_bound_lamports(rounding_k_lamports, 1, state.sol_price_usd).ok()?;

    if assert_balance_sheet_holds(new_tvl, new_liability, new_equity, new_reserve, bound).is_err()
    {
        return None;
    }

    state.total_lst_amount = new_lst;
    state.amusd_supply = new_amusd_supply;
    state.rounding_reserve_lamports = new_reserve;

    Some(bound)
}

fn model_mint_asol(state: &mut ModelState, lst_amount: u64) -> Option<u64> {
    if lst_amount < MIN_LST_DEPOSIT {
        return None;
    }

    let old_tvl = state.tvl();
    let old_liability = state.liability();
    let old_cr = compute_cr_bps(old_tvl, old_liability);
    let old_claimable =
        compute_claimable_equity_sol(old_tvl, old_liability, state.rounding_reserve_lamports)?;

    let bound = derive_rounding_bound_lamports(2, 0, state.sol_price_usd).ok()?;
    let mut effective_reserve = state.rounding_reserve_lamports;

    if state.asol_supply == 0 {
        if old_tvl < old_liability {
            return None;
        }

        let lhs = old_tvl as i128;
        let rhs = (old_liability as i128).checked_add(effective_reserve as i128)?;
        let diff = if lhs >= rhs {
            (lhs - rhs) as u128
        } else {
            (rhs - lhs) as u128
        };

        if diff > bound as u128 {
            return None;
        }

        if old_claimable > 0 {
            effective_reserve = effective_reserve.checked_add(old_claimable)?;
            if effective_reserve > state.max_rounding_reserve_lamports {
                return None;
            }
        }
    }

    let sol_value = compute_tvl_sol(lst_amount, state.lst_to_sol_rate)?;
    let sol_value_up = mul_div_up(lst_amount, state.lst_to_sol_rate, SOL_PRECISION)?;

    let current_nav = if state.asol_supply == 0 {
        SOL_PRECISION
    } else {
        let nav = nav_asol_with_reserve(old_tvl, old_liability, effective_reserve, state.asol_supply)?;
        if nav == 0 {
            return None;
        }
        nav
    };

    let asol_gross = if state.asol_supply == 0 {
        sol_value
    } else {
        mul_div_down(sol_value, SOL_PRECISION, current_nav)?
    };

    let asol_ref_up = if state.asol_supply == 0 {
        sol_value_up
    } else {
        mul_div_up(sol_value_up, SOL_PRECISION, current_nav)?
    };

    let delta_asol = compute_rounding_delta_units(asol_gross, asol_ref_up)?;
    let reserve_credit = if state.asol_supply == 0 {
        delta_asol
    } else {
        asol_dust_to_lamports_up(delta_asol, current_nav)?
    };

    let fee_bps = compute_dynamic_fee_bps(
        state.fee_asol_mint_bps,
        FeeAction::AsolMint,
        old_cr,
        state.min_cr_bps,
        state.target_cr_bps,
        state.fee_min_multiplier_bps,
        state.fee_max_multiplier_bps,
        state.uncertainty_index_bps,
        state.uncertainty_max_bps,
    )?;

    let (asol_net, _) = apply_fee(asol_gross, fee_bps)?;
    if asol_net < MIN_ASOL_MINT {
        return None;
    }

    let new_lst = state.total_lst_amount.checked_add(lst_amount)?;
    let new_asol_supply = state.asol_supply.checked_add(asol_gross)?;
    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate)?;
    let new_reserve = credit_rounding_reserve(
        effective_reserve,
        reserve_credit,
        state.max_rounding_reserve_lamports,
    )
    .ok()?;
    let new_equity = compute_accounting_equity_sol(new_tvl, old_liability, new_reserve)?;

    if assert_balance_sheet_holds(new_tvl, old_liability, new_equity, new_reserve, bound).is_err() {
        return None;
    }

    state.total_lst_amount = new_lst;
    state.asol_supply = new_asol_supply;
    state.rounding_reserve_lamports = new_reserve;

    Some(bound)
}

fn model_redeem_asol(state: &mut ModelState, asol_amount: u64) -> Option<u64> {
    if asol_amount == 0 || state.asol_supply == 0 {
        return None;
    }

    let amount = asol_amount.min(state.asol_supply);

    let old_tvl = state.tvl();
    let old_liability = state.liability();
    let old_cr = compute_cr_bps(old_tvl, old_liability);
    let solvent_mode = old_cr >= BPS_PRECISION;

    let fee_bps = compute_dynamic_fee_bps(
        state.fee_asol_redeem_bps,
        FeeAction::AsolRedeem,
        old_cr,
        state.min_cr_bps,
        state.target_cr_bps,
        state.fee_min_multiplier_bps,
        state.fee_max_multiplier_bps,
        state.uncertainty_index_bps,
        state.uncertainty_max_bps,
    )?;

    let (asol_net_in, _) = apply_fee(amount, fee_bps)?;
    if asol_net_in == 0 {
        return None;
    }

    let nav = nav_asol_with_reserve(
        old_tvl,
        old_liability,
        state.rounding_reserve_lamports,
        state.asol_supply,
    )?;
    if nav == 0 {
        return None;
    }

    let sol_down = mul_div_down(asol_net_in, nav, SOL_PRECISION)?;
    let lst_down = mul_div_down(sol_down, SOL_PRECISION, state.lst_to_sol_rate)?;

    let (lst_out, reserve_debit) = if solvent_mode {
        let sol_up = mul_div_up(asol_net_in, nav, SOL_PRECISION)?;
        let lst_up = mul_div_up(sol_up, SOL_PRECISION, state.lst_to_sol_rate)?;
        let delta_lst = compute_rounding_delta_units(lst_down, lst_up)?;
        let debit = lst_dust_to_lamports_up(delta_lst, state.lst_to_sol_rate)?;
        if debit <= state.rounding_reserve_lamports {
            (lst_up, debit)
        } else {
            (lst_down, 0u64)
        }
    } else {
        (lst_down, 0u64)
    };

    if lst_out < MIN_LST_DEPOSIT {
        return None;
    }

    let new_lst = state.total_lst_amount.checked_sub(lst_out)?;
    if !(new_lst >= MIN_PROTOCOL_TVL || new_lst == 0) {
        return None;
    }

    let new_asol_supply = state.asol_supply.checked_sub(asol_net_in)?;
    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate)?;
    let new_reserve = debit_rounding_reserve(state.rounding_reserve_lamports, reserve_debit).ok()?;
    let new_equity = compute_accounting_equity_sol(new_tvl, old_liability, new_reserve)?;
    let new_cr = if old_liability == 0 {
        u64::MAX
    } else {
        compute_cr_bps(new_tvl, old_liability)
    };

    if assert_cr_above_minimum(new_cr, state.min_cr_bps).is_err() {
        return None;
    }

    let bound = derive_rounding_bound_lamports(2, 0, state.sol_price_usd).ok()?;
    if assert_balance_sheet_holds(new_tvl, old_liability, new_equity, new_reserve, bound).is_err() {
        return None;
    }

    state.total_lst_amount = new_lst;
    state.asol_supply = new_asol_supply;
    state.rounding_reserve_lamports = new_reserve;

    Some(bound)
}

#[test]
fn property_random_action_sequences_preserve_invariants() {
    const SEEDS: u64 = 50;
    const STEPS_PER_SEED: usize = 10_000;

    for seed in 1..=SEEDS {
        let mut rng = seed;
        let mut state = ModelState::seeded();

        for _ in 0..STEPS_PER_SEED {
            if xorshift64(&mut rng) % 97 == 0 {
                state.sol_price_usd = rand_range(&mut rng, 40 * USD_PRECISION, 160 * USD_PRECISION);
                state.uncertainty_index_bps = rand_range(&mut rng, 0, 1_000);
            }
            if xorshift64(&mut rng) % 131 == 0 {
                state.lst_to_sol_rate = rand_range(&mut rng, 900_000_000, 1_150_000_000);
            }

            let maybe_bound = match xorshift64(&mut rng) % 4 {
                0 => {
                    let amt = rand_range(&mut rng, MIN_LST_DEPOSIT, 20 * SOL_PRECISION);
                    model_mint_amusd(&mut state, amt)
                }
                1 => {
                    let cap = state.amusd_supply.min(2_000 * USD_PRECISION);
                    let amt = if cap == 0 { 0 } else { rand_range(&mut rng, 1, cap) };
                    model_redeem_amusd(&mut state, amt)
                }
                2 => {
                    let amt = rand_range(&mut rng, MIN_LST_DEPOSIT, 20 * SOL_PRECISION);
                    model_mint_asol(&mut state, amt)
                }
                _ => {
                    let cap = state.asol_supply.min(20 * SOL_PRECISION);
                    let amt = if cap == 0 { 0 } else { rand_range(&mut rng, 1, cap) };
                    model_redeem_asol(&mut state, amt)
                }
            };

            let bound = maybe_bound
                .unwrap_or_else(|| derive_rounding_bound_lamports(3, 1, state.sol_price_usd).unwrap());

            assert_model_invariants(&state, bound);
        }
    }
}
