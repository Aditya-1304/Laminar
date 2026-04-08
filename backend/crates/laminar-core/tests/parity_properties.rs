use laminar_core::{
    apply_fee, asol_dust_to_lamports_up, balance_sheet_holds, compute_accounting_equity_sol,
    compute_claimable_equity_sol, compute_cr_bps, compute_dynamic_fee_bps, compute_liability_sol,
    compute_rounding_delta_units, compute_tvl_sol, credit_rounding_reserve, debit_rounding_reserve,
    derive_rounding_bound_lamports, lst_dust_to_lamports_up, mul_div_down, mul_div_up,
    nav_asol_with_reserve, preview_mint_amusd, preview_mint_asol, preview_redeem_amusd,
    preview_redeem_asol, usd_dust_to_lamports_up, FeeAction, MintAmusdPreviewInput,
    MintAsolPreviewInput, RedeemAmusdPreviewInput, RedeemAsolPreviewInput, VaultBalanceSheet,
    VaultQuoteContext, BPS_PRECISION, MIN_AMUSD_MINT, MIN_ASOL_MINT, MIN_LST_DEPOSIT,
    MIN_PROTOCOL_TVL, SOL_PRECISION, USD_PRECISION,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    fn to_vault_context(self) -> VaultQuoteContext {
        VaultQuoteContext {
            current_lst_amount: self.total_lst_amount,
            current_amusd_supply: self.amusd_supply,
            current_asol_supply: self.asol_supply,
            current_rounding_reserve_lamports: self.rounding_reserve_lamports,
            max_rounding_reserve_lamports: self.max_rounding_reserve_lamports,
            lst_to_sol_rate: self.lst_to_sol_rate,
            safe_price_usd: self.sol_price_usd,
            redeem_price_usd: self.sol_price_usd,
            min_cr_bps: self.min_cr_bps,
            target_cr_bps: self.target_cr_bps,
            uncertainty_index_bps: self.uncertainty_index_bps,
            uncertainty_max_bps: self.uncertainty_max_bps,
            fee_amusd_mint_bps: self.fee_amusd_mint_bps,
            fee_amusd_redeem_bps: self.fee_amusd_redeem_bps,
            fee_asol_mint_bps: self.fee_asol_mint_bps,
            fee_asol_redeem_bps: self.fee_asol_redeem_bps,
            fee_min_multiplier_bps: self.fee_min_multiplier_bps,
            fee_max_multiplier_bps: self.fee_max_multiplier_bps,
            mint_paused: false,
            redeem_paused: false,
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

fn sync_state_from_sheet(state: &mut ModelState, sheet: &VaultBalanceSheet) {
    state.total_lst_amount = sheet.lst_amount;
    state.amusd_supply = sheet.amusd_supply;
    state.asol_supply = sheet.asol_supply;
    state.rounding_reserve_lamports = sheet.rounding_reserve_lamports;
}

fn assert_model_invariants(state: &ModelState, rounding_bound_lamports: u64) {
    let tvl = state.tvl();
    let liability = state.liability();
    let accounting_equity =
        compute_accounting_equity_sol(tvl, liability, state.rounding_reserve_lamports).unwrap();

    assert!(state.rounding_reserve_lamports <= state.max_rounding_reserve_lamports);
    assert!(balance_sheet_holds(
        tvl,
        liability,
        accounting_equity,
        state.rounding_reserve_lamports,
        rounding_bound_lamports,
    )
    .unwrap());
}

fn model_mint_amusd(state: &mut ModelState, lst_amount: u64) -> Option<u64> {
    if lst_amount < MIN_LST_DEPOSIT {
        return None;
    }

    let old_tvl = state.tvl();
    let old_liability = state.liability();
    let old_cr = compute_cr_bps(old_tvl, old_liability);

    let sol_value = compute_tvl_sol(lst_amount, state.lst_to_sol_rate).ok()?;
    let sol_value_up = mul_div_up(lst_amount, state.lst_to_sol_rate, SOL_PRECISION).ok()?;

    let amusd_gross = mul_div_down(sol_value, state.sol_price_usd, SOL_PRECISION).ok()?;
    if amusd_gross < MIN_AMUSD_MINT {
        return None;
    }

    let amusd_gross_up = mul_div_up(sol_value_up, state.sol_price_usd, SOL_PRECISION).ok()?;
    let delta_usd = compute_rounding_delta_units(amusd_gross, amusd_gross_up).ok()?;
    let reserve_credit = usd_dust_to_lamports_up(delta_usd, state.sol_price_usd).ok()?;

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
    )
    .ok()?;

    let (amusd_to_user, _) = apply_fee(amusd_gross, fee_bps).ok()?;
    if amusd_to_user < MIN_AMUSD_MINT {
        return None;
    }

    let new_lst = state.total_lst_amount.checked_add(lst_amount)?;
    let new_amusd_supply = state.amusd_supply.checked_add(amusd_gross)?;
    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate).ok()?;
    let new_liability = compute_liability_sol(new_amusd_supply, state.sol_price_usd).ok()?;
    let new_reserve = credit_rounding_reserve(
        state.rounding_reserve_lamports,
        reserve_credit,
        state.max_rounding_reserve_lamports,
    )
    .ok()?;

    let new_cr = compute_cr_bps(new_tvl, new_liability);
    if new_cr < state.min_cr_bps {
        return None;
    }

    let equity = compute_accounting_equity_sol(new_tvl, new_liability, new_reserve).ok()?;
    let bound = derive_rounding_bound_lamports(2, 1, state.sol_price_usd).ok()?;
    if !balance_sheet_holds(new_tvl, new_liability, equity, new_reserve, bound).ok()? {
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
            FeeAction::AmusdRedeem,
            old_cr,
            state.min_cr_bps,
            state.target_cr_bps,
            state.fee_min_multiplier_bps,
            state.fee_max_multiplier_bps,
            state.uncertainty_index_bps,
            state.uncertainty_max_bps,
        )
        .ok()?;
        let (net, fee) = apply_fee(amount, fee_bps).ok()?;
        if net == 0 {
            return None;
        }
        (net, fee)
    };

    let sol_par_down = mul_div_down(amusd_net_in, SOL_PRECISION, state.sol_price_usd).ok()?;
    let lst_par_down = mul_div_down(sol_par_down, SOL_PRECISION, state.lst_to_sol_rate).ok()?;

    let (lst_out, reserve_debit, rounding_k_lamports) = if insolvency_mode {
        let haircut_bps = old_cr.min(BPS_PRECISION);
        let sol_haircut = mul_div_down(sol_par_down, haircut_bps, BPS_PRECISION).ok()?;
        let lst_haircut = mul_div_down(sol_haircut, SOL_PRECISION, state.lst_to_sol_rate).ok()?;
        (lst_haircut, 0u64, 3u64)
    } else {
        let sol_up = mul_div_up(amusd_net_in, SOL_PRECISION, state.sol_price_usd).ok()?;
        let lst_up = mul_div_up(sol_up, SOL_PRECISION, state.lst_to_sol_rate).ok()?;
        let delta_lst = compute_rounding_delta_units(lst_par_down, lst_up).ok()?;
        let lamport_debit = lst_dust_to_lamports_up(delta_lst, state.lst_to_sol_rate).ok()?;

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
        compute_liability_sol(new_amusd_supply, state.sol_price_usd).ok()?
    };

    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate).ok()?;
    let new_reserve =
        debit_rounding_reserve(state.rounding_reserve_lamports, reserve_debit).ok()?;
    let new_equity = compute_accounting_equity_sol(new_tvl, new_liability, new_reserve).ok()?;
    let bound = derive_rounding_bound_lamports(rounding_k_lamports, 1, state.sol_price_usd).ok()?;

    if !balance_sheet_holds(new_tvl, new_liability, new_equity, new_reserve, bound).ok()? {
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
        compute_claimable_equity_sol(old_tvl, old_liability, state.rounding_reserve_lamports)
            .ok()?;

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

    let sol_value = compute_tvl_sol(lst_amount, state.lst_to_sol_rate).ok()?;
    let sol_value_up = mul_div_up(lst_amount, state.lst_to_sol_rate, SOL_PRECISION).ok()?;

    let current_nav = if state.asol_supply == 0 {
        SOL_PRECISION
    } else {
        let nav =
            nav_asol_with_reserve(old_tvl, old_liability, effective_reserve, state.asol_supply)
                .ok()
                .flatten()?;
        if nav == 0 {
            return None;
        }
        nav
    };

    let asol_gross = if state.asol_supply == 0 {
        sol_value
    } else {
        mul_div_down(sol_value, SOL_PRECISION, current_nav).ok()?
    };

    let asol_ref_up = if state.asol_supply == 0 {
        sol_value_up
    } else {
        mul_div_up(sol_value_up, SOL_PRECISION, current_nav).ok()?
    };

    let delta_asol = compute_rounding_delta_units(asol_gross, asol_ref_up).ok()?;
    let reserve_credit = if state.asol_supply == 0 {
        delta_asol
    } else {
        asol_dust_to_lamports_up(delta_asol, current_nav).ok()?
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
    )
    .ok()?;

    let (asol_net, _) = apply_fee(asol_gross, fee_bps).ok()?;
    if asol_net < MIN_ASOL_MINT {
        return None;
    }

    let new_lst = state.total_lst_amount.checked_add(lst_amount)?;
    let new_asol_supply = state.asol_supply.checked_add(asol_gross)?;
    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate).ok()?;
    let new_reserve = credit_rounding_reserve(
        effective_reserve,
        reserve_credit,
        state.max_rounding_reserve_lamports,
    )
    .ok()?;
    let new_equity = compute_accounting_equity_sol(new_tvl, old_liability, new_reserve).ok()?;

    if !balance_sheet_holds(new_tvl, old_liability, new_equity, new_reserve, bound).ok()? {
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
    )
    .ok()?;

    let (asol_net_in, _) = apply_fee(amount, fee_bps).ok()?;
    if asol_net_in == 0 {
        return None;
    }

    let nav = nav_asol_with_reserve(
        old_tvl,
        old_liability,
        state.rounding_reserve_lamports,
        state.asol_supply,
    )
    .ok()
    .flatten()?;
    if nav == 0 {
        return None;
    }

    let sol_down = mul_div_down(asol_net_in, nav, SOL_PRECISION).ok()?;
    let lst_down = mul_div_down(sol_down, SOL_PRECISION, state.lst_to_sol_rate).ok()?;

    let (lst_out, reserve_debit) = if solvent_mode {
        let sol_up = mul_div_up(asol_net_in, nav, SOL_PRECISION).ok()?;
        let lst_up = mul_div_up(sol_up, SOL_PRECISION, state.lst_to_sol_rate).ok()?;
        let delta_lst = compute_rounding_delta_units(lst_down, lst_up).ok()?;
        let debit = lst_dust_to_lamports_up(delta_lst, state.lst_to_sol_rate).ok()?;
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
    let new_tvl = compute_tvl_sol(new_lst, state.lst_to_sol_rate).ok()?;
    let new_reserve =
        debit_rounding_reserve(state.rounding_reserve_lamports, reserve_debit).ok()?;
    let new_equity = compute_accounting_equity_sol(new_tvl, old_liability, new_reserve).ok()?;
    let new_cr = if old_liability == 0 {
        u64::MAX
    } else {
        compute_cr_bps(new_tvl, old_liability)
    };

    if new_cr < state.min_cr_bps {
        return None;
    }

    let bound = derive_rounding_bound_lamports(2, 0, state.sol_price_usd).ok()?;
    if !balance_sheet_holds(new_tvl, old_liability, new_equity, new_reserve, bound).ok()? {
        return None;
    }

    state.total_lst_amount = new_lst;
    state.asol_supply = new_asol_supply;
    state.rounding_reserve_lamports = new_reserve;

    Some(bound)
}

#[test]
fn property_random_action_sequences_preserve_invariants() {
    const SEEDS: u64 = 24;
    const STEPS_PER_SEED: usize = 2_000;

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
                    let amt = if cap == 0 {
                        0
                    } else {
                        rand_range(&mut rng, 1, cap)
                    };
                    model_redeem_amusd(&mut state, amt)
                }
                2 => {
                    let amt = rand_range(&mut rng, MIN_LST_DEPOSIT, 20 * SOL_PRECISION);
                    model_mint_asol(&mut state, amt)
                }
                _ => {
                    let cap = state.asol_supply.min(20 * SOL_PRECISION);
                    let amt = if cap == 0 {
                        0
                    } else {
                        rand_range(&mut rng, 1, cap)
                    };
                    model_redeem_asol(&mut state, amt)
                }
            };

            let bound = maybe_bound.unwrap_or_else(|| {
                derive_rounding_bound_lamports(3, 1, state.sol_price_usd).unwrap()
            });

            assert_model_invariants(&state, bound);
        }
    }
}

#[test]
fn property_preview_transitions_match_model_state() {
    const SEEDS: u64 = 12;
    const STEPS_PER_SEED: usize = 1_500;

    for seed in 1..=SEEDS {
        let mut rng = seed;
        let mut expected = ModelState::seeded();
        let mut actual = expected;

        for step in 0..STEPS_PER_SEED {
            if xorshift64(&mut rng) % 97 == 0 {
                let new_price = rand_range(&mut rng, 40 * USD_PRECISION, 160 * USD_PRECISION);
                let new_uncertainty = rand_range(&mut rng, 0, 1_000);
                expected.sol_price_usd = new_price;
                actual.sol_price_usd = new_price;
                expected.uncertainty_index_bps = new_uncertainty;
                actual.uncertainty_index_bps = new_uncertainty;
            }

            if xorshift64(&mut rng) % 131 == 0 {
                let new_rate = rand_range(&mut rng, 900_000_000, 1_150_000_000);
                expected.lst_to_sol_rate = new_rate;
                actual.lst_to_sol_rate = new_rate;
            }

            let action = xorshift64(&mut rng) % 4;

            match action {
                0 => {
                    let amt = rand_range(&mut rng, MIN_LST_DEPOSIT, 20 * SOL_PRECISION);
                    let expected_bound = model_mint_amusd(&mut expected, amt);
                    let preview = preview_mint_amusd(&MintAmusdPreviewInput {
                        context: actual.to_vault_context(),
                        lst_amount: amt,
                        min_amusd_out: 0,
                    });

                    match (expected_bound, preview) {
                        (Some(bound), Ok(preview)) => {
                            assert_eq!(preview.rounding_bound_lamports, bound);
                            sync_state_from_sheet(&mut actual, &preview.post_mint_balance_sheet);
                            assert_eq!(actual, expected, "seed {seed} step {step}: mint_amusd");
                            assert_model_invariants(&actual, bound);
                        }
                        (None, Err(_)) => {}
                        (Some(_), Err(err)) => {
                            panic!("seed {seed} step {step}: mint_amusd preview failed: {err:?}")
                        }
                        (None, Ok(preview)) => {
                            panic!("seed {seed} step {step}: mint_amusd unexpectedly succeeded: {preview:?}")
                        }
                    }
                }
                1 => {
                    let cap = actual.amusd_supply.min(2_000 * USD_PRECISION);
                    let amt = if cap == 0 {
                        0
                    } else {
                        rand_range(&mut rng, 1, cap)
                    };
                    let expected_bound = model_redeem_amusd(&mut expected, amt);
                    let preview = preview_redeem_amusd(&RedeemAmusdPreviewInput {
                        context: actual.to_vault_context(),
                        amusd_amount: amt,
                        min_lst_out: MIN_LST_DEPOSIT,
                        stability_pool_amusd_available: 0,
                        effective_lst_amount: None,
                        effective_amusd_supply: None,
                        effective_asol_supply: None,
                        effective_rounding_reserve_lamports: None,
                        drawdown_rounds_executed: 0,
                    });

                    match (expected_bound, preview) {
                        (Some(bound), Ok(preview)) => {
                            assert_eq!(preview.rounding_bound_lamports, bound);
                            sync_state_from_sheet(
                                &mut actual,
                                &preview.post_redemption_balance_sheet,
                            );
                            assert_eq!(actual, expected, "seed {seed} step {step}: redeem_amusd");
                            assert_model_invariants(&actual, bound);
                        }
                        (None, Err(_)) => {}
                        (Some(_), Err(err)) => {
                            panic!("seed {seed} step {step}: redeem_amusd preview failed: {err:?}")
                        }
                        (None, Ok(preview)) => {
                            panic!("seed {seed} step {step}: redeem_amusd unexpectedly succeeded: {preview:?}")
                        }
                    }
                }
                2 => {
                    let amt = rand_range(&mut rng, MIN_LST_DEPOSIT, 20 * SOL_PRECISION);
                    let expected_bound = model_mint_asol(&mut expected, amt);
                    let preview = preview_mint_asol(&MintAsolPreviewInput {
                        context: actual.to_vault_context(),
                        lst_amount: amt,
                        min_asol_out: 0,
                    });

                    match (expected_bound, preview) {
                        (Some(bound), Ok(preview)) => {
                            assert_eq!(preview.rounding_bound_lamports, bound);
                            sync_state_from_sheet(&mut actual, &preview.post_mint_balance_sheet);
                            assert_eq!(actual, expected, "seed {seed} step {step}: mint_asol");
                            assert_model_invariants(&actual, bound);
                        }
                        (None, Err(_)) => {}
                        (Some(_), Err(err)) => {
                            panic!("seed {seed} step {step}: mint_asol preview failed: {err:?}")
                        }
                        (None, Ok(preview)) => {
                            panic!("seed {seed} step {step}: mint_asol unexpectedly succeeded: {preview:?}")
                        }
                    }
                }
                _ => {
                    let cap = actual.asol_supply.min(20 * SOL_PRECISION);
                    let amt = if cap == 0 {
                        0
                    } else {
                        rand_range(&mut rng, 1, cap)
                    };
                    let expected_bound = model_redeem_asol(&mut expected, amt);
                    let preview = preview_redeem_asol(&RedeemAsolPreviewInput {
                        context: actual.to_vault_context(),
                        asol_amount: amt,
                        min_lst_out: MIN_LST_DEPOSIT,
                    });

                    match (expected_bound, preview) {
                        (Some(bound), Ok(preview)) => {
                            assert_eq!(preview.rounding_bound_lamports, bound);
                            sync_state_from_sheet(
                                &mut actual,
                                &preview.post_redemption_balance_sheet,
                            );
                            assert_eq!(actual, expected, "seed {seed} step {step}: redeem_asol");
                            assert_model_invariants(&actual, bound);
                        }
                        (None, Err(_)) => {}
                        (Some(_), Err(err)) => {
                            panic!("seed {seed} step {step}: redeem_asol preview failed: {err:?}")
                        }
                        (None, Ok(preview)) => {
                            panic!("seed {seed} step {step}: redeem_asol unexpectedly succeeded: {preview:?}")
                        }
                    }
                }
            }
        }
    }
}
