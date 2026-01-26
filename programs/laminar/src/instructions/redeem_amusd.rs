//! Redeem amUSD instruction - exits stable debt position
//! User burns amUSD and receives LST collateral back

use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, Burn}
};
use crate::{events::AmUSDRedeemed, state::*};
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;
use crate::unlock_state;
use crate::lock_state;

pub fn handler(
  ctx: Context<RedeemAmUSD>,
  amusd_amount: u64,
  min_lst_out: u64,
) -> Result<()> {

  lock_state!(ctx.accounts.global_state);

  let redeem_paused = ctx.accounts.global_state.redeem_paused;
  let mock_lst_to_sol_rate = ctx.accounts.global_state.mock_lst_to_sol_rate;
  let mock_sol_price_usd = ctx.accounts.global_state.mock_sol_price_usd;
  let current_lst_amount = ctx.accounts.global_state.total_lst_amount;
  let current_amusd_supply = ctx.accounts.global_state.amusd_supply;

  require!(!redeem_paused, LaminarError::RedeemPaused);
  require!(amusd_amount > 0, LaminarError::ZeroAmount);

  msg!("amUSD to redeem: {}", amusd_amount);

  // calculate SOL value to return 
  let sol_value_gross = mul_div_down(amusd_amount, SOL_PRECISION, mock_sol_price_usd)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("SOL value (before fee): {}", sol_value_gross);

  // Apply redemption fee (25 bps = 0.25%, lower than mint fee to encourage redemption)
  const REDEEM_FEE_BPS: u64 = 25;
  let (sol_value_net, fee_sol) = apply_fee(sol_value_gross, REDEEM_FEE_BPS)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("Fee: {} SOL (retained in vault)", fee_sol);
  msg!("Net: {} SOL (to user)", sol_value_net);

  // Convert SOL amounts to LST amounts
  let lst_net = mul_div_down(sol_value_net, SOL_PRECISION, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  require!(
    lst_net >= min_lst_out,
    LaminarError::SlippageExceeded
  );

  let lst_fee = mul_div_down(fee_sol, SOL_PRECISION, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;


  msg!("LST to user: {}", lst_net);
  msg!("LST fee to treasury: {}", lst_fee);

 // Total LST being removed (user + treasury fee)
  let total_lst_out = lst_net.checked_add(lst_fee)
    .ok_or(LaminarError::MathOverflow)?;

  let new_lst_amount = current_lst_amount
    .checked_sub(total_lst_out)
    .ok_or(LaminarError::InsufficientCollateral)?;

  // Compute new TVL from remaining LST
  let new_tvl = compute_tvl_sol(new_lst_amount, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let new_amusd_supply = current_amusd_supply
    .checked_sub(amusd_amount)
    .ok_or(LaminarError::InsufficientSupply)?;

  // Compute new liability and equity
  let new_liability = if new_amusd_supply > 0 {
    compute_liability_sol(new_amusd_supply, mock_sol_price_usd)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0 // No debt remaining
  };

  let new_equity = compute_equity_sol(new_tvl, new_liability);

  // Users must be able to exit even during crisis
  let new_cr = if new_amusd_supply > 0 {
    let cr = compute_cr_bps(new_tvl, new_liability);
    msg!("Post-redeem CR: {}bps ({}%)", cr, cr / 100);
    cr
  } else {
    msg!("All amUSD redeemed - CR check skipped");
    u64::MAX  // No debt = infinite CR
  };

  require!(
    ctx.accounts.vault.amount >= total_lst_out,
    LaminarError::InsufficientCollateral
  );

  // Invariant check 
  assert_balance_sheet_holds(new_tvl, new_liability, new_equity)?;

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

  token_interface::burn(cpi_ctx_burn, amusd_amount)?;
  msg!("Burned {} amUSD from user", amusd_amount);

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
  };

  // Update global state atomically
  let global_state = &mut ctx.accounts.global_state;
  global_state.total_lst_amount = new_lst_amount;
  global_state.amusd_supply = new_amusd_supply;

  msg!(" New TVL: {} lamports", new_tvl);
  msg!(" New amUSD supply: {}", new_amusd_supply);

  emit!(AmUSDRedeemed {
    user: ctx.accounts.user.key(),
    amusd_burned: amusd_amount,
    lst_received: lst_net,
    fee: lst_fee,
    new_tvl,
    new_cr_bps: new_cr,
  });

  unlock_state!(ctx.accounts.global_state);

  Ok(())
}

#[derive(Accounts)]
pub struct RedeemAmUSD<'info> {
  #[account(mut)]
  pub user: Signer<'info>,

  /// Globalstate PDA
  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    has_one = amusd_mint,
    has_one = treasury,
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  #[account(mut)]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  /// User's amUSD token account (source of burned amUSD)
  #[account(
    mut,
    token::mint = amusd_mint,
    token::authority = user,
  )]
  pub user_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// Treasury's LST token account (receives redemption fee)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = lst_mint,
    associated_token::authority = treasury,
  )]
  pub treasury_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// User's LST token account (receives redeemed LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,

  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (source of LST)
  #[account (
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
  )]
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Vault authority PDA - signs transfers from vault
  /// CHECK: PDA Validated by seeds 
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
}

