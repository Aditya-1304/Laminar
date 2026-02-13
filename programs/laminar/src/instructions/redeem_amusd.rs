//! Redeem amUSD instruction - exits stable debt position
//! User burns amUSD and receives LST collateral back
use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, Burn}
};
use crate::{constants::{AMUSD_REDEEM_FEE_BPS, MIN_PROTOCOL_TVL}, events::AmUSDRedeemed, instructions::sync_exchange_rate_in_place, state::*};
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;

pub fn handler(
  ctx: Context<RedeemAmUSD>,
  amusd_amount: u64,
  min_lst_out: u64,
) -> Result<()> {
  
  // All validations before any state changes
  assert_not_cpi_context()?;

  // sync first
  {
  let global_state = &mut ctx.accounts.global_state;
  global_state.validate_version()?;
  sync_exchange_rate_in_place(global_state, ctx.accounts.clock.slot)?;
  }

  // read only borrow
  let global_state = &ctx.accounts.global_state;

  assert_oracle_freshness_and_confidence(
    ctx.accounts.clock.slot, 
    global_state.last_oracle_update_slot, 
    global_state.max_oracle_staleness_slots, 
    global_state.mock_sol_price_usd, 
    global_state.mock_oracle_confidence_usd, 
    global_state.max_conf_bps
  )?;

  // Capture values
  let sol_price_used = global_state.mock_sol_price_usd;
  let lst_to_sol_rate = global_state.mock_lst_to_sol_rate;
  let current_lst_amount = global_state.total_lst_amount;
  let current_amusd_supply = global_state.amusd_supply;
  let target_cr_bps = global_state.target_cr_bps;

  let current_rounding_reserve = global_state.rounding_reserve_lamports;

  // Configured hard cap for reserve growth.
  let max_rounding_reserve = global_state.max_rounding_reserve_lamports;

  // Validations
  require!(!global_state.redeem_paused, LaminarError::RedeemPaused);
  require!(amusd_amount > 0, LaminarError::ZeroAmount);
  require!(min_lst_out > 0, LaminarError::ZeroAmount);
  require!(min_lst_out >= MIN_LST_DEPOSIT, LaminarError::AmountTooSmall);

  msg!("amUSD to redeem: {}", amusd_amount);

  // All math logic
  let old_tvl = compute_tvl_sol(current_lst_amount, lst_to_sol_rate).ok_or(LaminarError::MathOverflow)?;

  let old_liability = if current_amusd_supply > 0 {
    compute_liability_sol(current_amusd_supply, sol_price_used).ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let old_cr_bps = compute_cr_bps(old_tvl, old_liability);
  let min_cr_bps = global_state.min_cr_bps;

  // Whitepaper requires drawdown-first when CR < min_cr_bps.
  // Stability Pool is not implemented yet, so this pre-stability build
  // deterministically treats the pool as exhausted (A == 0) and proceeds
  // to haircut mode only when CR < 100%.
  let post_drawdown_cr_bps = old_cr_bps;
  if post_drawdown_cr_bps < min_cr_bps {
    msg!("CR below min: drawdown-first required by spec; Stability Pool not implemented yet, treating pool as exhausted");
  }

  let insolvency_mode = post_drawdown_cr_bps < BPS_PRECISION;

  let (amusd_net_in, amusd_fee_in) = if insolvency_mode {
    (amusd_amount, 0u64)
  } else {
    let fee_bps = fee_bps_decrease_when_low(AMUSD_REDEEM_FEE_BPS, post_drawdown_cr_bps, target_cr_bps);
    let (net_in, fee_in) = apply_fee(amusd_amount, fee_bps)
      .ok_or(LaminarError::MathOverflow)?;
    require!(net_in > 0, LaminarError::AmountTooSmall);
    (net_in, fee_in)
  };
  
  msg!("amUSD input: {}", amusd_amount);
  msg!("amUSD fee (to treasury): {}", amusd_fee_in);
  msg!("amUSD net burn basis: {}", amusd_net_in);

  // Baseline par path (all-down)
  let sol_value_par_down = mul_div_down(amusd_net_in, SOL_PRECISION, sol_price_used)
    .ok_or(LaminarError::MathOverflow)?;
  let lst_par_down = mul_div_down(sol_value_par_down, SOL_PRECISION, lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let (sol_value_gross, lst_out, reserve_debit_from_redeem, rounding_k_lamports) = if insolvency_mode {
    // Haircut path for CR < 100%
    let haircut_bps = post_drawdown_cr_bps.min(BPS_PRECISION);

    let sol_value_haircut = mul_div_down(sol_value_par_down, haircut_bps, BPS_PRECISION)
      .ok_or(LaminarError::MathOverflow)?;
    let lst_haircut = mul_div_down(sol_value_haircut, SOL_PRECISION, lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    (sol_value_haircut, lst_haircut, 0u64, 3u64)
  } else {
    // Solvent path: user-favoring rounding, reserve debited by deterministic delta
    let sol_value_up = mul_div_up(amusd_net_in, SOL_PRECISION, sol_price_used)
      .ok_or(LaminarError::MathOverflow)?;
    let lst_gross_up = mul_div_up(sol_value_up, SOL_PRECISION, lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    let redeem_rounding_delta_lst = compute_rounding_delta_units(lst_par_down, lst_gross_up)
      .ok_or(LaminarError::MathOverflow)?;
    let lamport_debit = lst_dust_to_lamports_up(redeem_rounding_delta_lst, lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    (sol_value_up, lst_gross_up, lamport_debit, 2u64)
  };

  msg!("SOL value (after mode rules): {}", sol_value_gross);

  require!(lst_out >= min_lst_out, LaminarError::SlippageExceeded);
  let total_lst_out = lst_out;

  // Calculate new state values
  let new_lst_amount = current_lst_amount
    .checked_sub(total_lst_out)
    .ok_or(LaminarError::InsufficientCollateral)?;

  require!(
    new_lst_amount >= MIN_PROTOCOL_TVL || new_lst_amount == 0,
    LaminarError::BelowMinimumTVL
  );

  let new_tvl = compute_tvl_sol(new_lst_amount, lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let new_amusd_supply = current_amusd_supply
    .checked_sub(amusd_net_in)
    .ok_or(LaminarError::InsufficientSupply)?;

  let new_liability = if new_amusd_supply > 0 {
    compute_liability_sol(new_amusd_supply, sol_price_used)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let new_rounding_reserve = debit_rounding_reserve(current_rounding_reserve, reserve_debit_from_redeem)?;

  // signed accounting equity (can be negative under insolvency).
  let new_accounting_equity = compute_accounting_equity_sol(new_tvl, new_liability, new_rounding_reserve).ok_or(LaminarError::MathOverflow)?;

  // NOTE: No CR minimum check here because amUSD redemption improves or
  // maintains CR when the protocol is solvent (TVL >= liability).
  // Insolvent redemptions are blocked by the no-negative-equity invariant.
  // When CR < 150%, redemption fee decreases"
  // to ENCOURAGE debt repayment during stress - not block it.
  let new_cr = if new_amusd_supply > 0 {
    let cr = compute_cr_bps(new_tvl, new_liability);
    msg!("Post-redeem CR: {}bps ({}%)", cr, cr / 100);
    cr
  } else {
    msg!("All amUSD redeemed - CR check skipped");
    u64::MAX
  };

  // Deterministic rounding bound for redeem_amusd path:
  // (Usd -> SOL, SOL -> LST) => (k_lamports = 2, k_usd = 1)
  let rounding_bound_lamports = derive_rounding_bound_lamports(rounding_k_lamports, 1, sol_price_used)?;

  require!(
    ctx.accounts.user_amusd_account.amount >= amusd_amount,
    LaminarError::InsufficientSupply
  );

  // Verify vault has enough funds
  require!(
    ctx.accounts.vault.amount >= total_lst_out,
    LaminarError::InsufficientCollateral
  );

  // Invariants check
  assert_rounding_reserve_within_cap(new_rounding_reserve, max_rounding_reserve)?;
  assert_balance_sheet_holds(new_tvl, new_liability, new_accounting_equity, new_rounding_reserve, rounding_bound_lamports)?;

  // Update state BEFORE external calls
  
  
  {
    let global_state = &mut ctx.accounts.global_state;
    global_state.total_lst_amount = new_lst_amount;
    global_state.amusd_supply = new_amusd_supply;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
    global_state.rounding_reserve_lamports = new_rounding_reserve;
    msg!("State updated: LST={}, amUSD={}", new_lst_amount, new_amusd_supply);
  }

  
  // External calls (CPIs)
  
  // Transfer fee to treasury
  if amusd_fee_in > 0 {
    let transfer_fee_accounts = TransferChecked {
      from: ctx.accounts.user_amusd_account.to_account_info(),
      mint: ctx.accounts.amusd_mint.to_account_info(),
      to: ctx.accounts.treasury_amusd_account.to_account_info(),
      authority: ctx.accounts.user.to_account_info(),
    };

    let cpi_ctx_treasury = CpiContext::new(
      ctx.accounts.token_program.to_account_info(),
      transfer_fee_accounts,
    );

    token_interface::transfer_checked(cpi_ctx_treasury, amusd_fee_in, ctx.accounts.amusd_mint.decimals)?;
    msg!("Transferred {} amUSD fee to treasury", amusd_fee_in);
  }

  // Burn amUSD from user
  let burn_accounts = Burn {
    mint: ctx.accounts.amusd_mint.to_account_info(),
    from: ctx.accounts.user_amusd_account.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  let cpi_ctx_burn = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
    burn_accounts
  );

  token_interface::burn(cpi_ctx_burn, amusd_net_in)?;
  msg!("Burned {} amUSD from user", amusd_net_in);

  let seeds = &[VAULT_AUTHORITY_SEED, &[ctx.accounts.global_state.vault_authority_bump]];
  let signer = &[&seeds[..]];

  let transfer_user_accounts = TransferChecked {
    from: ctx.accounts.vault.to_account_info(),
    mint: ctx.accounts.lst_mint.to_account_info(),
    to: ctx.accounts.user_lst_account.to_account_info(),
    authority: ctx.accounts.vault_authority.to_account_info(),
  };

  let cpi_ctx_user = CpiContext::new_with_signer(
    ctx.accounts.token_program.to_account_info(),
    transfer_user_accounts,
    signer
  );

  token_interface::transfer_checked(cpi_ctx_user, lst_out, ctx.accounts.lst_mint.decimals)?;
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
    sol_price_used,
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
    has_one = treasury,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner,
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  /// amUSD mint
  #[account(
    mut,
    constraint = amusd_mint.mint_authority == anchor_lang::solana_program::program_option::COption::Some(global_state.key())   @ LaminarError::InvalidMintAuthority,
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  /// User's amUSD token account (source of burned amUSD)
  #[account(
    mut,
    token::mint = amusd_mint,
    token::authority = user,
    constraint = user_amusd_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// Treasury's LST token account (receives redemption fee)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = amusd_mint,
    associated_token::authority = treasury,
    associated_token::token_program = token_program,
    // constraint = treasury_amusd_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub treasury_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// User's LST token account (receives redeemed LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
    constraint = user_lst_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (source of LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
    constraint = vault.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
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
