use crate::{
    math::{build_vault_balance_sheet, MathResult, BPS_PRECISION},
    models::{
        BalanceSheetSnapshot, Epoch, GlobalStateModel, LaminarProtocolSnapshot, LstRateSnapshot,
        OracleSnapshot, ProjectionMetadata, Slot, StabilityPoolSnapshot,
    },
    normalization::normalize_stability_withdrawals_paused,
    quote::{StabilityPoolQuoteContext, VaultQuoteContext},
    risk::{
        classify_collateralization_mode, derive_confidence_bps, derive_lst_age_epochs,
        derive_oracle_age_slots, ProtocolRiskFlags, ProtocolRiskSnapshot,
    },
};

pub fn build_balance_sheet_snapshot(global: &GlobalStateModel) -> MathResult<BalanceSheetSnapshot> {
    let sheet = build_vault_balance_sheet(
        global.total_lst_amount,
        global.amusd_supply,
        global.asol_supply,
        global.rounding_reserve_lamports,
        global.mock_lst_to_sol_rate,
        global.mock_sol_price_usd,
    )?;

    Ok(BalanceSheetSnapshot {
        tvl_lamports: sheet.tvl_lamports,
        liability_lamports: sheet.liability_lamports,
        accounting_equity_lamports: sheet.accounting_equity_lamports,
        claimable_equity_lamports: sheet.claimable_equity_lamports,
        collateral_ratio_bps: sheet.collateral_ratio_bps,
        nav_amusd_lamports: sheet.nav_amusd_lamports,
        nav_asol_lamports: sheet.nav_asol_lamports,
        rounding_reserve_lamports: sheet.rounding_reserve_lamports,
        max_rounding_reserve_lamports: global.max_rounding_reserve_lamports,
    })
}

pub fn build_protocol_snapshot(
    global: GlobalStateModel,
    oracle: OracleSnapshot,
    lst_rate: LstRateSnapshot,
    stability_pool: StabilityPoolSnapshot,
    metadata: ProjectionMetadata,
) -> MathResult<LaminarProtocolSnapshot> {
    let balance_sheet = build_balance_sheet_snapshot(&global)?;

    Ok(LaminarProtocolSnapshot {
        global,
        oracle,
        lst_rate,
        stability_pool,
        balance_sheet,
        metadata,
    })
}

pub fn build_vault_quote_context(snapshot: &LaminarProtocolSnapshot) -> VaultQuoteContext {
    let global = &snapshot.global;
    let oracle = &snapshot.oracle;

    VaultQuoteContext {
        current_lst_amount: global.total_lst_amount,
        current_amusd_supply: global.amusd_supply,
        current_asol_supply: global.asol_supply,
        current_rounding_reserve_lamports: global.rounding_reserve_lamports,
        max_rounding_reserve_lamports: global.max_rounding_reserve_lamports,

        lst_to_sol_rate: global.mock_lst_to_sol_rate,
        safe_price_usd: oracle.price_safe_usd,
        redeem_price_usd: oracle.price_redeem_usd,

        min_cr_bps: global.min_cr_bps,
        target_cr_bps: global.target_cr_bps,

        uncertainty_index_bps: global.uncertainty_index_bps,
        uncertainty_max_bps: global.uncertainty_max_bps,

        fee_amusd_mint_bps: global.fee_amusd_mint_bps,
        fee_amusd_redeem_bps: global.fee_amusd_redeem_bps,
        fee_asol_mint_bps: global.fee_asol_mint_bps,
        fee_asol_redeem_bps: global.fee_asol_redeem_bps,
        fee_min_multiplier_bps: global.fee_min_multiplier_bps,
        fee_max_multiplier_bps: global.fee_max_multiplier_bps,

        mint_paused: global.mint_paused,
        redeem_paused: global.redeem_paused,
    }
}

pub fn build_stability_pool_quote_context(
    snapshot: &LaminarProtocolSnapshot,
) -> StabilityPoolQuoteContext {
    StabilityPoolQuoteContext {
        total_amusd: snapshot.stability_pool.total_amusd,
        total_asol: snapshot.stability_pool.total_asol,
        total_samusd: snapshot.stability_pool.total_samusd,
        stability_withdrawals_paused: normalize_stability_withdrawals_paused(
            snapshot.stability_pool.stability_withdrawals_paused,
        ),
        last_harvest_lst_to_sol_rate: snapshot.stability_pool.last_harvest_lst_to_sol_rate,

        price_safe_usd: snapshot.oracle.price_safe_usd,
        lst_to_sol_rate: snapshot.lst_rate.lst_to_sol_rate,
        nav_asol_lamports: snapshot.balance_sheet.nav_asol_lamports.unwrap_or(0),

        current_lst_amount: snapshot.global.total_lst_amount,
        current_amusd_supply: snapshot.global.amusd_supply,
        current_asol_supply: snapshot.global.asol_supply,
        current_rounding_reserve_lamports: snapshot.global.rounding_reserve_lamports,

        min_cr_bps: snapshot.global.min_cr_bps,
        nav_floor_lamports: snapshot.global.nav_floor_lamports,
        max_asol_mint_per_round: snapshot.global.max_asol_mint_per_round,
    }
}

pub fn build_protocol_risk_flags(
    snapshot: &LaminarProtocolSnapshot,
    current_slot: Slot,
    current_epoch: Epoch,
) -> MathResult<ProtocolRiskFlags> {
    let oracle_age_slots = derive_oracle_age_slots(current_slot, snapshot.oracle.last_update_slot);
    let lst_age_epochs =
        derive_lst_age_epochs(current_epoch, snapshot.lst_rate.last_lst_update_epoch);
    let confidence_bps = derive_confidence_bps(
        snapshot.oracle.confidence_usd,
        snapshot.oracle.price_ema_usd,
    )?;

    let oracle_stale = oracle_age_slots
        .map(|age| age > snapshot.oracle.max_staleness_slots)
        .unwrap_or(true);

    let lst_stale = lst_age_epochs
        .map(|age| age > snapshot.lst_rate.max_lst_stale_epochs)
        .unwrap_or(true);

    let high_confidence = confidence_bps
        .map(|bps| bps > snapshot.oracle.max_conf_bps)
        .unwrap_or(false);

    let insolvency_mode = snapshot
        .balance_sheet
        .collateral_ratio_bps
        .map(|cr| cr < BPS_PRECISION)
        .unwrap_or(false);

    let drawdown_expected = snapshot
        .balance_sheet
        .collateral_ratio_bps
        .map(|cr| cr < snapshot.global.min_cr_bps)
        .unwrap_or(false)
        && snapshot.stability_pool.total_amusd > 0;

    Ok(ProtocolRiskFlags {
        mint_paused: snapshot.global.mint_paused,
        redeem_paused: snapshot.global.redeem_paused,
        stability_withdrawals_paused: normalize_stability_withdrawals_paused(
            snapshot.stability_pool.stability_withdrawals_paused,
        ),
        oracle_stale,
        lst_stale,
        high_confidence,
        insolvency_mode,
        drawdown_expected,
    })
}

pub fn build_protocol_risk_snapshot(
    snapshot: &LaminarProtocolSnapshot,
    current_slot: Slot,
    current_epoch: Epoch,
) -> MathResult<ProtocolRiskSnapshot> {
    let flags = build_protocol_risk_flags(snapshot, current_slot, current_epoch)?;
    let oracle_age_slots = derive_oracle_age_slots(current_slot, snapshot.oracle.last_update_slot);
    let lst_age_epochs =
        derive_lst_age_epochs(current_epoch, snapshot.lst_rate.last_lst_update_epoch);
    let confidence_bps = derive_confidence_bps(
        snapshot.oracle.confidence_usd,
        snapshot.oracle.price_ema_usd,
    )?;

    Ok(ProtocolRiskSnapshot {
        flags,
        collateralization_mode: classify_collateralization_mode(
            snapshot.balance_sheet.collateral_ratio_bps,
            snapshot.global.min_cr_bps,
        ),
        collateral_ratio_bps: snapshot.balance_sheet.collateral_ratio_bps,
        min_cr_bps: snapshot.global.min_cr_bps,
        target_cr_bps: snapshot.global.target_cr_bps,
        oracle_age_slots,
        lst_age_epochs,
        confidence_bps,
        tvl_lamports: snapshot.balance_sheet.tvl_lamports,
        liability_lamports: snapshot.balance_sheet.liability_lamports,
    })
}
