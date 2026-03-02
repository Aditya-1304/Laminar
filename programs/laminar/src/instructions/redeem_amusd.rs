//! Redeem amUSD instruction - exits stable debt position
//! User burns amUSD and receives LST collateral back
use anchor_lang::prelude::*;
use anchor_lang::prelude::program_option::COption;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Burn, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
  constants::{LST_RATE_BACKEND_MOCK, MIN_PROTOCOL_TVL},
  error::LaminarError,
  events::AmUSDRedeemed,
  instructions::sync_exchange_rate_in_place,
  state::*,
};
use crate::invariants::*;
use crate::math::*;
use crate::oracle::load_oracle_pricing_in_place;

const MAX_DRAWDOWN_ROUNDS_PER_REDEEM: u8 = 8;

/// Executes one Stability Pool conversion round when CR < min.
///
/// Returns:
/// - `Ok(true)` if a round executed.
/// - `Ok(false)` if no round was needed/possible.
fn execute_drawdown_round_if_needed<'info>(
  global_state: &mut Account<'info, GlobalState>,
  stability_pool_state: &mut Account<'info, StabilityPoolState>,
  amusd_mint: &mut InterfaceAccount<'info, Mint>,
  asol_mint: &mut InterfaceAccount<'info, Mint>,
  pool_amusd_vault: &mut InterfaceAccount<'info, TokenAccount>,
  pool_asol_vault: &mut InterfaceAccount<'info, TokenAccount>,
  stability_pool_authority: &UncheckedAccount<'info>,
  token_program: &Interface<'info, TokenInterface>,
  price_safe_usd: u64,
) -> Result<bool> {
  require!(price_safe_usd > 0, LaminarError::InvalidParameter);

  require!(
    stability_pool_state.global_state == global_state.key(),
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    global_state.amusd_mint == amusd_mint.key(),
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    global_state.asol_mint == asol_mint.key(),
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    stability_pool_state.pool_amusd_vault == pool_amusd_vault.key(),
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    stability_pool_state.pool_asol_vault == pool_asol_vault.key(),
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    pool_amusd_vault.amount == stability_pool_state.total_amusd,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    pool_asol_vault.amount == stability_pool_state.total_asol,
    LaminarError::StabilityPoolStateMismatch
  );

  let tvl = compute_tvl_sol(global_state.total_lst_amount, global_state.mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let old_liability = if global_state.amusd_supply > 0 {
    compute_liability_sol(global_state.amusd_supply, price_safe_usd)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let old_cr_bps = compute_cr_bps(tvl, old_liability);
  if old_cr_bps >= global_state.min_cr_bps {
    return Ok(false);
  }

  let pool_amusd = stability_pool_state.total_amusd;
  if pool_amusd == 0 {
    return Ok(false);
  }

  require!(
    global_state.nav_floor_lamports > 0,
    LaminarError::InvalidParameter
  );
  require!(
    global_state.max_asol_mint_per_round > 0,
    LaminarError::InvalidParameter
  );

  let l_sol_max = mul_div_down(tvl, BPS_PRECISION, global_state.min_cr_bps)
    .ok_or(LaminarError::MathOverflow)?;

  let mut amusd_supply_max = mul_div_down(l_sol_max, price_safe_usd, SOL_PRECISION)
    .ok_or(LaminarError::MathOverflow)?;

  for _ in 0..4 {
    let liab = compute_liability_sol(amusd_supply_max, price_safe_usd)
      .ok_or(LaminarError::MathOverflow)?;
    if liab <= l_sol_max || amusd_supply_max == 0 {
      break;
    }
    amusd_supply_max = amusd_supply_max.saturating_sub(1);
  }

  let burn_needed = global_state.amusd_supply.saturating_sub(amusd_supply_max);
  if burn_needed == 0 {
    return Ok(false);
  }

  let burn_target = burn_needed.min(pool_amusd);

  let nav_pre = nav_asol_with_reserve(
    tvl,
    old_liability,
    global_state.rounding_reserve_lamports,
    global_state.asol_supply,
  )
  .unwrap_or(0);
  let nav_conv = nav_pre.max(global_state.nav_floor_lamports);

  let max_sol_by_cap = mul_div_down(global_state.max_asol_mint_per_round, nav_conv, SOL_PRECISION)
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
    asol_minted <= global_state.max_asol_mint_per_round,
    LaminarError::ConversionCapExceeded
  );

  global_state.amusd_supply = global_state
    .amusd_supply
    .checked_sub(burn_amount)
    .ok_or(LaminarError::MathOverflow)?;
  global_state.asol_supply = global_state
    .asol_supply
    .checked_add(asol_minted)
    .ok_or(LaminarError::MathOverflow)?;

  stability_pool_state.total_amusd = stability_pool_state
    .total_amusd
    .checked_sub(burn_amount)
    .ok_or(LaminarError::MathOverflow)?;
  stability_pool_state.total_asol = stability_pool_state
    .total_asol
    .checked_add(asol_minted)
    .ok_or(LaminarError::MathOverflow)?;

  global_state.operation_counter = global_state.operation_counter.saturating_add(1);

  let sp_seeds = &[
    STABILITY_POOL_AUTHORITY_SEED,
    &[stability_pool_state.pool_authority_bump],
  ];
  let sp_signer = &[&sp_seeds[..]];

  token_interface::burn(
    CpiContext::new_with_signer(
      token_program.to_account_info(),
      Burn {
        mint: amusd_mint.to_account_info(),
        from: pool_amusd_vault.to_account_info(),
        authority: stability_pool_authority.to_account_info(),
      },
      sp_signer,
    ),
    burn_amount,
  )?;

  let gs_seeds = &[GLOBAL_STATE_SEED, &[global_state.bump]];
  let gs_signer = &[&gs_seeds[..]];

  token_interface::mint_to(
    CpiContext::new_with_signer(
      token_program.to_account_info(),
      MintTo {
        mint: asol_mint.to_account_info(),
        to: pool_asol_vault.to_account_info(),
        authority: global_state.to_account_info(),
      },
      gs_signer,
    ),
    asol_minted,
  )?;

  pool_amusd_vault.reload()?;
  pool_asol_vault.reload()?;
  amusd_mint.reload()?;
  asol_mint.reload()?;

  require!(
    pool_amusd_vault.amount == stability_pool_state.total_amusd,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    pool_asol_vault.amount == stability_pool_state.total_asol,
    LaminarError::StabilityPoolStateMismatch
  );
  require!(
    amusd_mint.supply == global_state.amusd_supply,
    LaminarError::BalanceSheetViolation
  );
  require!(
    asol_mint.supply == global_state.asol_supply,
    LaminarError::BalanceSheetViolation
  );

  Ok(true)
}

pub fn handler(ctx: Context<RedeemAmUSD>, amusd_amount: u64, min_lst_out: u64) -> Result<()> {
  assert_not_cpi_context()?;

  // Sync first.
  let pricing = {
    let global_state = &mut ctx.accounts.global_state;
    global_state.validate_version()?;

    if global_state.lst_rate_backend == LST_RATE_BACKEND_MOCK {
      assert_lst_snapshot_fresh(
        ctx.accounts.clock.slot,
        global_state.last_tvl_update_slot,
        global_state.max_oracle_staleness_slots,
      )?;
    }

    sync_exchange_rate_in_place(global_state, &ctx.accounts.clock, ctx.remaining_accounts)?;
    load_oracle_pricing_in_place(global_state, &ctx.accounts.clock, ctx.remaining_accounts)?
  };

  {
    let gs = &ctx.accounts.global_state;
    assert_oracle_freshness_and_confidence(
      ctx.accounts.clock.slot,
      gs.last_oracle_update_slot,
      gs.max_oracle_staleness_slots,
      gs.mock_sol_price_usd,
      gs.mock_oracle_confidence_usd,
      gs.max_conf_bps,
    )?;
  }

  ctx.accounts.stability_pool_state.validate_version()?;

  // Validate immutable user inputs before any drawdown side-effects.
  {
    let gs = &ctx.accounts.global_state;
    require!(!gs.redeem_paused, LaminarError::RedeemPaused);
  }
  require!(amusd_amount > 0, LaminarError::ZeroAmount);
  require!(min_lst_out > 0, LaminarError::ZeroAmount);
  require!(min_lst_out >= MIN_LST_DEPOSIT, LaminarError::AmountTooSmall);

  let min_cr_bps = ctx.accounts.global_state.min_cr_bps;

  // Snapshot CR before redemption (post oracle+LST sync).
  let mut post_drawdown_cr_bps = {
    let gs = &ctx.accounts.global_state;
    let tvl = compute_tvl_sol(gs.total_lst_amount, gs.mock_lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;
    let liab = if gs.amusd_supply > 0 {
      compute_liability_sol(gs.amusd_supply, pricing.price_safe_usd)
        .ok_or(LaminarError::MathOverflow)?
    } else {
      0
    };
    compute_cr_bps(tvl, liab)
  };

  let mut drawdown_rounds_executed = 0u8;

  // CR < min => execute drawdown rounds before any collateral-out redemption.
  if post_drawdown_cr_bps < min_cr_bps {
    msg!("CR below min; running drawdown-first rounds before redeem_amusd");

    for _ in 0..MAX_DRAWDOWN_ROUNDS_PER_REDEEM {
      let executed = execute_drawdown_round_if_needed(
        ctx.accounts.global_state.as_mut(),
        ctx.accounts.stability_pool_state.as_mut(),
        ctx.accounts.amusd_mint.as_mut(),
        ctx.accounts.asol_mint.as_mut(),
        ctx.accounts.pool_amusd_vault.as_mut(),
        ctx.accounts.pool_asol_vault.as_mut(),
        &ctx.accounts.stability_pool_authority,
        &ctx.accounts.token_program,
        pricing.price_safe_usd,
      )?;

      if !executed {
        break;
      }

      drawdown_rounds_executed = drawdown_rounds_executed.saturating_add(1);

      let gs = &ctx.accounts.global_state;
      let tvl_now = compute_tvl_sol(gs.total_lst_amount, gs.mock_lst_to_sol_rate)
        .ok_or(LaminarError::MathOverflow)?;
      let liab_now = if gs.amusd_supply > 0 {
        compute_liability_sol(gs.amusd_supply, pricing.price_safe_usd)
          .ok_or(LaminarError::MathOverflow)?
      } else {
        0
      };
      post_drawdown_cr_bps = compute_cr_bps(tvl_now, liab_now);

      if post_drawdown_cr_bps >= min_cr_bps || ctx.accounts.stability_pool_state.total_amusd == 0 {
        break;
      }
    }

    if post_drawdown_cr_bps < min_cr_bps
      && ctx.accounts.stability_pool_state.total_amusd > 0
      && drawdown_rounds_executed == MAX_DRAWDOWN_ROUNDS_PER_REDEEM
    {
      return err!(LaminarError::ConversionCapExceeded);
    }
  }

  if drawdown_rounds_executed > 0 {
    msg!(
      "Drawdown rounds executed before redeem: {}",
      drawdown_rounds_executed
    );
  }

  // Capture values from post-drawdown, pre-redemption state.
  let global_state = &ctx.accounts.global_state;
  let sol_price_redeem_usd = pricing.price_redeem_usd;
  let sol_price_safe_usd = pricing.price_safe_usd;
  let lst_to_sol_rate = global_state.mock_lst_to_sol_rate;
  let current_lst_amount = global_state.total_lst_amount;
  let current_amusd_supply = global_state.amusd_supply;
  let target_cr_bps = global_state.target_cr_bps;
  let fee_amusd_redeem_bps = global_state.fee_amusd_redeem_bps;
  let fee_min_multiplier_bps = global_state.fee_min_multiplier_bps;
  let fee_max_multiplier_bps = global_state.fee_max_multiplier_bps;
  let uncertainty_index_bps = global_state.uncertainty_index_bps;
  let uncertainty_max_bps = global_state.uncertainty_max_bps;

  let current_rounding_reserve = global_state.rounding_reserve_lamports;
  let max_rounding_reserve = global_state.max_rounding_reserve_lamports;

  msg!("amUSD to redeem: {}", amusd_amount);

  let old_tvl =
    compute_tvl_sol(current_lst_amount, lst_to_sol_rate).ok_or(LaminarError::MathOverflow)?;

  let old_liability = if current_amusd_supply > 0 {
    compute_liability_sol(current_amusd_supply, sol_price_safe_usd)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let old_cr_bps = post_drawdown_cr_bps;
  let insolvency_mode = post_drawdown_cr_bps < BPS_PRECISION;

  let (amusd_net_in, amusd_fee_in) = if insolvency_mode {
    (amusd_amount, 0u64)
  } else {
    let fee_bps = compute_dynamic_fee_bps(
      fee_amusd_redeem_bps,
      FeeAction::AmUSDRedeem,
      post_drawdown_cr_bps,
      min_cr_bps,
      target_cr_bps,
      fee_min_multiplier_bps,
      fee_max_multiplier_bps,
      uncertainty_index_bps,
      uncertainty_max_bps,
    )
    .ok_or(LaminarError::InvalidParameter)?;

    let (net_in, fee_in) = apply_fee(amusd_amount, fee_bps).ok_or(LaminarError::MathOverflow)?;
    require!(net_in > 0, LaminarError::AmountTooSmall);
    (net_in, fee_in)
  };

  msg!("amUSD input: {}", amusd_amount);
  msg!("amUSD fee (to treasury): {}", amusd_fee_in);
  msg!("amUSD net burn basis: {}", amusd_net_in);

  let sol_value_par_down = mul_div_down(amusd_net_in, SOL_PRECISION, sol_price_redeem_usd)
    .ok_or(LaminarError::MathOverflow)?;
  let lst_par_down = mul_div_down(sol_value_par_down, SOL_PRECISION, lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let (sol_value_gross, lst_out, reserve_debit_from_redeem, rounding_k_lamports) =
    if insolvency_mode {
      // Haircut path for CR < 100%.
      let haircut_bps = post_drawdown_cr_bps.min(BPS_PRECISION);

      let sol_value_haircut = mul_div_down(sol_value_par_down, haircut_bps, BPS_PRECISION)
        .ok_or(LaminarError::MathOverflow)?;
      let lst_haircut = mul_div_down(sol_value_haircut, SOL_PRECISION, lst_to_sol_rate)
        .ok_or(LaminarError::MathOverflow)?;

      (sol_value_haircut, lst_haircut, 0u64, 3u64)
    } else {
      // Solvent path: user-favoring rounding; reserve debited by deterministic delta.
      let sol_value_up = mul_div_up(amusd_net_in, SOL_PRECISION, sol_price_redeem_usd)
        .ok_or(LaminarError::MathOverflow)?;
      let lst_gross_up = mul_div_up(sol_value_up, SOL_PRECISION, lst_to_sol_rate)
        .ok_or(LaminarError::MathOverflow)?;

      let redeem_rounding_delta_lst = compute_rounding_delta_units(lst_par_down, lst_gross_up)
        .ok_or(LaminarError::MathOverflow)?;
      let lamport_debit = lst_dust_to_lamports_up(redeem_rounding_delta_lst, lst_to_sol_rate)
        .ok_or(LaminarError::MathOverflow)?;

      if lamport_debit <= current_rounding_reserve {
        (sol_value_up, lst_gross_up, lamport_debit, 2u64)
      } else {
        msg!("Rounding reserve insufficient for user-favoring redeem rounding: fallback to conservative");
        (sol_value_par_down, lst_par_down, 0u64, 2u64)
      }
    };

  msg!("SOL value (after mode rules): {}", sol_value_gross);

  require!(lst_out >= min_lst_out, LaminarError::SlippageExceeded);
  let total_lst_out = lst_out;

  let new_lst_amount = current_lst_amount
    .checked_sub(total_lst_out)
    .ok_or(LaminarError::InsufficientCollateral)?;

  require!(
    new_lst_amount >= MIN_PROTOCOL_TVL || new_lst_amount == 0,
    LaminarError::BelowMinimumTVL
  );

  let new_tvl = compute_tvl_sol(new_lst_amount, lst_to_sol_rate).ok_or(LaminarError::MathOverflow)?;

  let new_amusd_supply = current_amusd_supply
    .checked_sub(amusd_net_in)
    .ok_or(LaminarError::InsufficientSupply)?;

  let new_liability = if new_amusd_supply > 0 {
    compute_liability_sol(new_amusd_supply, sol_price_safe_usd).ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let new_rounding_reserve =
    debit_rounding_reserve(current_rounding_reserve, reserve_debit_from_redeem)?;

  let new_accounting_equity = compute_accounting_equity_sol(
    new_tvl,
    new_liability,
    new_rounding_reserve,
  )
  .ok_or(LaminarError::MathOverflow)?;

  let new_cr = if new_amusd_supply > 0 {
    let cr = compute_cr_bps(new_tvl, new_liability);
    msg!("Post-redeem CR: {}bps ({}%)", cr, cr / 100);
    cr
  } else {
    msg!("All amUSD redeemed - CR check skipped");
    u64::MAX
  };

  // (USD -> SOL, SOL -> LST): k_usd=1 and k_lamports path-dependent.
  let rounding_bound_lamports =
    derive_rounding_bound_lamports(rounding_k_lamports, 1, sol_price_safe_usd)?;

  require!(
    ctx.accounts.user_amusd_account.amount >= amusd_amount,
    LaminarError::InsufficientSupply
  );
  require!(
    ctx.accounts.vault.amount >= total_lst_out,
    LaminarError::InsufficientCollateral
  );

  assert_rounding_reserve_within_cap(new_rounding_reserve, max_rounding_reserve)?;
  assert_balance_sheet_holds(
    new_tvl,
    new_liability,
    new_accounting_equity,
    new_rounding_reserve,
    rounding_bound_lamports,
  )?;

  {
    let global_state = &mut ctx.accounts.global_state;
    global_state.total_lst_amount = new_lst_amount;
    global_state.amusd_supply = new_amusd_supply;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
    global_state.rounding_reserve_lamports = new_rounding_reserve;
    msg!("State updated: LST={}, amUSD={}", new_lst_amount, new_amusd_supply);
  }

  if amusd_fee_in > 0 {
    let transfer_fee_accounts = TransferChecked {
      from: ctx.accounts.user_amusd_account.to_account_info(),
      mint: ctx.accounts.amusd_mint.to_account_info(),
      to: ctx.accounts.treasury_amusd_account.to_account_info(),
      authority: ctx.accounts.user.to_account_info(),
    };

    token_interface::transfer_checked(
      CpiContext::new(ctx.accounts.token_program.to_account_info(), transfer_fee_accounts),
      amusd_fee_in,
      ctx.accounts.amusd_mint.decimals,
    )?;
    msg!("Transferred {} amUSD fee to treasury", amusd_fee_in);
  }

  let burn_accounts = Burn {
    mint: ctx.accounts.amusd_mint.to_account_info(),
    from: ctx.accounts.user_amusd_account.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  token_interface::burn(
    CpiContext::new(ctx.accounts.token_program.to_account_info(), burn_accounts),
    amusd_net_in,
  )?;
  msg!("Burned {} amUSD from user", amusd_net_in);

  let seeds = &[VAULT_AUTHORITY_SEED, &[ctx.accounts.global_state.vault_authority_bump]];
  let signer = &[&seeds[..]];

  let transfer_user_accounts = TransferChecked {
    from: ctx.accounts.vault.to_account_info(),
    mint: ctx.accounts.lst_mint.to_account_info(),
    to: ctx.accounts.user_lst_account.to_account_info(),
    authority: ctx.accounts.vault_authority.to_account_info(),
  };

  token_interface::transfer_checked(
    CpiContext::new_with_signer(
      ctx.accounts.token_program.to_account_info(),
      transfer_user_accounts,
      signer,
    ),
    lst_out,
    ctx.accounts.lst_mint.decimals,
  )?;
  msg!("Transferred {} LST to user", lst_out);

  ctx.accounts.vault.reload()?;
  ctx.accounts.amusd_mint.reload()?;

  let expected_vault_balance = ctx.accounts.global_state.total_lst_amount;
  require!(
    ctx.accounts.vault.amount == expected_vault_balance,
    LaminarError::BalanceSheetViolation
  );
  require!(
    ctx.accounts.amusd_mint.supply == ctx.accounts.global_state.amusd_supply,
    LaminarError::BalanceSheetViolation
  );

  msg!("Redeem complete!");
  msg!("New TVL: {} lamports", new_tvl);
  msg!("New amUSD supply: {}", new_amusd_supply);

  emit!(AmUSDRedeemed {
    user: ctx.accounts.user.key(),
    amusd_burned: amusd_net_in,
    lst_received: lst_out,
    fee: amusd_fee_in,
    old_tvl,
    new_tvl,
    old_cr_bps,
    new_cr_bps: new_cr,
    sol_price_used: sol_price_redeem_usd,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

  Ok(())
}

#[derive(Accounts)]
pub struct RedeemAmUSD<'info> {
  #[account(mut)]
  pub user: Signer<'info>,

  /// GlobalState PDA
  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    has_one = amusd_mint,
    has_one = asol_mint,
    has_one = treasury,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner,
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  /// amUSD mint
  #[account(
    mut,
    constraint = amusd_mint.mint_authority == COption::Some(global_state.key()) @ LaminarError::InvalidMintAuthority,
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  /// aSOL mint (needed for drawdown-first conversion path)
  #[account(
    mut,
    address = global_state.asol_mint @ LaminarError::InvalidMint,
    constraint = asol_mint.mint_authority == COption::Some(global_state.key()) @ LaminarError::InvalidMintAuthority,
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  /// Stability pool state (required for drawdown-first redemption flow)
  #[account(
    mut,
    seeds = [STABILITY_POOL_STATE_SEED],
    bump = stability_pool_state.bump,
    has_one = global_state,
    has_one = pool_amusd_vault,
    has_one = pool_asol_vault,
  )]
  pub stability_pool_state: Box<Account<'info, StabilityPoolState>>,

  /// CHECK: PDA signer authority for Stability Pool vault CPIs.
  #[account(
    seeds = [STABILITY_POOL_AUTHORITY_SEED],
    bump = stability_pool_state.pool_authority_bump,
  )]
  pub stability_pool_authority: UncheckedAccount<'info>,

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

  /// User's amUSD token account (source of burned amUSD)
  #[account(
    mut,
    token::mint = amusd_mint,
    token::authority = user,
    constraint = user_amusd_account.close_authority == COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// Treasury's amUSD token account (receives redemption fee)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = amusd_mint,
    associated_token::authority = treasury,
    associated_token::token_program = token_program,
  )]
  pub treasury_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// User's LST token account (receives redeemed LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
    constraint = user_lst_account.close_authority == COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (source of LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
    constraint = vault.close_authority == COption::None @ LaminarError::InvalidAccountState,
  )]
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Vault authority PDA - signs transfers from vault
  /// CHECK: PDA validated by seeds
  #[account(
    seeds = [VAULT_AUTHORITY_SEED],
    bump,
  )]
  pub vault_authority: UncheckedAccount<'info>,

  /// LST mint
  #[account(
    constraint = lst_mint.key() == global_state.supported_lst_mint @ LaminarError::UnsupportedLST
  )]
  pub lst_mint: Box<InterfaceAccount<'info, Mint>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,
  pub clock: Sysvar<'info, Clock>,
}
