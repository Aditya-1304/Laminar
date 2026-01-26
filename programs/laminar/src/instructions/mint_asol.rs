//! Mint aSOL instruction - Creates leveraged equity position
//! User deposits LST collateral and receives aSOL at current NAV

use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, MintTo}
};

use crate::{events::AsolMinted, state::*};
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;
use crate::unlock_state;
use crate::lock_state;

pub fn handler(
  ctx: Context<MintAsol>,
  lst_amount: u64,
  min_asol_out: u64,
) -> Result<()> {

  lock_state!(ctx.accounts.global_state);

  let mint_paused = ctx.accounts.global_state.mint_paused;
  let mock_lst_to_sol_rate = ctx.accounts.global_state.mock_lst_to_sol_rate;
  let mock_sol_price_usd = ctx.accounts.global_state.mock_sol_price_usd;
  let current_lst_amount = ctx.accounts.global_state.total_lst_amount;
  let current_amusd_supply = ctx.accounts.global_state.amusd_supply;
  let current_asol_supply = ctx.accounts.global_state.asol_supply;

  require!(!mint_paused, LaminarError::MintPaused);
  require!(lst_amount > 0, LaminarError::ZeroAmount);

  // SOL value of deposited LST
  let sol_value = compute_tvl_sol(lst_amount, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("LST deposited: {}", lst_amount);
  msg!("SOL value: {}", sol_value);

  // Compute current TVL from LST holdings
  let current_tvl = compute_tvl_sol(current_lst_amount, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;
  // Compute current liability to calculate NAV
  let current_liability = if current_amusd_supply > 0 {
    compute_liability_sol(current_amusd_supply, mock_sol_price_usd)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0 // No debt exists yet
  };

  // Compute current NAV (needed for both calculation and event emission)
  let current_nav = if current_asol_supply == 0 {
    SOL_PRECISION
  } else {
    nav_asol(current_tvl, current_liability, current_asol_supply)
  };

  // First mint: NAV = 1 SOL per aSOL 
  let asol_gross = if current_asol_supply == 0 {
    sol_value
  } else {
    if current_nav == 0 {
      return Err(LaminarError::InsolventProtocol.into());
    }

    // asol_to_mint = sol_value / nav_asol
    mul_div_down(sol_value, SOL_PRECISION, current_nav)
      .ok_or(LaminarError::MathOverflow)?
  };

  msg!("aSOL gross (before fee): {}", asol_gross);

  // Apply mint fee (30 bps = 0.30%)
  const BASE_FEE_BPS: u64 = 30;
  let (asol_net, fee) = apply_fee(asol_gross, BASE_FEE_BPS)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("Fee: {} aSOL", fee);
  msg!("aSOL net (to user): {}", asol_net);

  require!(
    asol_net >= min_asol_out,
    LaminarError::SlippageExceeded
  );

  let new_lst_amount = current_lst_amount
    .checked_add(lst_amount)
    .ok_or(LaminarError::MathOverflow)?;

  let new_tvl = compute_tvl_sol(new_lst_amount, mock_lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let new_asol_supply = current_asol_supply
    .checked_add(asol_gross)
    .ok_or(LaminarError::MathOverflow)?;

  // Liability doesn't change (only equity increases)
  let new_liability = current_liability;

  let new_equity = compute_equity_sol(new_tvl, new_liability);

  let new_cr = if new_liability > 0 {
    compute_cr_bps(new_tvl, new_liability)
  } else {
    // No debt exists, CR is infinite
    u64::MAX
  };

  if new_liability > 0 {
    msg!("Post-mint CR: {}bps ({}%)", new_cr, new_cr / 100);
  } else {
    msg!("Post-mint CR: infinite (no debt exists)");
  }

  // Reject aSOL mints if protocol is insolvent (aSOL would be worthless)
  // This prevents users from depositing into a dead protocol
  if new_liability > 0 && new_equity == 0 {
    return Err(LaminarError::InsolventProtocol.into());
  }
  
  assert_balance_sheet_holds(new_tvl, new_liability, new_equity)?;

  // Transfer LST from user to vault
  let transfer_accounts = TransferChecked {
    from: ctx.accounts.user_lst_account.to_account_info(),
    mint: ctx.accounts.lst_mint.to_account_info(),
    to: ctx.accounts.vault.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  let cpi_ctx = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
    transfer_accounts,
  );

  token_interface::transfer_checked(cpi_ctx, lst_amount, ctx.accounts.lst_mint.decimals)?;

  msg!("Transferred {} LST to vault", lst_amount);

  // Mint aSOL to user (net amount after fee)
  let seeds = &[GLOBAL_STATE_SEED, &[ctx.bumps.global_state]];
  let signer = &[&seeds[..]];

  let mint_to_user = MintTo {
    mint: ctx.accounts.asol_mint.to_account_info(),
    to: ctx.accounts.user_asol_account.to_account_info(),
    authority: ctx.accounts.global_state.to_account_info(),
  };

  let cpi_ctx_user = CpiContext::new_with_signer(
    ctx.accounts.token_program.to_account_info(),
    mint_to_user,
    signer,
  );

  token_interface::mint_to(cpi_ctx_user, asol_net)?;
  msg!("Minted {} aSOL to user", asol_net);

  // Mint fee to treasury
  let mint_to_treasury = MintTo {
    mint: ctx.accounts.asol_mint.to_account_info(),
    to: ctx.accounts.treasury_asol_account.to_account_info(),
    authority: ctx.accounts.global_state.to_account_info(),
  };

  let cpi_ctx_treasury = CpiContext::new_with_signer(
    ctx.accounts.token_program.to_account_info(),
    mint_to_treasury,
    signer,
  );

  token_interface::mint_to(cpi_ctx_treasury, fee)?;
  msg!("Minted {} aSOL fee to treasury", fee);

  // Update global state atomically
  let global_state = &mut ctx.accounts.global_state;
  global_state.total_lst_amount = new_lst_amount;
  global_state.asol_supply = new_asol_supply;

  msg!("Mint complete!");
  msg!("New TVL: {} lamports", new_tvl);
  msg!("New aSOL supply: {} (user {} + treasury {})", new_asol_supply, asol_net, fee);

  emit!(AsolMinted {
    user: ctx.accounts.user.key(),
    lst_deposited: lst_amount,
    asol_minted: asol_net,
    fee,
    nav: current_nav,
    new_tvl,
    new_equity,
  });

  unlock_state!(ctx.accounts.global_state);

  Ok(())
}

#[derive(Accounts)]
pub struct MintAsol<'info> {
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
  pub global_state: Box<Account<'info, GlobalState>>,

  /// aSOL mint
  #[account(mut)]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  /// User's aSOL token account (receives minted aSOL)
  #[account (
    mut,
    token::mint = asol_mint,
    token::authority = user,
  )]
  pub user_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Treasury's aSOL token account (receives protocol fees)
  /// SECURITY: Must be owned by treasury wallet to prevent fee theft
  /// Auto-created on first mint to reduce operational friction
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = asol_mint,
    associated_token::authority = treasury,
  )]
  pub treasury_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// User's LST token account (source of collateral)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (receives LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
  )]
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

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
  pub lst_mint: Box<InterfaceAccount<'info, Mint>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,

}