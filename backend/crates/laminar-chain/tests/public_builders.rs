use anchor_lang::prelude::Pubkey;
use laminar_chain::{
    build_deposit_amusd_instruction, build_mint_amusd_instruction, build_redeem_amusd_instruction,
    build_withdraw_underlying_instruction, global_state_pda, stability_pool_authority_pda,
    stability_pool_state_pda, treasury_token_account, vault_authority_pda, vault_token_account,
    DepositAmusdBuildInput, LaminarBuilderConfig, MintAmusdBuildInput, ProgramAddresses,
    RedeemAmusdBuildInput, WithdrawUnderlyingBuildInput, LAMINAR_PROGRAM_ID,
};
use laminar_core::{
    Address, GlobalStateModel, LstRateBackend, OracleBackend, StabilityPoolSnapshot,
};

fn addr(pubkey: Pubkey) -> Address {
    Address::from(pubkey.to_string())
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

fn sample_stability_pool(global_state_pubkey: Pubkey) -> StabilityPoolSnapshot {
    StabilityPoolSnapshot {
        global_state: addr(global_state_pubkey),
        samusd_mint: addr(Pubkey::new_unique()),
        pool_amusd_vault: addr(Pubkey::new_unique()),
        pool_asol_vault: addr(Pubkey::new_unique()),
        ..StabilityPoolSnapshot::default()
    }
}

#[test]
fn mint_amusd_builder_matches_expected_order_and_appends_remaining_accounts() {
    let global = sample_global_state(OracleBackend::PythPush, LstRateBackend::SanctumStakePool);
    let user = Pubkey::new_unique();
    let user_amusd_account = Pubkey::new_unique();
    let user_lst_account = Pubkey::new_unique();
    let programs = ProgramAddresses::new(spl_token::id());

    let envelope = build_mint_amusd_instruction(&MintAmusdBuildInput {
        global_state: global.clone(),
        user,
        user_amusd_account,
        user_lst_account,
        programs: programs.clone(),
        config: LaminarBuilderConfig::default(),
        lst_amount: 1_000,
        min_amusd_out: 900,
    })
    .unwrap();

    let ix = envelope.full_instruction();
    let (global_state, _) = global_state_pda();
    let (vault_authority, _) = vault_authority_pda();
    let treasury = Pubkey::try_from(global.treasury.as_str()).unwrap();
    let amusd_mint = Pubkey::try_from(global.amusd_mint.as_str()).unwrap();
    let lst_mint = Pubkey::try_from(global.supported_lst_mint.as_str()).unwrap();
    let oracle_feed = Pubkey::try_from(global.pyth_sol_usd_price_account.as_str()).unwrap();
    let stake_pool = Pubkey::try_from(global.lst_stake_pool.as_str()).unwrap();

    assert_eq!(ix.program_id, LAMINAR_PROGRAM_ID);
    assert_eq!(ix.accounts.len(), 16);

    assert_eq!(ix.accounts[0].pubkey, user);
    assert!(ix.accounts[0].is_signer);
    assert!(ix.accounts[0].is_writable);

    assert_eq!(ix.accounts[1].pubkey, global_state);
    assert_eq!(ix.accounts[2].pubkey, amusd_mint);
    assert_eq!(ix.accounts[3].pubkey, user_amusd_account);
    assert_eq!(
        ix.accounts[4].pubkey,
        treasury_token_account(&treasury, &amusd_mint, &programs.token_program)
    );
    assert_eq!(ix.accounts[5].pubkey, treasury);
    assert_eq!(ix.accounts[6].pubkey, user_lst_account);
    assert_eq!(
        ix.accounts[7].pubkey,
        vault_token_account(&lst_mint, &programs.token_program)
    );
    assert_eq!(ix.accounts[8].pubkey, vault_authority);
    assert_eq!(ix.accounts[9].pubkey, lst_mint);
    assert_eq!(ix.accounts[10].pubkey, programs.token_program);
    assert_eq!(ix.accounts[11].pubkey, programs.associated_token_program);
    assert_eq!(ix.accounts[12].pubkey, programs.system_program);
    assert_eq!(ix.accounts[13].pubkey, programs.clock_sysvar);
    assert_eq!(ix.accounts[14].pubkey, oracle_feed);
    assert_eq!(ix.accounts[15].pubkey, stake_pool);
}

#[test]
fn redeem_amusd_builder_includes_stability_pool_accounts() {
    let global = sample_global_state(OracleBackend::Mock, LstRateBackend::Mock);
    let global_state_pubkey = global_state_pda().0;
    let stability = sample_stability_pool(global_state_pubkey);
    let user = Pubkey::new_unique();
    let user_amusd_account = Pubkey::new_unique();
    let user_lst_account = Pubkey::new_unique();
    let programs = ProgramAddresses::new(spl_token::id());

    let envelope = build_redeem_amusd_instruction(&RedeemAmusdBuildInput {
        global_state: global.clone(),
        stability_pool: stability.clone(),
        user,
        user_amusd_account,
        user_lst_account,
        programs: programs.clone(),
        config: LaminarBuilderConfig::default(),
        amusd_amount: 1_000,
        min_lst_out: 500,
    })
    .unwrap();

    let ix = envelope.full_instruction();
    let (stability_pool_state, _) = stability_pool_state_pda();
    let (stability_pool_authority, _) = stability_pool_authority_pda();

    assert_eq!(ix.program_id, LAMINAR_PROGRAM_ID);
    assert_eq!(ix.accounts.len(), 19);

    assert_eq!(ix.accounts[4].pubkey, stability_pool_state);
    assert_eq!(ix.accounts[5].pubkey, stability_pool_authority);
    assert_eq!(
        ix.accounts[6].pubkey,
        Pubkey::try_from(stability.pool_amusd_vault.as_str()).unwrap()
    );
    assert_eq!(
        ix.accounts[7].pubkey,
        Pubkey::try_from(stability.pool_asol_vault.as_str()).unwrap()
    );
    assert_eq!(ix.accounts[8].pubkey, user_amusd_account);
    assert_eq!(ix.accounts[11].pubkey, user_lst_account);
}

#[test]
fn withdraw_underlying_builder_uses_mock_backends_without_remaining_accounts() {
    let global = sample_global_state(OracleBackend::Mock, LstRateBackend::Mock);
    let global_state_pubkey = global_state_pda().0;
    let stability = sample_stability_pool(global_state_pubkey);
    let user = Pubkey::new_unique();
    let user_samusd_account = Pubkey::new_unique();
    let user_amusd_account = Pubkey::new_unique();
    let user_asol_account = Pubkey::new_unique();
    let programs = ProgramAddresses::new(spl_token::id());

    let envelope = build_withdraw_underlying_instruction(&WithdrawUnderlyingBuildInput {
        global_state: global.clone(),
        stability_pool: stability.clone(),
        user,
        user_samusd_account,
        user_amusd_account,
        user_asol_account,
        programs: programs.clone(),
        config: LaminarBuilderConfig::default(),
        samusd_amount: 1_000,
        min_amusd_out: 1,
        min_asol_out: 1,
    })
    .unwrap();

    let ix = envelope.full_instruction();

    assert_eq!(ix.program_id, LAMINAR_PROGRAM_ID);
    assert_eq!(ix.accounts.len(), 16);
    assert_eq!(ix.accounts[0].pubkey, user);
    assert_eq!(ix.accounts[7].pubkey, user_samusd_account);
    assert_eq!(ix.accounts[8].pubkey, user_amusd_account);
    assert_eq!(ix.accounts[9].pubkey, user_asol_account);
    assert_eq!(ix.accounts[12].pubkey, programs.token_program);
    assert_eq!(ix.accounts[13].pubkey, programs.associated_token_program);
    assert_eq!(ix.accounts[14].pubkey, programs.system_program);
    assert_eq!(ix.accounts[15].pubkey, programs.clock_sysvar);
}

#[test]
fn deposit_amusd_builder_uses_stability_pool_snapshot_accounts() {
    let global = sample_global_state(OracleBackend::Mock, LstRateBackend::Mock);
    let global_state_pubkey = global_state_pda().0;
    let stability = sample_stability_pool(global_state_pubkey);
    let user = Pubkey::new_unique();
    let user_amusd_account = Pubkey::new_unique();
    let user_samusd_account = Pubkey::new_unique();

    let envelope = build_deposit_amusd_instruction(&DepositAmusdBuildInput {
        global_state: global,
        stability_pool: stability.clone(),
        user,
        user_amusd_account,
        user_samusd_account,
        programs: ProgramAddresses::new(spl_token::id()),
        config: LaminarBuilderConfig::default(),
        amusd_amount: 1_000,
        min_samusd_out: 900,
    })
    .unwrap();

    let ix = envelope.full_instruction();

    assert_eq!(ix.accounts.len(), 15);
    assert_eq!(ix.accounts[8].pubkey, user_samusd_account);
    assert_eq!(
        ix.accounts[9].pubkey,
        Pubkey::try_from(stability.pool_amusd_vault.as_str()).unwrap()
    );
    assert_eq!(
        ix.accounts[10].pubkey,
        Pubkey::try_from(stability.pool_asol_vault.as_str()).unwrap()
    );
}
