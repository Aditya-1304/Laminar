use anchor_lang::prelude::program_option::COption;
use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Burn, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
  constants::LST_RATE_BACKEND_MOCK,
  error::LaminarError,
  invariants::assert_lst_snapshot_fresh,
  invariants::assert_not_cpi_context,
  instructions::sync_exchange_rate_in_place,
  math::{
    compute_cr_bps, compute_liability_sol, compute_tvl_sol, mul_div_down, nav_asol_with_reserve,
    BPS_PRECISION, MIN_AMUSD_MINT, SOL_PRECISION,
  },
  oracle::load_oracle_pricing_in_place,
  state::*,
};

fn load_pricing_and_sync<'info>(
  global_state: &mut GlobalState,
  clock: &Clock,
  remaining_accounts: &[AccountInfo<'info>],
) -> Result<crate::oracle::OraclePricing> {
  if global_state.lst_rate_backend == LST_RATE_BACKEND_MOCK {
    assert_lst_snapshot_fresh(
      clock.slot,
      global_state.last_tvl_update_slot,
      global_state.max_oracle_staleness_slots,
    )?;
  }

  sync_exchange_rate_in_place(global_state, clock, remaining_accounts)?;
  load_oracle_pricing_in_place(global_state, clock, remaining_accounts)
}

fn current_asol_nav(global_state: &GlobalState, price_safe_usd: u64) -> Result<u64> {
  let tvl = compute_tvl_sol(global_state.total_lst_amount, global_state.mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let liability = if global_state.amusd_supply > 0 {
    compute_liability_sol(global_state.amusd_supply, price_safe_usd)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  Ok(
    nav_asol_with_reserve(
      tvl,
      liability,
      global_state.rounding_reserve_lamports,
      global_state.asol_supply,
    )
    .unwrap_or(0),
  )
}

fn pool_value_sol(total_amusd: u64, total_asol: u64, price_safe_usd: u64, nav_asol: u64) -> Result<u64> {
  let amusd_component = mul_div_down(total_amusd, SOL_PRECISION, price_safe_usd)
    .ok_or(LaminarError::MathOverflow)?;

  let asol_component = if total_asol == 0 || nav_asol == 0 {
    0
  } else {
    mul_div_down(total_asol, nav_asol, SOL_PRECISION).ok_or(LaminarError::MathOverflow)?
  };

  amusd_component
    .checked_add(asol_component)
    .ok_or(LaminarError::MathOverflow.into())
}

pub fn initialize_stability_pool_handler(ctx: Context<InitializeStabilityPool>) -> Result<()> {
  let global_state = &mut ctx.accounts.global_state;
  global_state.validate_version()?;

  let stability_pool_state = &mut ctx.accounts.stability_pool_state;

  stability_pool_state.version = CURRENT_VERSION;
  stability_pool_state.bump = ctx.bumps.stability_pool_state;
  stability_pool_state.pool_authority_bump = ctx.bumps.stability_pool_authority;
  stability_pool_state.global_state = global_state.key();
  stability_pool_state.samusd_mint = ctx.accounts.samusd_mint.key();
  stability_pool_state.pool_amusd_vault = ctx.accounts.pool_amusd_vault.key();
  stability_pool_state.pool_asol_vault = ctx.accounts.pool_asol_vault.key();
  stability_pool_state.total_amusd = 0;
  stability_pool_state.total_asol = 0;
  stability_pool_state.total_samusd = 0;
  stability_pool_state.withdrawls_paused = false;
  stability_pool_state.last_harvest_lst_to_sol_rate = global_state.mock_lst_to_sol_rate;
  stability_pool_state._reserved = [0; 4];

  global_state.operation_counter = global_state.operation_counter.saturating_add(1);

  emit!(crate::events::StabilityPoolInitialized {
    authority: ctx.accounts.authority.key(),
    samusd_mint: ctx.accounts.samusd_mint.key(),
    pool_amusd_vault: ctx.accounts.pool_amusd_vault.key(),
    pool_asol_vault: ctx.accounts.pool_asol_vault.key(),
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

  Ok(())
}

pub fn deposit_amusd_handler(
  ctx: Context<DepositAmUSD>,
  amusd_amount: u64,
  min_samusd_out: u64,
) -> Result<()> {
  assert_not_cpi_context()?;

  require!(amusd_amount > 0, LaminarError::ZeroAmount);
  require!(min_samusd_out > 0, LaminarError::ZeroAmount);
  require!(amusd_amount >= MIN_AMUSD_MINT, LaminarError::AmountTooSmall);

  {
    let stability = &ctx.accounts.stability_pool_state;
    require!(ctx.accounts.pool_amusd_vault.amount == stability.total_amusd, LaminarError::StabilityPoolStateMismatch);
    require!(ctx.accounts.pool_asol_vault.amount == stability.total_asol, LaminarError::StabilityPoolStateMismatch);
  }

  let pricing = {
    let global_state = &mut ctx.accounts.global_state;
    global_state.validate_version()?;
    load_pricing_and_sync(global_state, &ctx.accounts.clock, ctx.remaining_accounts)?
  };
  ctx.accounts.stability_pool_state.validate_version()?;

  let global_ro = &ctx.accounts.global_state;
  let stability_ro = &ctx.accounts.stability_pool_state;

  let price_safe_usd = pricing.price_safe_usd;
  let nav_asol = current_asol_nav(global_ro, price_safe_usd)?;

  let deposit_value_sol = mul_div_down(amusd_amount, SOL_PRECISION, price_safe_usd)
    .ok_or(LaminarError::MathOverflow)?;
  require!(deposit_value_sol > 0, LaminarError::AmountTooSmall);

  let pool_value_before = pool_value_sol(
    stability_ro.total_amusd,
    stability_ro.total_asol,
    price_safe_usd,
    nav_asol,
  )?;

  let samusd_minted = if stability_ro.total_samusd == 0 || pool_value_before == 0 {
    amusd_amount
  } else {
    mul_div_down(deposit_value_sol, stability_ro.total_samusd, pool_value_before)
      .ok_or(LaminarError::MathOverflow)?
  };

  require!(samusd_minted > 0, LaminarError::AmountTooSmall);
  require!(samusd_minted >= min_samusd_out, LaminarError::SlippageExceeded);

  let new_total_amusd = stability_ro
    .total_amusd
    .checked_add(amusd_amount)
    .ok_or(LaminarError::MathOverflow)?;
  let new_total_samusd = stability_ro
    .total_samusd
    .checked_add(samusd_minted)
    .ok_or(LaminarError::MathOverflow)?;

  {
    let global_state = &mut ctx.accounts.global_state;
    let stability = &mut ctx.accounts.stability_pool_state;
    stability.total_amusd = new_total_amusd;
    stability.total_samusd = new_total_samusd;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
  }

  let transfer_accounts = TransferChecked {
    from: ctx.accounts.user_amusd_account.to_account_info(),
    mint: ctx.accounts.amusd_mint.to_account_info(),
    to: ctx.accounts.pool_amusd_vault.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  token_interface::transfer_checked(
    CpiContext::new(ctx.accounts.token_program.to_account_info(), transfer_accounts),
    amusd_amount,
    ctx.accounts.amusd_mint.decimals,
  )?;

  let sp_seeds = &[
    STABILITY_POOL_AUTHORITY_SEED,
    &[ctx.accounts.stability_pool_state.pool_authority_bump],
  ];
  let sp_signer = &[&sp_seeds[..]];

  let mint_accounts = MintTo {
    mint: ctx.accounts.samusd_mint.to_account_info(),
    to: ctx.accounts.user_samusd_account.to_account_info(),
    authority: ctx.accounts.stability_pool_authority.to_account_info(),
  };

  token_interface::mint_to(
    CpiContext::new_with_signer(
      ctx.accounts.token_program.to_account_info(),
      mint_accounts,
      sp_signer,
    ),
    samusd_minted,
  )?;

  ctx.accounts.pool_amusd_vault.reload()?;
  ctx.accounts.samusd_mint.reload()?;
  require!(
    ctx.accounts.pool_amusd_vault.amount == ctx.accounts.stability_pool_state.total_amusd,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    ctx.accounts.samusd_mint.supply == ctx.accounts.stability_pool_state.total_samusd,
    LaminarError::StabilityPoolStateMismatch
  );

  emit!(crate::events::StabilityPoolDeposited {
    user: ctx.accounts.user.key(),
    amusd_in: amusd_amount,
    samusd_minted,
    total_amusd: ctx.accounts.stability_pool_state.total_amusd,
    total_asol: ctx.accounts.stability_pool_state.total_asol,
    total_samusd: ctx.accounts.stability_pool_state.total_samusd,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

  Ok(())
}

pub fn withdraw_underlying_handler(
  ctx: Context<WithdrawUnderlying>,
  samusd_amount: u64,
  min_amusd_out: u64,
  min_asol_out: u64,
) -> Result<()> {
  assert_not_cpi_context()?;

  require!(samusd_amount > 0, LaminarError::ZeroAmount);

  {
    let stability = &ctx.accounts.stability_pool_state;
    require!(ctx.accounts.pool_amusd_vault.amount == stability.total_amusd, LaminarError::StabilityPoolStateMismatch);
    require!(ctx.accounts.pool_asol_vault.amount == stability.total_asol, LaminarError::StabilityPoolStateMismatch);
  }

  {
    let global_state = &ctx.accounts.global_state;
    global_state.validate_version()?;
  }
  ctx.accounts.stability_pool_state.validate_version()?;

  let stability_ro = &ctx.accounts.stability_pool_state;
  require!(!stability_ro.withdrawls_paused, LaminarError::StabilityPoolWithdrawalsPaused);
  require!(stability_ro.total_samusd > 0, LaminarError::StabilityPoolEmpty);

  require!(
    ctx.accounts.user_samusd_account.amount >= samusd_amount,
    LaminarError::InsufficientSupply
  );

  let amusd_out = mul_div_down(stability_ro.total_amusd, samusd_amount, stability_ro.total_samusd)
    .ok_or(LaminarError::MathOverflow)?;
  let asol_out = mul_div_down(stability_ro.total_asol, samusd_amount, stability_ro.total_samusd)
    .ok_or(LaminarError::MathOverflow)?;

  require!(amusd_out >= min_amusd_out, LaminarError::SlippageExceeded);
  require!(asol_out >= min_asol_out, LaminarError::SlippageExceeded);
  require!(amusd_out > 0 || asol_out > 0, LaminarError::AmountTooSmall);

  let new_total_amusd = stability_ro
    .total_amusd
    .checked_sub(amusd_out)
    .ok_or(LaminarError::MathOverflow)?;
  let new_total_asol = stability_ro
    .total_asol
    .checked_sub(asol_out)
    .ok_or(LaminarError::MathOverflow)?;
  let new_total_samusd = stability_ro
    .total_samusd
    .checked_sub(samusd_amount)
    .ok_or(LaminarError::MathOverflow)?;

  {
    let global_state = &mut ctx.accounts.global_state;
    let stability = &mut ctx.accounts.stability_pool_state;
    stability.total_amusd = new_total_amusd;
    stability.total_asol = new_total_asol;
    stability.total_samusd = new_total_samusd;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
  }

  let burn_accounts = Burn {
    mint: ctx.accounts.samusd_mint.to_account_info(),
    from: ctx.accounts.user_samusd_account.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  token_interface::burn(
    CpiContext::new(ctx.accounts.token_program.to_account_info(), burn_accounts),
    samusd_amount,
  )?;

  let sp_seeds = &[
    STABILITY_POOL_AUTHORITY_SEED,
    &[ctx.accounts.stability_pool_state.pool_authority_bump],
  ];
  let sp_signer = &[&sp_seeds[..]];

  if amusd_out > 0 {
    let transfer_amusd = TransferChecked {
      from: ctx.accounts.pool_amusd_vault.to_account_info(),
      mint: ctx.accounts.amusd_mint.to_account_info(),
      to: ctx.accounts.user_amusd_account.to_account_info(),
      authority: ctx.accounts.stability_pool_authority.to_account_info(),
    };

    token_interface::transfer_checked(
      CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_amusd,
        sp_signer,
      ),
      amusd_out,
      ctx.accounts.amusd_mint.decimals,
    )?;
  }

  if asol_out > 0 {
    let transfer_asol = TransferChecked {
      from: ctx.accounts.pool_asol_vault.to_account_info(),
      mint: ctx.accounts.asol_mint.to_account_info(),
      to: ctx.accounts.user_asol_account.to_account_info(),
      authority: ctx.accounts.stability_pool_authority.to_account_info(),
    };

    token_interface::transfer_checked(
      CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_asol,
        sp_signer,
      ),
      asol_out,
      ctx.accounts.asol_mint.decimals,
    )?;
  }

  ctx.accounts.pool_amusd_vault.reload()?;
  ctx.accounts.pool_asol_vault.reload()?;
  ctx.accounts.samusd_mint.reload()?;
  require!(
    ctx.accounts.pool_amusd_vault.amount == ctx.accounts.stability_pool_state.total_amusd,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    ctx.accounts.pool_asol_vault.amount == ctx.accounts.stability_pool_state.total_asol,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    ctx.accounts.samusd_mint.supply == ctx.accounts.stability_pool_state.total_samusd,
    LaminarError::StabilityPoolStateMismatch
  );

  emit!(crate::events::StabilityPoolWithdrawn {
    user: ctx.accounts.user.key(),
    samusd_burned: samusd_amount,
    amusd_out,
    asol_out,
    total_amusd: ctx.accounts.stability_pool_state.total_amusd,
    total_asol: ctx.accounts.stability_pool_state.total_asol,
    total_samusd: ctx.accounts.stability_pool_state.total_samusd,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

  Ok(())
}

pub fn harvest_yield_handler(ctx: Context<HarvestYield>) -> Result<()> {
  assert_not_cpi_context()?;

  let pricing = {
    let global_state = &mut ctx.accounts.global_state;
    global_state.validate_version()?;
    load_pricing_and_sync(global_state, &ctx.accounts.clock, ctx.remaining_accounts)?
  };
  ctx.accounts.stability_pool_state.validate_version()?;

  require!(
    ctx.accounts.pool_amusd_vault.amount == ctx.accounts.stability_pool_state.total_amusd,
    LaminarError::StabilityPoolStateMismatch
  );

  let price_safe_usd = pricing.price_safe_usd;
  let old_rate = ctx.accounts.stability_pool_state.last_harvest_lst_to_sol_rate;
  let new_rate = ctx.accounts.global_state.mock_lst_to_sol_rate;

  let mut yield_delta_sol = 0u64;
  let mut amusd_minted = 0u64;
  let mut negative_yield = false;

  if old_rate == 0 {
    let global_state = &mut ctx.accounts.global_state;
    let stability = &mut ctx.accounts.stability_pool_state;
    stability.last_harvest_lst_to_sol_rate = new_rate;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
  } else if new_rate <= old_rate {
    let global_state = &mut ctx.accounts.global_state;
    let stability = &mut ctx.accounts.stability_pool_state;
    stability.last_harvest_lst_to_sol_rate = new_rate;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
    negative_yield = new_rate < old_rate;
  } else {
    let rate_delta = new_rate
      .checked_sub(old_rate)
      .ok_or(LaminarError::ArithmeticOverflow)?;

    yield_delta_sol = mul_div_down(
      rate_delta,
      ctx.accounts.global_state.total_lst_amount,
      SOL_PRECISION,
    )
    .ok_or(LaminarError::MathOverflow)?;

    amusd_minted = mul_div_down(yield_delta_sol, price_safe_usd, SOL_PRECISION)
      .ok_or(LaminarError::MathOverflow)?;

    {
      let global_state = &mut ctx.accounts.global_state;
      let stability = &mut ctx.accounts.stability_pool_state;

      stability.last_harvest_lst_to_sol_rate = new_rate;

      if amusd_minted > 0 {
        stability.total_amusd = stability
          .total_amusd
          .checked_add(amusd_minted)
          .ok_or(LaminarError::MathOverflow)?;

        global_state.amusd_supply = global_state
          .amusd_supply
          .checked_add(amusd_minted)
          .ok_or(LaminarError::MathOverflow)?;
      }

      global_state.operation_counter = global_state.operation_counter.saturating_add(1);
    }

    if amusd_minted > 0 {
      let gs_seeds = &[GLOBAL_STATE_SEED, &[ctx.accounts.global_state.bump]];
      let gs_signer = &[&gs_seeds[..]];

      let mint_accounts = MintTo {
        mint: ctx.accounts.amusd_mint.to_account_info(),
        to: ctx.accounts.pool_amusd_vault.to_account_info(),
        authority: ctx.accounts.global_state.to_account_info(),
      };

      token_interface::mint_to(
        CpiContext::new_with_signer(
          ctx.accounts.token_program.to_account_info(),
          mint_accounts,
          gs_signer,
        ),
        amusd_minted,
      )?;
    }
  }

  ctx.accounts.pool_amusd_vault.reload()?;
  ctx.accounts.amusd_mint.reload()?;
  require!(
    ctx.accounts.pool_amusd_vault.amount == ctx.accounts.stability_pool_state.total_amusd,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    ctx.accounts.amusd_mint.supply == ctx.accounts.global_state.amusd_supply,
    LaminarError::BalanceSheetViolation
  );

  emit!(crate::events::StabilityYieldHarvested {
    harvester: ctx.accounts.harvester.key(),
    old_rate,
    new_rate,
    yield_delta_sol,
    amusd_minted,
    negative_yield,
    total_amusd: ctx.accounts.stability_pool_state.total_amusd,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

  Ok(())
}

pub fn execute_debt_equity_swap_handler(ctx: Context<ExecuteDebtEquitySwap>) -> Result<()> {
  assert_not_cpi_context()?;

  let pricing = {
    let global_state = &mut ctx.accounts.global_state;
    global_state.validate_version()?;
    load_pricing_and_sync(global_state, &ctx.accounts.clock, ctx.remaining_accounts)?
  };
  ctx.accounts.stability_pool_state.validate_version()?;

  {
    let stability = &ctx.accounts.stability_pool_state;
    require!(ctx.accounts.pool_amusd_vault.amount == stability.total_amusd, LaminarError::StabilityPoolStateMismatch);
    require!(ctx.accounts.pool_asol_vault.amount == stability.total_asol, LaminarError::StabilityPoolStateMismatch);
  }

  let price_safe_usd = pricing.price_safe_usd;

  let tvl = compute_tvl_sol(
    ctx.accounts.global_state.total_lst_amount,
    ctx.accounts.global_state.mock_lst_to_sol_rate,
  )
  .ok_or(LaminarError::MathOverflow)?;

  let old_liability = if ctx.accounts.global_state.amusd_supply > 0 {
    compute_liability_sol(ctx.accounts.global_state.amusd_supply, price_safe_usd)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let old_cr_bps = compute_cr_bps(tvl, old_liability);
  require!(
    old_cr_bps < ctx.accounts.global_state.min_cr_bps,
    LaminarError::NoConversionNeeded
  );

  let pool_amusd = ctx.accounts.stability_pool_state.total_amusd;
  require!(pool_amusd > 0, LaminarError::StabilityPoolEmpty);

  let l_sol_max = mul_div_down(tvl, BPS_PRECISION, ctx.accounts.global_state.min_cr_bps)
    .ok_or(LaminarError::MathOverflow)?;

  let mut amusd_supply_max = mul_div_down(l_sol_max, price_safe_usd, SOL_PRECISION)
    .ok_or(LaminarError::MathOverflow)?;

  for _ in 0..4 {
    let liab = compute_liability_sol(amusd_supply_max, price_safe_usd).ok_or(LaminarError::MathOverflow)?;
    if liab <= l_sol_max || amusd_supply_max == 0 {
      break;
    }
    amusd_supply_max = amusd_supply_max.saturating_sub(1);
  }

  let burn_needed = ctx
    .accounts
    .global_state
    .amusd_supply
    .saturating_sub(amusd_supply_max);
  require!(burn_needed > 0, LaminarError::NoConversionNeeded);

  let burn_target = burn_needed.min(pool_amusd);

  require!(
    ctx.accounts.global_state.nav_floor_lamports > 0,
    LaminarError::InvalidParameter
  );
  require!(
    ctx.accounts.global_state.max_asol_mint_per_round > 0,
    LaminarError::InvalidParameter
  );

  let nav_pre = current_asol_nav(&ctx.accounts.global_state, price_safe_usd)?;
  let nav_conv = nav_pre.max(ctx.accounts.global_state.nav_floor_lamports);

  let max_sol_by_cap = mul_div_down(
    ctx.accounts.global_state.max_asol_mint_per_round,
    nav_conv,
    SOL_PRECISION,
  )
  .ok_or(LaminarError::MathOverflow)?;
  let max_burn_by_cap = mul_div_down(max_sol_by_cap, price_safe_usd, SOL_PRECISION)
    .ok_or(LaminarError::MathOverflow)?;

  let burn_amount = burn_target.min(max_burn_by_cap);
  require!(burn_amount > 0, LaminarError::ConversionOutputTooSmall);

  let sol_value = mul_div_down(burn_amount, SOL_PRECISION, price_safe_usd)
    .ok_or(LaminarError::MathOverflow)?;
  let asol_minted = mul_div_down(sol_value, SOL_PRECISION, nav_conv)
    .ok_or(LaminarError::MathOverflow)?;

  require!(asol_minted > 0, LaminarError::ConversionOutputTooSmall);
  require!(
    asol_minted <= ctx.accounts.global_state.max_asol_mint_per_round,
    LaminarError::ConversionCapExceeded
  );

  let new_amusd_supply = ctx
    .accounts
    .global_state
    .amusd_supply
    .checked_sub(burn_amount)
    .ok_or(LaminarError::MathOverflow)?;
  let new_asol_supply = ctx
    .accounts
    .global_state
    .asol_supply
    .checked_add(asol_minted)
    .ok_or(LaminarError::MathOverflow)?;

  let new_pool_amusd = ctx
    .accounts
    .stability_pool_state
    .total_amusd
    .checked_sub(burn_amount)
    .ok_or(LaminarError::MathOverflow)?;
  let new_pool_asol = ctx
    .accounts
    .stability_pool_state
    .total_asol
    .checked_add(asol_minted)
    .ok_or(LaminarError::MathOverflow)?;

  let new_liability = if new_amusd_supply > 0 {
    compute_liability_sol(new_amusd_supply, price_safe_usd).ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };
  let new_cr_bps = compute_cr_bps(tvl, new_liability);
  require!(new_cr_bps >= old_cr_bps, LaminarError::NoConversionNeeded);

  {
    let global_state = &mut ctx.accounts.global_state;
    let stability = &mut ctx.accounts.stability_pool_state;

    global_state.amusd_supply = new_amusd_supply;
    global_state.asol_supply = new_asol_supply;
    stability.total_amusd = new_pool_amusd;
    stability.total_asol = new_pool_asol;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
  }

  let sp_seeds = &[
    STABILITY_POOL_AUTHORITY_SEED,
    &[ctx.accounts.stability_pool_state.pool_authority_bump],
  ];
  let sp_signer = &[&sp_seeds[..]];

  let burn_accounts = Burn {
    mint: ctx.accounts.amusd_mint.to_account_info(),
    from: ctx.accounts.pool_amusd_vault.to_account_info(),
    authority: ctx.accounts.stability_pool_authority.to_account_info(),
  };

  token_interface::burn(
    CpiContext::new_with_signer(
      ctx.accounts.token_program.to_account_info(),
      burn_accounts,
      sp_signer,
    ),
    burn_amount,
  )?;

  let gs_seeds = &[GLOBAL_STATE_SEED, &[ctx.accounts.global_state.bump]];
  let gs_signer = &[&gs_seeds[..]];

  let mint_accounts = MintTo {
    mint: ctx.accounts.asol_mint.to_account_info(),
    to: ctx.accounts.pool_asol_vault.to_account_info(),
    authority: ctx.accounts.global_state.to_account_info(),
  };

  token_interface::mint_to(
    CpiContext::new_with_signer(
      ctx.accounts.token_program.to_account_info(),
      mint_accounts,
      gs_signer,
    ),
    asol_minted,
  )?;

  ctx.accounts.pool_amusd_vault.reload()?;
  ctx.accounts.pool_asol_vault.reload()?;
  ctx.accounts.amusd_mint.reload()?;
  ctx.accounts.asol_mint.reload()?;

  require!(
    ctx.accounts.pool_amusd_vault.amount == ctx.accounts.stability_pool_state.total_amusd,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    ctx.accounts.pool_asol_vault.amount == ctx.accounts.stability_pool_state.total_asol,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    ctx.accounts.amusd_mint.supply == ctx.accounts.global_state.amusd_supply,
    LaminarError::BalanceSheetViolation
  );
  require!(
    ctx.accounts.asol_mint.supply == ctx.accounts.global_state.asol_supply,
    LaminarError::BalanceSheetViolation
  );

  emit!(crate::events::DebtEquitySwapExecuted {
    executor: ctx.accounts.executor.key(),
    amusd_burned: burn_amount,
    asol_minted,
    nav_conv,
    cr_before_bps: old_cr_bps,
    cr_after_bps: new_cr_bps,
    price_safe_usd,
    pool_amusd_after: ctx.accounts.stability_pool_state.total_amusd,
    pool_asol_after: ctx.accounts.stability_pool_state.total_asol,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

  Ok(())
}

pub fn set_stability_withdrawals_paused_handler(
  ctx: Context<SetStabilityWithdrawalsPaused>,
  withdrawals_paused: bool,
) -> Result<()> {
  let global_state = &mut ctx.accounts.global_state;
  global_state.validate_version()?;

  let stability = &mut ctx.accounts.stability_pool_state;
  stability.validate_version()?;
  stability.withdrawls_paused = withdrawals_paused;

  global_state.operation_counter = global_state.operation_counter.saturating_add(1);

  Ok(())
}

#[derive(Accounts)]
pub struct InitializeStabilityPool<'info> {
  #[account(mut)]
  pub authority: Signer<'info>,

  #[account(
    mut,
    has_one = authority,
    seeds = [GLOBAL_STATE_SEED],
    bump
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  #[account(
    init,
    payer = authority,
    space = StabilityPoolState::LEN,
    seeds = [STABILITY_POOL_STATE_SEED],
    bump
  )]
  pub stability_pool_state: Box<Account<'info, StabilityPoolState>>,

  /// CHECK: PDA signer authority for pool vaults + s_amUSD minting.
  #[account(
    seeds = [STABILITY_POOL_AUTHORITY_SEED],
    bump
  )]
  pub stability_pool_authority: UncheckedAccount<'info>,

  #[account(
    init,
    payer = authority,
    mint::decimals = 6,
    mint::authority = stability_pool_authority,
    mint::freeze_authority = stability_pool_authority,
    mint::token_program = token_program,
  )]
  pub samusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    init,
    payer = authority,
    associated_token::mint = amusd_mint,
    associated_token::authority = stability_pool_authority,
    associated_token::token_program = token_program
  )]
  pub pool_amusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    init,
    payer = authority,
    associated_token::mint = asol_mint,
    associated_token::authority = stability_pool_authority,
    associated_token::token_program = token_program
  )]
  pub pool_asol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    mut,
    address = global_state.amusd_mint @ LaminarError::InvalidMint
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    address = global_state.asol_mint @ LaminarError::InvalidMint
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,
  pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct DepositAmUSD<'info> {
  #[account(mut)]
  pub user: Signer<'info>,

  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  #[account(
    mut,
    seeds = [STABILITY_POOL_STATE_SEED],
    bump = stability_pool_state.bump,
    has_one = global_state,
    has_one = samusd_mint,
    has_one = pool_amusd_vault,
    has_one = pool_asol_vault
  )]
  pub stability_pool_state: Box<Account<'info, StabilityPoolState>>,

  /// CHECK: PDA signer authority for pool vaults + s_amUSD minting.
  #[account(
    seeds = [STABILITY_POOL_AUTHORITY_SEED],
    bump = stability_pool_state.pool_authority_bump
  )]
  pub stability_pool_authority: UncheckedAccount<'info>,

  #[account(
    mut,
    address = global_state.amusd_mint @ LaminarError::InvalidMint,
    constraint = amusd_mint.mint_authority == COption::Some(global_state.key()) @ LaminarError::InvalidMintAuthority
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    address = global_state.asol_mint @ LaminarError::InvalidMint
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    address = stability_pool_state.samusd_mint @ LaminarError::InvalidMint,
    constraint = samusd_mint.mint_authority == COption::Some(stability_pool_authority.key()) @ LaminarError::InvalidMintAuthority
  )]
  pub samusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    token::mint = amusd_mint,
    token::authority = user,
    constraint = user_amusd_account.close_authority == COption::None @ LaminarError::InvalidAccountState
  )]
  pub user_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = samusd_mint,
    associated_token::authority = user,
    associated_token::token_program = token_program
  )]
  pub user_samusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    mut,
    address = stability_pool_state.pool_amusd_vault @ LaminarError::InvalidAccountState,
    token::mint = amusd_mint,
    token::authority = stability_pool_authority
  )]
  pub pool_amusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    mut,
    address = stability_pool_state.pool_asol_vault @ LaminarError::InvalidAccountState,
    token::mint = asol_mint,
    token::authority = stability_pool_authority
  )]
  pub pool_asol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,
  pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct WithdrawUnderlying<'info> {
  #[account(mut)]
  pub user: Signer<'info>,

  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  #[account(
    mut,
    seeds = [STABILITY_POOL_STATE_SEED],
    bump = stability_pool_state.bump,
    has_one = global_state,
    has_one = samusd_mint,
    has_one = pool_amusd_vault,
    has_one = pool_asol_vault
  )]
  pub stability_pool_state: Box<Account<'info, StabilityPoolState>>,

  /// CHECK: PDA signer authority for pool vaults + s_amUSD minting.
  #[account(
    seeds = [STABILITY_POOL_AUTHORITY_SEED],
    bump = stability_pool_state.pool_authority_bump
  )]
  pub stability_pool_authority: UncheckedAccount<'info>,

  #[account(
    mut,
    address = global_state.amusd_mint @ LaminarError::InvalidMint
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    address = global_state.asol_mint @ LaminarError::InvalidMint
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    address = stability_pool_state.samusd_mint @ LaminarError::InvalidMint
  )]
  pub samusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    token::mint = samusd_mint,
    token::authority = user,
    constraint = user_samusd_account.close_authority == COption::None @ LaminarError::InvalidAccountState
  )]
  pub user_samusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = amusd_mint,
    associated_token::authority = user,
    associated_token::token_program = token_program
  )]
  pub user_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = asol_mint,
    associated_token::authority = user,
    associated_token::token_program = token_program
  )]
  pub user_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    mut,
    address = stability_pool_state.pool_amusd_vault @ LaminarError::InvalidAccountState,
    token::mint = amusd_mint,
    token::authority = stability_pool_authority
  )]
  pub pool_amusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    mut,
    address = stability_pool_state.pool_asol_vault @ LaminarError::InvalidAccountState,
    token::mint = asol_mint,
    token::authority = stability_pool_authority
  )]
  pub pool_asol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,
  pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct HarvestYield<'info> {
  #[account(mut)]
  pub harvester: Signer<'info>,

  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  #[account(
    mut,
    seeds = [STABILITY_POOL_STATE_SEED],
    bump = stability_pool_state.bump,
    has_one = global_state,
    has_one = pool_amusd_vault
  )]
  pub stability_pool_state: Box<Account<'info, StabilityPoolState>>,

  #[account(
    mut,
    address = global_state.amusd_mint @ LaminarError::InvalidMint,
    constraint = amusd_mint.mint_authority == COption::Some(global_state.key()) @ LaminarError::InvalidMintAuthority
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    address = stability_pool_state.pool_amusd_vault @ LaminarError::InvalidAccountState
  )]
  pub pool_amusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ExecuteDebtEquitySwap<'info> {
  #[account(mut)]
  pub executor: Signer<'info>,

  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  #[account(
    mut,
    seeds = [STABILITY_POOL_STATE_SEED],
    bump = stability_pool_state.bump,
    has_one = global_state,
    has_one = pool_amusd_vault,
    has_one = pool_asol_vault
  )]
  pub stability_pool_state: Box<Account<'info, StabilityPoolState>>,

  /// CHECK: PDA signer authority for pool vaults + s_amUSD minting.
  #[account(
    seeds = [STABILITY_POOL_AUTHORITY_SEED],
    bump = stability_pool_state.pool_authority_bump
  )]
  pub stability_pool_authority: UncheckedAccount<'info>,

  #[account(
    mut,
    address = global_state.amusd_mint @ LaminarError::InvalidMint,
    constraint = amusd_mint.mint_authority == COption::Some(global_state.key()) @ LaminarError::InvalidMintAuthority
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    address = global_state.asol_mint @ LaminarError::InvalidMint,
    constraint = asol_mint.mint_authority == COption::Some(global_state.key()) @ LaminarError::InvalidMintAuthority
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  #[account(
    mut,
    address = stability_pool_state.pool_amusd_vault @ LaminarError::InvalidAccountState,
    token::mint = amusd_mint,
    token::authority = stability_pool_authority
  )]
  pub pool_amusd_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  #[account(
    mut,
    address = stability_pool_state.pool_asol_vault @ LaminarError::InvalidAccountState,
    token::mint = asol_mint,
    token::authority = stability_pool_authority
  )]
  pub pool_asol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct SetStabilityWithdrawalsPaused<'info> {
  #[account(mut)]
  pub authority: Signer<'info>,

  #[account(
    mut,
    has_one = authority,
    seeds = [GLOBAL_STATE_SEED],
    bump
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  #[account(
    mut,
    seeds = [STABILITY_POOL_STATE_SEED],
    bump = stability_pool_state.bump,
    has_one = global_state
  )]
  pub stability_pool_state: Box<Account<'info, StabilityPoolState>>,

  pub clock: Sysvar<'info, Clock>,
}
