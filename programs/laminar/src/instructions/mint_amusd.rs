//! Mint amUSD instrution - creates stable debt
//! User deposits LST collateral and receives amUSD at $1 NAV

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, MintTo};
use crate::state::*;
use crate::math::*;
use crate::invariants::*;

pub fn handler(
  ctx: Context<MintAmUSD>,
  lst_amount: u64, 
) -> Result<()> {

  let mint_paused = ctx.accounts.global_state.mint_paused;
  let mock_lst_to_sol_rate = ctx.accounts.global_state.mock_lst_to_sol_rate;
  let mock_sol_price_usd = ctx.accounts.global_state.mock_sol_price_usd;
  let current_tvl = ctx.accounts.global_state.total_collateral_lamports;
  let current_amusd_supply = ctx.accounts.global_state.amusd_supply;
  let min_cr_bps = ctx.accounts.global_state.min_cr_bps;

  require!(!mint_paused, ErrorCode::MintPaused);

  // SOL value of deposited LST
  let sol_value = compute_tvl_sol(lst_amount, mock_lst_to_sol_rate)
    .ok_or(ErrorCode::MathOverflow)?;

  msg!("LST deposited: {}", lst_amount);
  msg!("SOL value: {}", sol_value);

  // Calculates amUSD to mint: sol_value * price (in USD terms)
  // sol_value is in lamports (1e9), price is in USD (1e6)
  // Result should be in USD (1e6)
  let amusd_gross = mul_div_down(sol_value, mock_sol_price_usd, SOL_PRECISION)
    .ok_or(ErrorCode::MathOverflow)?;

  msg!("amUSD gross (before fee): {}", amusd_gross);

  // fee (0.5% = 50 bps for now - will change this later)
  const BASE_FEE_BPS: u64 = 50;
  let (amusd_net, fee) = apply_fee(amusd_gross, BASE_FEE_BPS)
    .ok_or(ErrorCode::MathOverflow)?;

  msg!("Fee: {} amUSD", fee);
  msg!("amUSD net (to User): {}", amusd_net);

  // Simulate post-state to check CR
  let new_tvl = current_tvl
    .checked_add(sol_value)
    .ok_or(ErrorCode::MathOverflow)?;

  let new_amusd_supply = current_amusd_supply
    .checked_add(amusd_gross)
    .ok_or(ErrorCode::MathOverflow)?;

  // compute new liability in SOL terms
  let new_liability = compute_liability_sol(new_amusd_supply, mock_sol_price_usd)
    .ok_or(ErrorCode::MathOverflow)?;

  // compute new equity
  let new_equity = compute_equity_sol(new_tvl, new_liability);

  // compute new CR
  let new_cr = compute_cr_bps(new_tvl, new_liability);

  msg!("Post-mint CR: {}bps ({}%)", new_cr, new_cr / 100);

  // Invariant checks BEFORE any state changes
  assert_cr_above_minimum(new_cr, min_cr_bps)?;
  assert_balance_sheet_holds(new_tvl, new_liability, new_equity)?;

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

  // Mint amUSD to user (net amount after fee)
  let seeds = &[GLOBAL_STATE_SEED, &[ctx.bumps.global_state]];
  let signer = &[&seeds[..]];

  let mint_to_user = MintTo {
        mint: ctx.accounts.amusd_mint.to_account_info(),
        to: ctx.accounts.user_amusd_account.to_account_info(),
        authority: ctx.accounts.global_state.to_account_info(),
    };
    let cpi_ctx_user = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        mint_to_user,
        signer,
    );
    token_interface::mint_to(cpi_ctx_user, amusd_net)?;
    msg!("Minted {} amUSD to user", amusd_net);

    // Mint fee to treasury
    let mint_to_treasury = MintTo {
      mint: ctx.accounts.amusd_mint.to_account_info(),
      to: ctx.accounts.treasury_amusd_account.to_account_info(),
      authority: ctx.accounts.global_state.to_account_info(),
    };

    let cpi_ctx_treasury = CpiContext::new_with_signer(
      ctx.accounts.token_program.to_account_info(),
      mint_to_treasury,
      signer,
    );

    token_interface::mint_to(cpi_ctx_treasury, fee)?;
    msg!("Minted {} amUSD fee to treasury", fee);

    let global_state = &mut ctx.accounts.global_state;
    global_state.total_collateral_lamports = new_tvl;
    global_state.amusd_supply = new_amusd_supply;

    msg!("✅ Mint complete!");
    msg!("New TVL: {} lamports", new_tvl);
    msg!("New amUSD supply: {} (user {} + treasury {})", new_amusd_supply, amusd_net, fee);

  Ok(())
}

#[derive(Accounts)]
pub struct MintAmUSD<'info> {
  #[account(mut)]
  pub user: Signer<'info>,

  /// GlobalState PDA
  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    has_one = amusd_mint,
    has_one = treasury,
  )]
  pub global_state: Account<'info, GlobalState>,

  /// amUSD mint
  #[account(mut)]
  pub amusd_mint: InterfaceAccount<'info, Mint>,

  /// User's amUSD token account (recieves minted amUSD)
  #[account(
    mut,
    token::mint = amusd_mint,
    token::authority = user,
  )]
  pub user_amusd_account: InterfaceAccount<'info, TokenAccount>,

  /// Treasury's amUSD token account (recieves protocol fees)
  /// SECURITY: Must be owned by treasury wallet to prevent fee theft
  #[account(
    mut,
    token::mint = amusd_mint,
    token::authority = treasury,
  )]
  pub treasury_amusd_account: InterfaceAccount<'info, TokenAccount>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// User's LST token account (source of collateral)
  #[account(
    mut,
    token::mint = lst_mint,
  )]
  pub user_lst_account: InterfaceAccount<'info, TokenAccount>,

  /// Protocol vault (recieves LST)
  #[account(
    mut,
    token::mint = lst_mint,
  )]
  pub vault: InterfaceAccount<'info, TokenAccount>,

  /// LST mint
  pub lst_mint: InterfaceAccount<'info, Mint>,

  pub token_program: Interface<'info, TokenInterface>,
}

#[error_code]
pub enum ErrorCode {
  #[msg("Minting is currently paused")]
  MintPaused,

  #[msg("Math overflow occurred")]
  MathOverflow,
}