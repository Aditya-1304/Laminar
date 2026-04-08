use std::str::FromStr;

use anchor_lang::prelude::Pubkey;
use laminar_chain::{
    build_execute_debt_equity_swap_instruction, build_harvest_yield_instruction,
    build_initialize_instruction, build_initialize_stability_pool_instruction,
    build_set_stability_withdrawals_paused_instruction, build_sync_exchange_rate_instruction,
    build_update_uncertainty_index_instruction, global_state_pda, stability_pool_amusd_vault,
    stability_pool_asol_vault, stability_pool_authority_pda, stability_pool_state_pda,
    vault_authority_pda, vault_token_account, ExecuteDebtEquitySwapBuildInput,
    HarvestYieldBuildInput, InitializeBuildInput, InitializeStabilityPoolBuildInput,
    LaminarBuilderConfig, ProgramAddresses, SetStabilityWithdrawalsPausedBuildInput,
    SyncExchangeRateBuildInput, UpdateUncertaintyIndexBuildInput, LAMINAR_PROGRAM_ID,
};
use laminar_core::{
    Address, GlobalStateModel, LstRateBackend, OracleBackend, StabilityPoolSnapshot,
};

fn addr(pubkey: Pubkey) -> Address {
    Address::from(pubkey.to_string())
}

fn as_pubkey(address: &Address) -> Pubkey {
    Pubkey::from_str(address.as_str()).unwrap()
}

fn sample_global_state(
    oracle_backend: OracleBackend,
    lst_rate_backend: LstRateBackend,
) -> GlobalStateModel {
    let treasury = Pubkey::new_unique();
    let amusd_mint = Pubkey::new_unique();
    let asol_mint = Pubkey::new_unique();
    let supported_lst_mint = Pubkey::new_unique();
    let pyth_feed = Pubkey::new_unique();
    let stake_pool = Pubkey::new_unique();

    GlobalStateModel {
        treasury: addr(treasury),
        amusd_mint: addr(amusd_mint),
        asol_mint: addr(asol_mint),
        supported_lst_mint: addr(supported_lst_mint),
        oracle_backend,
        lst_rate_backend,
        pyth_sol_usd_price_account: addr(pyth_feed),
        lst_stake_pool: addr(stake_pool),
        ..GlobalStateModel::default()
    }
}

fn sample_stability_pool(
    global_state_pubkey: Pubkey,
    amusd_mint: Pubkey,
    asol_mint: Pubkey,
    token_program: Pubkey,
) -> StabilityPoolSnapshot {
    StabilityPoolSnapshot {
        global_state: addr(global_state_pubkey),
        samusd_mint: addr(Pubkey::new_unique()),
        pool_amusd_vault: addr(stability_pool_amusd_vault(&amusd_mint, &token_program)),
        pool_asol_vault: addr(stability_pool_asol_vault(&asol_mint, &token_program)),
        ..StabilityPoolSnapshot::default()
    }
}

#[test]
fn initialize_builder_derives_global_and_vault_accounts() {
    let authority = Pubkey::new_unique();
    let amusd_mint = Pubkey::new_unique();
    let asol_mint = Pubkey::new_unique();
    let lst_mint = Pubkey::new_unique();
    let programs = ProgramAddresses::new(spl_token::id());

    let envelope = build_initialize_instruction(&InitializeBuildInput {
        authority,
        amusd_mint,
        asol_mint,
        lst_mint,
        programs: programs.clone(),
        config: LaminarBuilderConfig::default(),
        min_cr_bps: 13_000,
        target_cr_bps: 15_000,
        mock_sol_price_usd: 100_000_000,
        mock_lst_to_sol_rate: 1_000_000_000,
    })
    .unwrap();

    let ix = envelope.full_instruction();
    let (global_state, _) = global_state_pda();
    let (vault_authority, _) = vault_authority_pda();

    assert_eq!(ix.program_id, LAMINAR_PROGRAM_ID);
    assert_eq!(ix.accounts.len(), 11);
    assert!(envelope.remaining_accounts.is_empty());

    assert_eq!(ix.accounts[0].pubkey, authority);
    assert_eq!(ix.accounts[1].pubkey, global_state);
    assert_eq!(ix.accounts[2].pubkey, amusd_mint);
    assert_eq!(ix.accounts[3].pubkey, asol_mint);
    assert_eq!(
        ix.accounts[4].pubkey,
        vault_token_account(&lst_mint, &programs.token_program)
    );
    assert_eq!(ix.accounts[5].pubkey, lst_mint);
    assert_eq!(ix.accounts[6].pubkey, vault_authority);
    assert_eq!(ix.accounts[7].pubkey, programs.token_program);
    assert_eq!(ix.accounts[8].pubkey, programs.associated_token_program);
    assert_eq!(ix.accounts[9].pubkey, programs.system_program);
    assert_eq!(ix.accounts[10].pubkey, programs.clock_sysvar);
}

#[test]
fn initialize_stability_pool_builder_derives_pool_pdas_and_vaults() {
    let global = sample_global_state(OracleBackend::Mock, LstRateBackend::Mock);
    let authority = Pubkey::new_unique();
    let samusd_mint = Pubkey::new_unique();
    let programs = ProgramAddresses::new(spl_token::id());

    let envelope =
        build_initialize_stability_pool_instruction(&InitializeStabilityPoolBuildInput {
            global_state: global.clone(),
            authority,
            samusd_mint,
            programs: programs.clone(),
            config: LaminarBuilderConfig::default(),
        })
        .unwrap();

    let ix = envelope.full_instruction();
    let (global_state, _) = global_state_pda();
    let (stability_pool_state, _) = stability_pool_state_pda();
    let (stability_pool_authority, _) = stability_pool_authority_pda();
    let amusd_mint = as_pubkey(&global.amusd_mint);
    let asol_mint = as_pubkey(&global.asol_mint);

    assert_eq!(ix.program_id, LAMINAR_PROGRAM_ID);
    assert_eq!(ix.accounts.len(), 13);

    assert_eq!(ix.accounts[0].pubkey, authority);
    assert_eq!(ix.accounts[1].pubkey, global_state);
    assert_eq!(ix.accounts[2].pubkey, stability_pool_state);
    assert_eq!(ix.accounts[3].pubkey, stability_pool_authority);
    assert_eq!(ix.accounts[4].pubkey, samusd_mint);
    assert_eq!(
        ix.accounts[5].pubkey,
        stability_pool_amusd_vault(&amusd_mint, &programs.token_program)
    );
    assert_eq!(
        ix.accounts[6].pubkey,
        stability_pool_asol_vault(&asol_mint, &programs.token_program)
    );
    assert_eq!(ix.accounts[7].pubkey, amusd_mint);
    assert_eq!(ix.accounts[8].pubkey, asol_mint);
    assert_eq!(ix.accounts[9].pubkey, programs.token_program);
    assert_eq!(ix.accounts[10].pubkey, programs.associated_token_program);
    assert_eq!(ix.accounts[11].pubkey, programs.system_program);
    assert_eq!(ix.accounts[12].pubkey, programs.clock_sysvar);
}

#[test]
fn sync_exchange_rate_builder_only_attaches_lst_remaining_accounts() {
    let global = sample_global_state(OracleBackend::PythPush, LstRateBackend::SanctumStakePool);

    let envelope = build_sync_exchange_rate_instruction(&SyncExchangeRateBuildInput {
        global_state: global.clone(),
        config: LaminarBuilderConfig::default(),
    })
    .unwrap();

    let ix = envelope.full_instruction();
    let stake_pool = as_pubkey(&global.lst_stake_pool);
    let oracle_feed = as_pubkey(&global.pyth_sol_usd_price_account);

    assert_eq!(ix.accounts.len(), 3);
    assert_eq!(ix.accounts[2].pubkey, stake_pool);
    assert!(!ix.accounts.iter().any(|meta| meta.pubkey == oracle_feed));
}

#[test]
fn update_uncertainty_builder_only_attaches_oracle_remaining_accounts() {
    let global = sample_global_state(OracleBackend::PythPush, LstRateBackend::SanctumStakePool);
    let updater = Pubkey::new_unique();

    let envelope = build_update_uncertainty_index_instruction(&UpdateUncertaintyIndexBuildInput {
        global_state: global.clone(),
        updater,
        config: LaminarBuilderConfig::default(),
    })
    .unwrap();

    let ix = envelope.full_instruction();
    let stake_pool = as_pubkey(&global.lst_stake_pool);
    let oracle_feed = as_pubkey(&global.pyth_sol_usd_price_account);

    assert_eq!(ix.accounts.len(), 4);
    assert_eq!(ix.accounts[0].pubkey, updater);
    assert_eq!(ix.accounts[3].pubkey, oracle_feed);
    assert!(!ix.accounts.iter().any(|meta| meta.pubkey == stake_pool));
}

#[test]
fn harvest_yield_builder_appends_oracle_then_lst_remaining_accounts() {
    let global = sample_global_state(OracleBackend::PythPush, LstRateBackend::SanctumStakePool);
    let programs = ProgramAddresses::new(spl_token::id());
    let global_state_pubkey = global_state_pda().0;
    let stability = sample_stability_pool(
        global_state_pubkey,
        as_pubkey(&global.amusd_mint),
        as_pubkey(&global.asol_mint),
        programs.token_program,
    );
    let harvester = Pubkey::new_unique();

    let envelope = build_harvest_yield_instruction(&HarvestYieldBuildInput {
        global_state: global.clone(),
        stability_pool: stability.clone(),
        harvester,
        programs: programs.clone(),
        config: LaminarBuilderConfig::default(),
    })
    .unwrap();

    let ix = envelope.full_instruction();
    let oracle_feed = as_pubkey(&global.pyth_sol_usd_price_account);
    let stake_pool = as_pubkey(&global.lst_stake_pool);

    assert_eq!(ix.accounts.len(), 9);
    assert_eq!(ix.accounts[0].pubkey, harvester);
    assert_eq!(ix.accounts[3].pubkey, as_pubkey(&global.amusd_mint));
    assert_eq!(
        ix.accounts[4].pubkey,
        as_pubkey(&stability.pool_amusd_vault)
    );
    assert_eq!(ix.accounts[5].pubkey, programs.token_program);
    assert_eq!(ix.accounts[6].pubkey, programs.clock_sysvar);
    assert_eq!(ix.accounts[7].pubkey, oracle_feed);
    assert_eq!(ix.accounts[8].pubkey, stake_pool);
}

#[test]
fn execute_debt_equity_swap_builder_matches_keeper_account_order() {
    let global = sample_global_state(OracleBackend::PythPush, LstRateBackend::SanctumStakePool);
    let programs = ProgramAddresses::new(spl_token::id());
    let global_state_pubkey = global_state_pda().0;
    let stability = sample_stability_pool(
        global_state_pubkey,
        as_pubkey(&global.amusd_mint),
        as_pubkey(&global.asol_mint),
        programs.token_program,
    );
    let executor = Pubkey::new_unique();
    let (stability_pool_authority, _) = stability_pool_authority_pda();

    let envelope = build_execute_debt_equity_swap_instruction(&ExecuteDebtEquitySwapBuildInput {
        global_state: global.clone(),
        stability_pool: stability.clone(),
        executor,
        programs: programs.clone(),
        config: LaminarBuilderConfig::default(),
    })
    .unwrap();

    let ix = envelope.full_instruction();
    let oracle_feed = as_pubkey(&global.pyth_sol_usd_price_account);
    let stake_pool = as_pubkey(&global.lst_stake_pool);

    assert_eq!(ix.accounts.len(), 12);
    assert_eq!(ix.accounts[0].pubkey, executor);
    assert_eq!(ix.accounts[3].pubkey, stability_pool_authority);
    assert_eq!(ix.accounts[4].pubkey, as_pubkey(&global.amusd_mint));
    assert_eq!(ix.accounts[5].pubkey, as_pubkey(&global.asol_mint));
    assert_eq!(
        ix.accounts[6].pubkey,
        as_pubkey(&stability.pool_amusd_vault)
    );
    assert_eq!(ix.accounts[7].pubkey, as_pubkey(&stability.pool_asol_vault));
    assert_eq!(ix.accounts[8].pubkey, programs.token_program);
    assert_eq!(ix.accounts[9].pubkey, programs.clock_sysvar);
    assert_eq!(ix.accounts[10].pubkey, oracle_feed);
    assert_eq!(ix.accounts[11].pubkey, stake_pool);
}

#[test]
fn set_stability_withdrawals_paused_builder_uses_pdas_only() {
    let authority = Pubkey::new_unique();

    let envelope = build_set_stability_withdrawals_paused_instruction(
        &SetStabilityWithdrawalsPausedBuildInput {
            authority,
            config: LaminarBuilderConfig::default(),
            withdrawals_paused: true,
        },
    )
    .unwrap();

    let ix = envelope.full_instruction();
    let (global_state, _) = global_state_pda();
    let (stability_pool_state, _) = stability_pool_state_pda();

    assert_eq!(ix.accounts.len(), 4);
    assert_eq!(ix.accounts[0].pubkey, authority);
    assert_eq!(ix.accounts[1].pubkey, global_state);
    assert_eq!(ix.accounts[2].pubkey, stability_pool_state);
    assert_eq!(
        ix.accounts[3].pubkey,
        ProgramAddresses::new(spl_token::id()).clock_sysvar
    );
}
