use anchor_lang::{prelude::Pubkey, AnchorDeserialize};
use laminar_core::{
    build_protocol_snapshot, derive_confidence_bps, normalize_lst_rate_backend,
    normalize_oracle_backend, Address, GlobalStateModel, LaminarProtocolSnapshot, LstRateSnapshot,
    OracleSnapshot, ProjectionMetadata, StabilityPoolSnapshot,
};
use spl_token::solana_program::program_option::COption;
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::{Account as SplTokenAccount, AccountState, Mint as SplMint};
use thiserror::Error;

use crate::rpc::ChainAccount;
use crate::wire::{GlobalStateWire, StabilityPoolStateWire, ID as LAMINAR_PROGRAM_ID};

#[derive(Debug, Error)]
pub enum ChainDecodeError {
    #[error("account {address} has owner {owner}, expected {expected}")]
    InvalidProgramOwner {
        address: Pubkey,
        owner: Pubkey,
        expected: Pubkey,
    },
    #[error("account {address} is too small to contain an Anchor discriminator")]
    AccountTooSmall { address: Pubkey },
    #[error("failed to decode anchor account `{label}` at {address}")]
    AnchorDecodeFailed {
        address: Pubkey,
        label: &'static str,
    },
    #[error("account {address} has unsupported token-program owner {owner}")]
    InvalidTokenProgramOwner { address: Pubkey, owner: Pubkey },
    #[error("failed to decode SPL mint at {0}")]
    MintDecodeFailed(Pubkey),
    #[error("failed to decode SPL token account at {0}")]
    TokenAccountDecodeFailed(Pubkey),
    #[error("invalid oracle snapshot cached in global_state")]
    InvalidOracleSnapshot,
    #[error(transparent)]
    CoreMath(#[from] laminar_core::MathError),
}

pub struct DecodedProtocolAccounts {
    pub global_state_address: Pubkey,
    pub global_state: GlobalStateWire,
    pub stability_pool_state_address: Option<Pubkey>,
    pub stability_pool_state: Option<StabilityPoolStateWire>,
    pub protocol_snapshot: LaminarProtocolSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedMint {
    pub address: Pubkey,
    pub program_owner: Pubkey,
    pub mint_authority: Option<Pubkey>,
    pub supply: u64,
    pub decimals: u8,
    pub is_initialized: bool,
    pub freeze_authority: Option<Pubkey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTokenAccount {
    pub address: Pubkey,
    pub program_owner: Pubkey,
    pub mint: Pubkey,
    pub token_owner: Pubkey,
    pub amount: u64,
    pub delegate: Option<Pubkey>,
    pub close_authority: Option<Pubkey>,
    pub is_native: Option<u64>,
    pub is_frozen: bool,
}

pub fn decode_global_state_account(
    account: &ChainAccount,
) -> Result<GlobalStateWire, ChainDecodeError> {
    ensure_program_owner(account, &LAMINAR_PROGRAM_ID)?;
    decode_anchor_account::<GlobalStateWire>(account, "GlobalState")
}

pub fn decode_stability_pool_state_account(
    account: &ChainAccount,
) -> Result<StabilityPoolStateWire, ChainDecodeError> {
    ensure_program_owner(account, &LAMINAR_PROGRAM_ID)?;
    decode_anchor_account::<StabilityPoolStateWire>(account, "StabilityPoolState")
}

pub fn decode_global_state_model(
    account: &ChainAccount,
) -> Result<GlobalStateModel, ChainDecodeError> {
    let global_state = decode_global_state_account(account)?;
    Ok(global_state_to_model(&global_state))
}

pub fn decode_protocol_accounts(
    global_state_account: &ChainAccount,
    stability_pool_state_account: Option<&ChainAccount>,
    indexed_slot: Option<u64>,
) -> Result<DecodedProtocolAccounts, ChainDecodeError> {
    let global_state = decode_global_state_account(global_state_account)?;
    let stability_pool_state = match stability_pool_state_account {
        Some(account) => Some(decode_stability_pool_state_account(account)?),
        None => None,
    };

    let global_model = global_state_to_model(&global_state);
    let oracle_snapshot = global_state_to_oracle_snapshot(&global_state)?;
    let lst_rate_snapshot = global_state_to_lst_rate_snapshot(&global_state);
    let stability_pool_snapshot = stability_pool_state
        .as_ref()
        .map(stability_pool_state_to_snapshot)
        .unwrap_or_default();

    let protocol_snapshot = build_protocol_snapshot(
        global_model,
        oracle_snapshot,
        lst_rate_snapshot,
        stability_pool_snapshot,
        ProjectionMetadata {
            indexed_slot,
            simulated_slot: None,
        },
    )?;

    Ok(DecodedProtocolAccounts {
        global_state_address: global_state_account.pubkey,
        global_state,
        stability_pool_state_address: stability_pool_state_account.map(|a| a.pubkey),
        stability_pool_state,
        protocol_snapshot,
    })
}

pub fn decode_mint_account(account: &ChainAccount) -> Result<DecodedMint, ChainDecodeError> {
    ensure_token_program_owner(account)?;
    let mint = StateWithExtensions::<SplMint>::unpack(&account.data)
        .map_err(|_| ChainDecodeError::MintDecodeFailed(account.pubkey))?;

    Ok(DecodedMint {
        address: account.pubkey,
        program_owner: account.owner,
        mint_authority: coption_pubkey(mint.base.mint_authority),
        supply: mint.base.supply,
        decimals: mint.base.decimals,
        is_initialized: mint.base.is_initialized,
        freeze_authority: coption_pubkey(mint.base.freeze_authority),
    })
}

pub fn decode_token_account(
    account: &ChainAccount,
) -> Result<DecodedTokenAccount, ChainDecodeError> {
    ensure_token_program_owner(account)?;
    let token_account = StateWithExtensions::<SplTokenAccount>::unpack(&account.data)
        .map_err(|_| ChainDecodeError::TokenAccountDecodeFailed(account.pubkey))?;

    Ok(DecodedTokenAccount {
        address: account.pubkey,
        program_owner: account.owner,
        mint: token_account.base.mint,
        token_owner: token_account.base.owner,
        amount: token_account.base.amount,
        delegate: coption_pubkey(token_account.base.delegate),
        close_authority: coption_pubkey(token_account.base.close_authority),
        is_native: coption_u64(token_account.base.is_native),
        is_frozen: token_account.base.state == AccountState::Frozen,
    })
}

pub fn global_state_to_model(state: &GlobalStateWire) -> GlobalStateModel {
    GlobalStateModel {
        version: state.version,
        bump: state.bump,
        vault_authority_bump: state.vault_authority_bump,
        operation_counter: state.operation_counter,
        authority: address(state.authority),
        amusd_mint: address(state.amusd_mint),
        asol_mint: address(state.asol_mint),
        treasury: address(state.treasury),
        supported_lst_mint: address(state.supported_lst_mint),
        total_lst_amount: state.total_lst_amount,
        amusd_supply: state.amusd_supply,
        asol_supply: state.asol_supply,
        min_cr_bps: state.min_cr_bps,
        target_cr_bps: state.target_cr_bps,
        mint_paused: state.mint_paused,
        redeem_paused: state.redeem_paused,
        mock_sol_price_usd: state.mock_sol_price_usd,
        mock_lst_to_sol_rate: state.mock_lst_to_sol_rate,
        fee_amusd_mint_bps: state.fee_amusd_mint_bps,
        fee_amusd_redeem_bps: state.fee_amusd_redeem_bps,
        fee_asol_mint_bps: state.fee_asol_mint_bps,
        fee_asol_redeem_bps: state.fee_asol_redeem_bps,
        fee_min_multiplier_bps: state.fee_min_multiplier_bps,
        fee_max_multiplier_bps: state.fee_max_multiplier_bps,
        rounding_reserve_lamports: state.rounding_reserve_lamports,
        max_rounding_reserve_lamports: state.max_rounding_reserve_lamports,
        uncertainty_index_bps: state.uncertainty_index_bps,
        flash_loan_utilization_bps: state.flash_loan_utilization_bps,
        flash_outstanding_lamports: state.flash_outstanding_lamports,
        max_oracle_staleness_slots: state.max_oracle_staleness_slots,
        max_conf_bps: state.max_conf_bps,
        uncertainty_max_bps: state.uncertainty_max_bps,
        max_lst_stale_epochs: state.max_lst_stale_epochs,
        nav_floor_lamports: state.nav_floor_lamports,
        max_asol_mint_per_round: state.max_asol_mint_per_round,
        last_tvl_update_slot: state.last_tvl_update_slot,
        last_oracle_update_slot: state.last_oracle_update_slot,
        mock_oracle_confidence_usd: state.mock_oracle_confidence_usd,
        oracle_backend: normalize_oracle_backend(state.oracle_backend),
        lst_rate_backend: normalize_lst_rate_backend(state.lst_rate_backend),
        pyth_sol_usd_price_account: address(state.pyth_sol_usd_price_account),
        lst_stake_pool: address(state.lst_stake_pool),
        last_lst_update_epoch: state.last_lst_update_epoch,
    }
}

pub fn global_state_to_oracle_snapshot(
    state: &GlobalStateWire,
) -> Result<OracleSnapshot, ChainDecodeError> {
    let price_safe_usd = state
        .mock_sol_price_usd
        .checked_sub(state.mock_oracle_confidence_usd)
        .ok_or(ChainDecodeError::InvalidOracleSnapshot)?;

    if price_safe_usd == 0 {
        return Err(ChainDecodeError::InvalidOracleSnapshot);
    }

    let confidence_bps =
        derive_confidence_bps(state.mock_oracle_confidence_usd, state.mock_sol_price_usd)?
            .unwrap_or(0);

    Ok(OracleSnapshot {
        backend: normalize_oracle_backend(state.oracle_backend),
        price_safe_usd,
        price_redeem_usd: state.mock_sol_price_usd,
        price_ema_usd: state.mock_sol_price_usd,
        confidence_usd: state.mock_oracle_confidence_usd,
        confidence_bps,
        uncertainty_index_bps: state.uncertainty_index_bps,
        last_update_slot: state.last_oracle_update_slot,
        max_staleness_slots: state.max_oracle_staleness_slots,
        max_conf_bps: state.max_conf_bps,
    })
}

pub fn global_state_to_lst_rate_snapshot(state: &GlobalStateWire) -> LstRateSnapshot {
    LstRateSnapshot {
        backend: normalize_lst_rate_backend(state.lst_rate_backend),
        supported_lst_mint: address(state.supported_lst_mint),
        lst_to_sol_rate: state.mock_lst_to_sol_rate,
        stake_pool: address(state.lst_stake_pool),
        last_tvl_update_slot: state.last_tvl_update_slot,
        last_lst_update_epoch: state.last_lst_update_epoch,
        max_lst_stale_epochs: state.max_lst_stale_epochs,
    }
}

pub fn stability_pool_state_to_snapshot(state: &StabilityPoolStateWire) -> StabilityPoolSnapshot {
    StabilityPoolSnapshot {
        version: state.version,
        bump: state.bump,
        pool_authority_bump: state.pool_authority_bump,
        global_state: address(state.global_state),
        samusd_mint: address(state.samusd_mint),
        pool_amusd_vault: address(state.pool_amusd_vault),
        pool_asol_vault: address(state.pool_asol_vault),
        total_amusd: state.total_amusd,
        total_asol: state.total_asol,
        total_samusd: state.total_samusd,
        stability_withdrawals_paused: state.withdrawls_paused,
        last_harvest_lst_to_sol_rate: state.last_harvest_lst_to_sol_rate,
    }
}

fn decode_anchor_account<T: AnchorDeserialize>(
    account: &ChainAccount,
    label: &'static str,
) -> Result<T, ChainDecodeError> {
    if account.data.len() < 8 {
        return Err(ChainDecodeError::AccountTooSmall {
            address: account.pubkey,
        });
    }

    let mut data: &[u8] = &account.data[8..];
    T::deserialize(&mut data).map_err(|_| ChainDecodeError::AnchorDecodeFailed {
        address: account.pubkey,
        label,
    })
}

fn ensure_program_owner(
    account: &ChainAccount,
    expected_owner: &Pubkey,
) -> Result<(), ChainDecodeError> {
    if account.owner != *expected_owner {
        return Err(ChainDecodeError::InvalidProgramOwner {
            address: account.pubkey,
            owner: account.owner,
            expected: *expected_owner,
        });
    }

    Ok(())
}

fn ensure_token_program_owner(account: &ChainAccount) -> Result<(), ChainDecodeError> {
    let token_program = spl_token::id();
    let token_2022_program = spl_token_2022::id();

    if account.owner != token_program && account.owner != token_2022_program {
        return Err(ChainDecodeError::InvalidTokenProgramOwner {
            address: account.pubkey,
            owner: account.owner,
        });
    }

    Ok(())
}

fn coption_pubkey(value: COption<Pubkey>) -> Option<Pubkey> {
    match value {
        COption::Some(pubkey) => Some(pubkey),
        COption::None => None,
    }
}

fn coption_u64(value: COption<u64>) -> Option<u64> {
    match value {
        COption::Some(amount) => Some(amount),
        COption::None => None,
    }
}

fn address(pubkey: Pubkey) -> Address {
    Address::from(pubkey.to_string())
}
