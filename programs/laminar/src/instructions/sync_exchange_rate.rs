//! sync_exchange_rate instruction - referesh cached LST pricing snapshot metadata
//! In current MVP, rate source is mocked in GloabalState.
//! This ensures deterministic ordering: sync first, then pricing.

use anchor_lang::prelude::*;

use crate::{error::LaminarError, state::*};


/// Refresh cached exchange-rate freshness metadata in-place.
/// we need to call this at the top of every price=sensitive instruction before pricing.
pub fn sync_exchange_rate_in_place(
  global_state: &mut GlobalState,
  current_slot: u64,
) -> Result<()> {
  require!(global_state.mock_lst_to_sol_rate > 0, LaminarError::InvalidParameter);

  // Blocks should not move backward
  require!(
    current_slot >= global_state.last_tvl_update_slot,
    LaminarError::InvalidParameter
  );

  global_state.last_tvl_update_slot = current_slot;
  Ok(())
}

pub fn handler(ctx: Context<SyncExchangeRate>) -> Result<()> {
  let global_state = &mut ctx.accounts.global_state;
  global_state.validate_version()?;

  sync_exchange_rate_in_place(global_state, ctx.accounts.clock.slot)?;
  global_state.operation_counter = global_state.operation_counter.saturating_add(1);
  
  msg!(
    "Exchange rate synced at slot {} (mock lst_to_sol_rate={})",
    ctx.accounts.clock.slot,
    global_state.mock_lst_to_sol_rate
  );

  Ok(())
}


#[derive(Accounts)]
pub struct SyncExchangeRate<'info> {
  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    constraint = global_state.to_account_info().owner == &crate::ID @LaminarError::InvalidAccountOwner,
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  pub clock: Sysvar<'info, Clock>,
}