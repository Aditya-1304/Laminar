//! sync_exchange_rate instruction - refresh cached LST pricing snapshot metadata
//!
//! - MOCK backend: advances freshness metadata using cached rate.
//! - SANCTUM_STAKE_POOL backend: reads intrinsic LST->SOL rate from stake-pool state.

use anchor_lang::prelude::*;
use spl_stake_pool::state::{AccountType, StakePool};

use crate::{
  constants::{LST_RATE_BACKEND_MOCK, LST_RATE_BACKEND_SANCTUM_STAKE_POOL, SOL_PRECISION},
  error::LaminarError,
  math::mul_div_down,
  state::*,
};

/// Refresh cached exchange-rate freshness metadata in-place.
///
/// When backend is `LST_RATE_BACKEND_SANCTUM_STAKE_POOL`, caller must pass the configured
/// stake-pool account in `remaining_accounts`.
pub fn sync_exchange_rate_in_place<'info>(
  global_state: &mut GlobalState,
  clock: &Clock,
  remaining_accounts: &[AccountInfo<'info>],
) -> Result<()> {
  match global_state.lst_rate_backend {
    LST_RATE_BACKEND_MOCK => {
      require!(global_state.mock_lst_to_sol_rate > 0, LaminarError::InvalidParameter);
      global_state.last_tvl_update_slot = clock.slot;
      global_state.last_lst_update_epoch = clock.epoch;
      Ok(())
    }

    LST_RATE_BACKEND_SANCTUM_STAKE_POOL => {
      require!(
        global_state.lst_stake_pool != Pubkey::default(),
        LaminarError::LstStakePoolNotSet,
      );

      let stake_pool_ai = find_remaining_account(remaining_accounts, &global_state.lst_stake_pool)
        .ok_or(LaminarError::LstStakePoolAccountMissing)?;

      require!(
        stake_pool_ai.key == &global_state.lst_stake_pool,
        LaminarError::LstStakePoolMismatch
      );
      require!(
        stake_pool_ai.owner == &spl_stake_pool::id(),
        LaminarError::LstStakePoolMismatch
      );
      require!(global_state.max_lst_stale_epochs > 0, LaminarError::InvalidParameter);

      let stake_pool_data = stake_pool_ai
        .try_borrow_data()
        .map_err(|_| LaminarError::LstStateLoadFailed)?;

      // Use borsh v1 explicitly to match spl-stake-pool dependency and avoid anchor borsh ambiguity.
      let stake_pool: StakePool = ::borsh::from_slice(stake_pool_data.as_ref())
        .map_err(|_| LaminarError::LstStateLoadFailed)?;

      require!(
        stake_pool.account_type == AccountType::StakePool,
        LaminarError::LstStateLoadFailed
      );
      require!(
        stake_pool.pool_mint == global_state.supported_lst_mint,
        LaminarError::UnsupportedLST
      );
      require!(
        stake_pool.pool_token_supply > 0,
        LaminarError::LstRateInvalid
      );

      let epoch_age = clock
        .epoch
        .checked_sub(stake_pool.last_update_epoch)
        .ok_or(LaminarError::ArithmeticOverflow)?;

      require!(
        epoch_age <= global_state.max_lst_stale_epochs,
        LaminarError::LstRateStale
      );

      let lst_to_sol_rate = mul_div_down(
        stake_pool.total_lamports,
        SOL_PRECISION,
        stake_pool.pool_token_supply,
      )
      .ok_or(LaminarError::ArithmeticOverflow)?;

      require!(lst_to_sol_rate > 0, LaminarError::LstRateInvalid);

      global_state.mock_lst_to_sol_rate = lst_to_sol_rate;
      global_state.last_tvl_update_slot = clock.slot;
      global_state.last_lst_update_epoch = clock.epoch;

      Ok(())
    }

    _ => err!(LaminarError::UnsupportedLstRateBackend),
  }
}

pub fn handler(ctx: Context<SyncExchangeRate>) -> Result<()> {
  let global_state = &mut ctx.accounts.global_state;
  global_state.validate_version()?;

  sync_exchange_rate_in_place(global_state, &ctx.accounts.clock, ctx.remaining_accounts)?;
  global_state.operation_counter = global_state.operation_counter.saturating_add(1);

  msg!(
    "Exchange rate synced at slot {} (mock lst_to_sol_rate={})",
    ctx.accounts.clock.slot,
    global_state.mock_lst_to_sol_rate
  );

  Ok(())
}

fn find_remaining_account<'info>(
  remaining_accounts: &[AccountInfo<'info>],
  expected_key: &Pubkey,
) -> Option<AccountInfo<'info>> {
  remaining_accounts
    .iter()
    .find(|acc| acc.key == expected_key)
    .cloned()
}

#[derive(Accounts)]
pub struct SyncExchangeRate<'info> {
  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner,
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  pub clock: Sysvar<'info, Clock>,
}
