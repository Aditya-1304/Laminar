//! Redeem aSOL instruction - exits leveraged equity position
//! User burns aSOL and receives LST collateral back at current NAV

use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, Burn}
};
use crate::state::*;
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;

pub fn handler(
  ctx: Context<RedeemAsol>,
  asol_amount: u64,
) -> Result<()> {

  let redeem_paused = ctx.accounts.global_state.redeem_paused;
  let mock_lst_to_sol_rate = ctx.accounts.global_state.mock_lst_to_sol_rate;
  let mock_sol_price_usd = ctx.accounts.global_state.mock_sol_price_usd;
  let current_tvl = ctx.accounts.global_state.total_collateral_lamports;
  let current_amusd_supply = ctx.accounts.global_state.amusd_supply;
  let current_asol_supply = ctx.accounts.global_state.asol_supply;

  require!(!redeem_paused, LaminarError::RedeemPaused);
  require!(asol_amount > 0, LaminarError::ZeroAmount);

  msg!("aSOL to redeem: {}", asol_amount);

  // Compute current liability
  let current_liability = if current_amusd_supply > 0 {
    compute_liability_sol(current_amusd_supply, mock_sol_price_usd)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  // Calculate current NAV
  let current_nav = nav_asol(current_tvl, current_liability, current_asol_supply);

  if current_nav == 0 {
    return Err(LaminarError::InsolventProtocol.into());
  }

  msg!("Current aSOL NAV: {} lamports per aSOL", current_nav);

  // Calculate SOL value to return (before fee)
  // sol_value = asol_amount * nav_asol
  let sol_value_gross = mul_div_down(asol_amount, current_nav, SOL_PRECISION)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("SOL value (before fee): {}", sol_value_gross);

  // Apply redemption fee (15 bps = 0.15%, lowest fee to encourage liquidity)
  const REDEEM_FEE_BPS: u64 = 15;
  let (sol_value_net, fee_sol) = apply_fee(sol_value_gross, REDEEM_FEE_BPS)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("Fee: {} SOL", fee_sol);
  msg!("Net: {} SOL (to user)", sol_value_net);

  // Convert SOL amounts to LST amounts
  let lst_gross = mul_div_down(sol_value_gross, SOL_PRECISION, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let lst_net = mul_div_down(sol_value_net, SOL_PRECISION, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  // Calculate fee by subtraction to ensure lst_gross = lst_net + lst_fee
  let lst_fee = lst_gross
    .checked_sub(lst_net)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("LST gross: {}", lst_gross);
  msg!("LST to user: {}", lst_net);
  msg!("LST fee to treasury: {}", lst_fee);

  // Simulate post-state for balance sheet check
  let new_tvl = current_tvl
    .checked_sub(sol_value_gross) // Full amount leaves (user gets net, treasury gets fee)
    .ok_or(LaminarError::InsufficientCollateral)?;

  let new_asol_supply = current_asol_supply
    .checked_sub(asol_amount)
    .ok_or(LaminarError::InsufficientSupply)?;

  // Liability doesn't change (only equity decreases)
  let new_liability = current_liability;

  let new_equity = compute_equity_sol(new_tvl, new_liability);

  // Users must be able to exit even during crisis - NO CR CHECK
  if new_liability > 0 {
    let new_cr = compute_cr_bps(new_tvl, new_liability);
    msg!("Post-redeem CR: {}bps ({}%)", new_cr, new_cr / 100);
  } else {
    msg!("No debt exists - CR check skipped");
  }

  // Invariant check
  assert_balance_sheet_holds(new_tvl, new_liability, new_equity)?;

  // Burn aSOL from user
  let burn_accounts = Burn {
    mint: ctx.accounts.asol_mint.to_account_info(),
    from: ctx.accounts.user_asol_account.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  let cpi_ctx_burn = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
    burn_accounts
  );

  token_interface::burn(cpi_ctx_burn, asol_amount)?;
  msg!("Burned {} aSOL from user", asol_amount);

  // Transfer LST from vault to user
  let seeds = &[VAULT_AUTHORITY_SEED, &[ctx.bumps.vault_authority]];
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

  token_interface::transfer_checked(cpi_ctx_user, lst_net, ctx.accounts.lst_mint.decimals)?;
  msg!("Transferred {} LST to user", lst_net);

  // Transfer fee to treasury
  if lst_fee > 0 {
    let transfer_treasury_accounts = TransferChecked {
      from: ctx.accounts.vault.to_account_info(),
      mint: ctx.accounts.lst_mint.to_account_info(),
      to: ctx.accounts.treasury_lst_account.to_account_info(),
      authority: ctx.accounts.vault_authority.to_account_info(),
    };

    let cpi_ctx_treasury = CpiContext::new_with_signer(
      ctx.accounts.token_program.to_account_info(),
      transfer_treasury_accounts,
      signer
    );

    token_interface::transfer_checked(cpi_ctx_treasury, lst_fee, ctx.accounts.lst_mint.decimals)?;
    msg!("Transferred {} LST fee to treasury", lst_fee);
  }

  // Update global state atomically
  let global_state = &mut ctx.accounts.global_state;
  global_state.total_collateral_lamports = new_tvl;
  global_state.asol_supply = new_asol_supply;

  msg!("✅ Redeem complete!");
  msg!("New TVL: {} lamports", new_tvl);
  msg!("New aSOL supply: {}", new_asol_supply);

  Ok(())
}

#[derive(Accounts)]
pub struct RedeemAsol<'info> {
  #[account(mut)]
  pub user: Signer<'info>,

  /// GlobalState PDA
  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    has_one = asol_mint,
    has_one = treasury,
  )]
  pub global_state: Account<'info, GlobalState>,

  /// aSOL mint
  #[account(mut)]
  pub asol_mint: InterfaceAccount<'info, Mint>,

  /// User's aSOL token account (source of burned aSOL)
  #[account(
    mut,
    token::mint = asol_mint,
    token::authority = user,
  )]
  pub user_asol_account: InterfaceAccount<'info, TokenAccount>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// Treasury's LST token account (receives redemption fee)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = lst_mint,
    associated_token::authority = treasury,
  )]
  pub treasury_lst_account: InterfaceAccount<'info, TokenAccount>,

  /// User's LST token account (receives redeemed LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
  )]
  pub user_lst_account: InterfaceAccount<'info, TokenAccount>,

  /// Protocol vault (source of LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
  )]
  pub vault: InterfaceAccount<'info, TokenAccount>,

  /// Vault authority PDA - signs transfers from vault
  /// CHECK: PDA validated by seeds
  #[account(
    seeds = [VAULT_AUTHORITY_SEED],
    bump,
  )]
  pub vault_authority: UncheckedAccount<'info>,

  /// LST mint - SECURITY: Must match whitelisted LST in GlobalState
  #[account(
    constraint = lst_mint.key() == global_state.supported_lst_mint @ LaminarError::UnsupportedLST
  )]
  pub lst_mint: InterfaceAccount<'info, Mint>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,
}