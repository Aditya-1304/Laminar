use std::str::FromStr;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use laminar_core::{
    Address, GlobalStateModel, LstRateBackend, OracleBackend, StabilityPoolSnapshot,
};
use thiserror::Error;

use crate::pda::{
    global_state_pda, stability_pool_amusd_vault, stability_pool_asol_vault,
    stability_pool_authority_pda, stability_pool_state_pda, treasury_token_account,
    vault_authority_pda, vault_token_account,
};
use crate::remaining_accounts::{
    resolve_remaining_accounts, RemainingAccountsConfig, RemainingAccountsError,
    ResolvedRemainingAccounts,
};
use crate::wire::{
    build_anchor_instruction_data, build_anchor_instruction_data_no_args, DepositAmusdArgs,
    EmergencyPauseArgs, InitializeArgs, MintAmusdArgs, MintAsolArgs, RedeemAmusdArgs,
    RedeemAsolArgs, SetOracleSourcesArgs, SetStabilityWithdrawalsPausedArgs, UpdateParametersArgs,
    WireError, WithdrawUnderlyingArgs, ID as LAMINAR_PROGRAM_ID,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputeBudgetConfig {
    pub unit_limit: Option<u32>,
    pub unit_price_micro_lamports: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LaminarInstructionEnvelope {
    pub instruction: Instruction,
    pub compute_budget: ComputeBudgetConfig,
    pub remaining_accounts: ResolvedRemainingAccounts,
}

impl LaminarInstructionEnvelope {
    pub fn new(instruction: Instruction, remaining_accounts: ResolvedRemainingAccounts) -> Self {
        Self {
            instruction,
            compute_budget: ComputeBudgetConfig::default(),
            remaining_accounts,
        }
    }

    pub fn with_compute_budget(mut self, compute_budget: ComputeBudgetConfig) -> Self {
        self.compute_budget = compute_budget;
        self
    }

    pub fn full_instruction(&self) -> Instruction {
        extend_instruction_with_remaining_accounts(
            self.instruction.clone(),
            &self.remaining_accounts,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramAddresses {
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
    pub clock_sysvar: Pubkey,
}

impl ProgramAddresses {
    pub fn new(token_program: Pubkey) -> Self {
        Self {
            token_program,
            associated_token_program: spl_associated_token_account::id(),
            system_program: Pubkey::from_str("11111111111111111111111111111111")
                .expect("valid system program id"),
            clock_sysvar: Pubkey::from_str("SysvarC1ock11111111111111111111111111111111")
                .expect("valid clock sysvar id"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LaminarBuilderConfig {
    pub remaining_accounts: RemainingAccountsConfig,
    pub compute_budget: ComputeBudgetConfig,
}

#[derive(Debug, Clone)]
pub struct MintAmusdBuildInput {
    pub global_state: GlobalStateModel,
    pub user: Pubkey,
    pub user_amusd_account: Pubkey,
    pub user_lst_account: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
    pub lst_amount: u64,
    pub min_amusd_out: u64,
}

#[derive(Debug, Clone)]
pub struct RedeemAmusdBuildInput {
    pub global_state: GlobalStateModel,
    pub stability_pool: StabilityPoolSnapshot,
    pub user: Pubkey,
    pub user_amusd_account: Pubkey,
    pub user_lst_account: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
    pub amusd_amount: u64,
    pub min_lst_out: u64,
}

#[derive(Debug, Clone)]
pub struct MintAsolBuildInput {
    pub global_state: GlobalStateModel,
    pub user: Pubkey,
    pub user_asol_account: Pubkey,
    pub user_lst_account: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
    pub lst_amount: u64,
    pub min_asol_out: u64,
}

#[derive(Debug, Clone)]
pub struct RedeemAsolBuildInput {
    pub global_state: GlobalStateModel,
    pub user: Pubkey,
    pub user_asol_account: Pubkey,
    pub user_lst_account: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
    pub asol_amount: u64,
    pub min_lst_out: u64,
}

#[derive(Debug, Clone)]
pub struct DepositAmusdBuildInput {
    pub global_state: GlobalStateModel,
    pub stability_pool: StabilityPoolSnapshot,
    pub user: Pubkey,
    pub user_amusd_account: Pubkey,
    pub user_samusd_account: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
    pub amusd_amount: u64,
    pub min_samusd_out: u64,
}

#[derive(Debug, Clone)]
pub struct WithdrawUnderlyingBuildInput {
    pub global_state: GlobalStateModel,
    pub stability_pool: StabilityPoolSnapshot,
    pub user: Pubkey,
    pub user_samusd_account: Pubkey,
    pub user_amusd_account: Pubkey,
    pub user_asol_account: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
    pub samusd_amount: u64,
    pub min_amusd_out: u64,
    pub min_asol_out: u64,
}

#[derive(Debug, Clone)]
pub struct InitializeBuildInput {
    pub authority: Pubkey,
    pub amusd_mint: Pubkey,
    pub asol_mint: Pubkey,
    pub lst_mint: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
    pub min_cr_bps: u64,
    pub target_cr_bps: u64,
    pub mock_sol_price_usd: u64,
    pub mock_lst_to_sol_rate: u64,
}

#[derive(Debug, Clone)]
pub struct InitializeStabilityPoolBuildInput {
    pub global_state: GlobalStateModel,
    pub authority: Pubkey,
    pub samusd_mint: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
}

#[derive(Debug, Clone)]
pub struct EmergencyPauseBuildInput {
    pub authority: Pubkey,
    pub config: LaminarBuilderConfig,
    pub mint_paused: bool,
    pub redeem_paused: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateParametersBuildInput {
    pub authority: Pubkey,
    pub config: LaminarBuilderConfig,
    pub new_min_cr_bps: u64,
    pub new_target_cr_bps: u64,
}

#[derive(Debug, Clone)]
pub struct SetOracleSourcesBuildInput {
    pub authority: Pubkey,
    pub config: LaminarBuilderConfig,
    pub oracle_backend: u8,
    pub pyth_sol_usd_price_account: Pubkey,
    pub lst_rate_backend: u8,
    pub lst_stake_pool: Pubkey,
}

#[derive(Debug, Clone)]
pub struct SyncExchangeRateBuildInput {
    pub global_state: GlobalStateModel,
    pub config: LaminarBuilderConfig,
}

#[derive(Debug, Clone)]
pub struct UpdateUncertaintyIndexBuildInput {
    pub global_state: GlobalStateModel,
    pub updater: Pubkey,
    pub config: LaminarBuilderConfig,
}

#[derive(Debug, Clone)]
pub struct HarvestYieldBuildInput {
    pub global_state: GlobalStateModel,
    pub stability_pool: StabilityPoolSnapshot,
    pub harvester: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
}

#[derive(Debug, Clone)]
pub struct ExecuteDebtEquitySwapBuildInput {
    pub global_state: GlobalStateModel,
    pub stability_pool: StabilityPoolSnapshot,
    pub executor: Pubkey,
    pub programs: ProgramAddresses,
    pub config: LaminarBuilderConfig,
}

#[derive(Debug, Clone)]
pub struct SetStabilityWithdrawalsPausedBuildInput {
    pub authority: Pubkey,
    pub config: LaminarBuilderConfig,
    pub withdrawals_paused: bool,
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("invalid pubkey in field `{field}`: {value}")]
    InvalidPubkey { field: &'static str, value: String },
    #[error("snapshot field `{field}` does not match derived canonical address: expected {expected}, got {actual}")]
    SnapshotMismatch {
        field: &'static str,
        expected: Pubkey,
        actual: Pubkey,
    },
    #[error(transparent)]
    RemainingAccounts(#[from] RemainingAccountsError),
    #[error(transparent)]
    Wire(#[from] WireError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedGlobalAddresses {
    global_state: Pubkey,
    treasury: Pubkey,
    amusd_mint: Pubkey,
    asol_mint: Pubkey,
    lst_mint: Pubkey,
    vault_authority: Pubkey,
    vault: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedStabilityPoolAddresses {
    stability_pool_state: Pubkey,
    stability_pool_authority: Pubkey,
    samusd_mint: Pubkey,
    pool_amusd_vault: Pubkey,
    pool_asol_vault: Pubkey,
}

pub fn build_initialize_instruction(
    input: &InitializeBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let (global_state, _) = global_state_pda();
    let (vault_authority, _) = vault_authority_pda();
    let vault = vault_token_account(&input.lst_mint, &input.programs.token_program);

    let accounts = vec![
        AccountMeta::new(input.authority, true),
        AccountMeta::new(global_state, false),
        AccountMeta::new(input.amusd_mint, false),
        AccountMeta::new(input.asol_mint, false),
        AccountMeta::new(vault, false),
        AccountMeta::new_readonly(input.lst_mint, false),
        AccountMeta::new_readonly(vault_authority, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "initialize",
        &InitializeArgs {
            min_cr_bps: input.min_cr_bps,
            target_cr_bps: input.target_cr_bps,
            mock_sol_price_usd: input.mock_sol_price_usd,
            mock_lst_to_sol_rate: input.mock_lst_to_sol_rate,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        ResolvedRemainingAccounts::default(),
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_initialize_stability_pool_instruction(
    input: &InitializeStabilityPoolBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let (stability_pool_state, _) = stability_pool_state_pda();
    let (stability_pool_authority, _) = stability_pool_authority_pda();

    let pool_amusd_vault =
        stability_pool_amusd_vault(&global.amusd_mint, &input.programs.token_program);
    let pool_asol_vault =
        stability_pool_asol_vault(&global.asol_mint, &input.programs.token_program);

    let accounts = vec![
        AccountMeta::new(input.authority, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(stability_pool_state, false),
        AccountMeta::new_readonly(stability_pool_authority, false),
        AccountMeta::new(input.samusd_mint, false),
        AccountMeta::new(pool_amusd_vault, false),
        AccountMeta::new(pool_asol_vault, false),
        AccountMeta::new(global.amusd_mint, false),
        AccountMeta::new(global.asol_mint, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data_no_args("initialize_stability_pool");

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        ResolvedRemainingAccounts::default(),
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_emergency_pause_instruction(
    input: &EmergencyPauseBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let (global_state, _) = global_state_pda();

    let accounts = vec![
        AccountMeta::new(input.authority, true),
        AccountMeta::new(global_state, false),
        AccountMeta::new_readonly(ProgramAddresses::new(spl_token::id()).clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "emergency_pause",
        &EmergencyPauseArgs {
            mint_paused: input.mint_paused,
            redeem_paused: input.redeem_paused,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        ResolvedRemainingAccounts::default(),
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_update_parameters_instruction(
    input: &UpdateParametersBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let (global_state, _) = global_state_pda();

    let accounts = vec![
        AccountMeta::new(input.authority, true),
        AccountMeta::new(global_state, false),
        AccountMeta::new_readonly(ProgramAddresses::new(spl_token::id()).clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "update_parameters",
        &UpdateParametersArgs {
            new_min_cr_bps: input.new_min_cr_bps,
            new_target_cr_bps: input.new_target_cr_bps,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        ResolvedRemainingAccounts::default(),
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_set_oracle_sources_instruction(
    input: &SetOracleSourcesBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let (global_state, _) = global_state_pda();

    let accounts = vec![
        AccountMeta::new(input.authority, true),
        AccountMeta::new(global_state, false),
        AccountMeta::new_readonly(ProgramAddresses::new(spl_token::id()).clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "set_oracle_sources",
        &SetOracleSourcesArgs {
            oracle_backend: input.oracle_backend,
            pyth_sol_usd_price_account: input.pyth_sol_usd_price_account,
            lst_rate_backend: input.lst_rate_backend,
            lst_stake_pool: input.lst_stake_pool,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        ResolvedRemainingAccounts::default(),
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_sync_exchange_rate_instruction(
    input: &SyncExchangeRateBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let (global_state, _) = global_state_pda();
    let remaining_accounts =
        resolve_lst_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(global_state, false),
        AccountMeta::new_readonly(ProgramAddresses::new(spl_token::id()).clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data_no_args("sync_exchange_rate");

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_update_uncertainty_index_instruction(
    input: &UpdateUncertaintyIndexBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let (global_state, _) = global_state_pda();
    let remaining_accounts =
        resolve_oracle_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.updater, true),
        AccountMeta::new(global_state, false),
        AccountMeta::new_readonly(ProgramAddresses::new(spl_token::id()).clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data_no_args("update_uncertainty_index");

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_harvest_yield_instruction(
    input: &HarvestYieldBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let stability = resolve_stability_pool_addresses(&input.stability_pool, global.global_state)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.harvester, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(stability.stability_pool_state, false),
        AccountMeta::new(global.amusd_mint, false),
        AccountMeta::new(stability.pool_amusd_vault, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data_no_args("harvest_yield");

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_execute_debt_equity_swap_instruction(
    input: &ExecuteDebtEquitySwapBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let stability = resolve_stability_pool_addresses(&input.stability_pool, global.global_state)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.executor, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(stability.stability_pool_state, false),
        AccountMeta::new_readonly(stability.stability_pool_authority, false),
        AccountMeta::new(global.amusd_mint, false),
        AccountMeta::new(global.asol_mint, false),
        AccountMeta::new(stability.pool_amusd_vault, false),
        AccountMeta::new(stability.pool_asol_vault, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data_no_args("execute_debt_equity_swap");

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_set_stability_withdrawals_paused_instruction(
    input: &SetStabilityWithdrawalsPausedBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let (global_state, _) = global_state_pda();
    let (stability_pool_state, _) = stability_pool_state_pda();

    let accounts = vec![
        AccountMeta::new(input.authority, true),
        AccountMeta::new(global_state, false),
        AccountMeta::new(stability_pool_state, false),
        AccountMeta::new_readonly(ProgramAddresses::new(spl_token::id()).clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "set_stability_withdrawals_paused",
        &SetStabilityWithdrawalsPausedArgs {
            withdrawals_paused: input.withdrawals_paused,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        ResolvedRemainingAccounts::default(),
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_mint_amusd_instruction(
    input: &MintAmusdBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.user, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(global.amusd_mint, false),
        AccountMeta::new(input.user_amusd_account, false),
        AccountMeta::new(
            treasury_token_account(
                &global.treasury,
                &global.amusd_mint,
                &input.programs.token_program,
            ),
            false,
        ),
        AccountMeta::new_readonly(global.treasury, false),
        AccountMeta::new(input.user_lst_account, false),
        AccountMeta::new(global.vault, false),
        AccountMeta::new_readonly(global.vault_authority, false),
        AccountMeta::new_readonly(global.lst_mint, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "mint_amusd",
        &MintAmusdArgs {
            lst_amount: input.lst_amount,
            min_amusd_out: input.min_amusd_out,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_redeem_amusd_instruction(
    input: &RedeemAmusdBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let stability = resolve_stability_pool_addresses(&input.stability_pool, global.global_state)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.user, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(global.amusd_mint, false),
        AccountMeta::new(global.asol_mint, false),
        AccountMeta::new(stability.stability_pool_state, false),
        AccountMeta::new_readonly(stability.stability_pool_authority, false),
        AccountMeta::new(stability.pool_amusd_vault, false),
        AccountMeta::new(stability.pool_asol_vault, false),
        AccountMeta::new(input.user_amusd_account, false),
        AccountMeta::new_readonly(global.treasury, false),
        AccountMeta::new(
            treasury_token_account(
                &global.treasury,
                &global.amusd_mint,
                &input.programs.token_program,
            ),
            false,
        ),
        AccountMeta::new(input.user_lst_account, false),
        AccountMeta::new(global.vault, false),
        AccountMeta::new_readonly(global.vault_authority, false),
        AccountMeta::new_readonly(global.lst_mint, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "redeem_amusd",
        &RedeemAmusdArgs {
            amusd_amount: input.amusd_amount,
            min_lst_out: input.min_lst_out,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_mint_asol_instruction(
    input: &MintAsolBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.user, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(global.asol_mint, false),
        AccountMeta::new(input.user_asol_account, false),
        AccountMeta::new(
            treasury_token_account(
                &global.treasury,
                &global.asol_mint,
                &input.programs.token_program,
            ),
            false,
        ),
        AccountMeta::new_readonly(global.treasury, false),
        AccountMeta::new(input.user_lst_account, false),
        AccountMeta::new(global.vault, false),
        AccountMeta::new_readonly(global.vault_authority, false),
        AccountMeta::new_readonly(global.lst_mint, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "mint_asol",
        &MintAsolArgs {
            lst_amount: input.lst_amount,
            min_asol_out: input.min_asol_out,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_redeem_asol_instruction(
    input: &RedeemAsolBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.user, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(global.asol_mint, false),
        AccountMeta::new(input.user_asol_account, false),
        AccountMeta::new_readonly(global.treasury, false),
        AccountMeta::new(
            treasury_token_account(
                &global.treasury,
                &global.asol_mint,
                &input.programs.token_program,
            ),
            false,
        ),
        AccountMeta::new(input.user_lst_account, false),
        AccountMeta::new(global.vault, false),
        AccountMeta::new_readonly(global.vault_authority, false),
        AccountMeta::new_readonly(global.lst_mint, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "redeem_asol",
        &RedeemAsolArgs {
            asol_amount: input.asol_amount,
            min_lst_out: input.min_lst_out,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_deposit_amusd_instruction(
    input: &DepositAmusdBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let stability = resolve_stability_pool_addresses(&input.stability_pool, global.global_state)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.user, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(stability.stability_pool_state, false),
        AccountMeta::new_readonly(stability.stability_pool_authority, false),
        AccountMeta::new(global.amusd_mint, false),
        AccountMeta::new_readonly(global.asol_mint, false),
        AccountMeta::new(stability.samusd_mint, false),
        AccountMeta::new(input.user_amusd_account, false),
        AccountMeta::new(input.user_samusd_account, false),
        AccountMeta::new(stability.pool_amusd_vault, false),
        AccountMeta::new(stability.pool_asol_vault, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "deposit_amusd",
        &DepositAmusdArgs {
            amusd_amount: input.amusd_amount,
            min_samusd_out: input.min_samusd_out,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn build_withdraw_underlying_instruction(
    input: &WithdrawUnderlyingBuildInput,
) -> Result<LaminarInstructionEnvelope, BuildError> {
    let global = resolve_global_addresses(&input.global_state, input.programs.token_program)?;
    let stability = resolve_stability_pool_addresses(&input.stability_pool, global.global_state)?;
    let remaining_accounts =
        resolve_pricing_remaining_accounts_for_global_state(&input.global_state, &input.config)?;

    let accounts = vec![
        AccountMeta::new(input.user, true),
        AccountMeta::new(global.global_state, false),
        AccountMeta::new(stability.stability_pool_state, false),
        AccountMeta::new_readonly(stability.stability_pool_authority, false),
        AccountMeta::new(global.amusd_mint, false),
        AccountMeta::new(global.asol_mint, false),
        AccountMeta::new(stability.samusd_mint, false),
        AccountMeta::new(input.user_samusd_account, false),
        AccountMeta::new(input.user_amusd_account, false),
        AccountMeta::new(input.user_asol_account, false),
        AccountMeta::new(stability.pool_amusd_vault, false),
        AccountMeta::new(stability.pool_asol_vault, false),
        AccountMeta::new_readonly(input.programs.token_program, false),
        AccountMeta::new_readonly(input.programs.associated_token_program, false),
        AccountMeta::new_readonly(input.programs.system_program, false),
        AccountMeta::new_readonly(input.programs.clock_sysvar, false),
    ];

    let data = build_anchor_instruction_data(
        "withdraw_underlying",
        &WithdrawUnderlyingArgs {
            samusd_amount: input.samusd_amount,
            min_amusd_out: input.min_amusd_out,
            min_asol_out: input.min_asol_out,
        },
    )?;

    Ok(LaminarInstructionEnvelope::new(
        Instruction {
            program_id: LAMINAR_PROGRAM_ID,
            accounts,
            data,
        },
        remaining_accounts,
    )
    .with_compute_budget(input.config.compute_budget))
}

pub fn extend_instruction_with_remaining_accounts(
    mut instruction: Instruction,
    resolved: &ResolvedRemainingAccounts,
) -> Instruction {
    instruction.accounts = merge_account_metas(instruction.accounts, &resolved.account_metas);
    instruction
}

pub fn merge_account_metas(mut base: Vec<AccountMeta>, extras: &[AccountMeta]) -> Vec<AccountMeta> {
    for extra in extras {
        if let Some(existing) = base.iter_mut().find(|meta| meta.pubkey == extra.pubkey) {
            existing.is_signer |= extra.is_signer;
            existing.is_writable |= extra.is_writable;
        } else {
            base.push(extra.clone());
        }
    }

    base
}

fn resolve_oracle_remaining_accounts_for_global_state(
    global_state: &GlobalStateModel,
    config: &LaminarBuilderConfig,
) -> Result<ResolvedRemainingAccounts, BuildError> {
    resolve_specific_remaining_accounts_for_global_state(
        global_state,
        &config.remaining_accounts,
        global_state.oracle_backend,
        LstRateBackend::Mock,
    )
}

fn resolve_lst_remaining_accounts_for_global_state(
    global_state: &GlobalStateModel,
    config: &LaminarBuilderConfig,
) -> Result<ResolvedRemainingAccounts, BuildError> {
    resolve_specific_remaining_accounts_for_global_state(
        global_state,
        &config.remaining_accounts,
        OracleBackend::Mock,
        global_state.lst_rate_backend,
    )
}

fn resolve_pricing_remaining_accounts_for_global_state(
    global_state: &GlobalStateModel,
    config: &LaminarBuilderConfig,
) -> Result<ResolvedRemainingAccounts, BuildError> {
    resolve_specific_remaining_accounts_for_global_state(
        global_state,
        &config.remaining_accounts,
        global_state.oracle_backend,
        global_state.lst_rate_backend,
    )
}

fn resolve_specific_remaining_accounts_for_global_state(
    global_state: &GlobalStateModel,
    fallback: &RemainingAccountsConfig,
    oracle_backend: OracleBackend,
    lst_rate_backend: LstRateBackend,
) -> Result<ResolvedRemainingAccounts, BuildError> {
    let resolved_config = remaining_accounts_config_from_global_state(global_state, fallback)?;
    Ok(resolve_remaining_accounts(
        oracle_backend,
        lst_rate_backend,
        &resolved_config,
    )?)
}

fn remaining_accounts_config_from_global_state(
    global_state: &GlobalStateModel,
    fallback: &RemainingAccountsConfig,
) -> Result<RemainingAccountsConfig, BuildError> {
    Ok(RemainingAccountsConfig {
        pyth_sol_usd_price_account: parse_optional_address(
            "global_state.pyth_sol_usd_price_account",
            &global_state.pyth_sol_usd_price_account,
        )?
        .or(fallback.pyth_sol_usd_price_account),
        lst_stake_pool: parse_optional_address(
            "global_state.lst_stake_pool",
            &global_state.lst_stake_pool,
        )?
        .or(fallback.lst_stake_pool),
    })
}

fn parse_optional_address(
    field: &'static str,
    value: &Address,
) -> Result<Option<Pubkey>, BuildError> {
    if value.as_str().is_empty() {
        return Ok(None);
    }

    let pubkey = parse_address(field, value)?;
    if pubkey == Pubkey::default() {
        Ok(None)
    } else {
        Ok(Some(pubkey))
    }
}

fn resolve_global_addresses(
    global_state: &GlobalStateModel,
    token_program: Pubkey,
) -> Result<ParsedGlobalAddresses, BuildError> {
    let (global_state_pda, _) = global_state_pda();
    let (vault_authority, _) = vault_authority_pda();

    let treasury = parse_address("global_state.treasury", &global_state.treasury)?;
    let amusd_mint = parse_address("global_state.amusd_mint", &global_state.amusd_mint)?;
    let asol_mint = parse_address("global_state.asol_mint", &global_state.asol_mint)?;
    let lst_mint = parse_address(
        "global_state.supported_lst_mint",
        &global_state.supported_lst_mint,
    )?;

    Ok(ParsedGlobalAddresses {
        global_state: global_state_pda,
        treasury,
        amusd_mint,
        asol_mint,
        lst_mint,
        vault_authority,
        vault: vault_token_account(&lst_mint, &token_program),
    })
}

fn resolve_stability_pool_addresses(
    stability_pool: &StabilityPoolSnapshot,
    expected_global_state: Pubkey,
) -> Result<ParsedStabilityPoolAddresses, BuildError> {
    let (stability_pool_state, _) = stability_pool_state_pda();
    let (stability_pool_authority, _) = crate::pda::stability_pool_authority_pda();

    let snapshot_global_state =
        parse_address("stability_pool.global_state", &stability_pool.global_state)?;
    if snapshot_global_state != expected_global_state {
        return Err(BuildError::SnapshotMismatch {
            field: "stability_pool.global_state",
            expected: expected_global_state,
            actual: snapshot_global_state,
        });
    }

    Ok(ParsedStabilityPoolAddresses {
        stability_pool_state,
        stability_pool_authority,
        samusd_mint: parse_address("stability_pool.samusd_mint", &stability_pool.samusd_mint)?,
        pool_amusd_vault: parse_address(
            "stability_pool.pool_amusd_vault",
            &stability_pool.pool_amusd_vault,
        )?,
        pool_asol_vault: parse_address(
            "stability_pool.pool_asol_vault",
            &stability_pool.pool_asol_vault,
        )?,
    })
}

fn parse_address(field: &'static str, value: &Address) -> Result<Pubkey, BuildError> {
    Pubkey::from_str(value.as_str()).map_err(|_| BuildError::InvalidPubkey {
        field,
        value: value.as_str().to_owned(),
    })
}
