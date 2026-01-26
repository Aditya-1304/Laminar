//! Mint amUSD instruction - creates stable debt
//! User deposits LST collateral and receives amUSD at $1 NAV

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, MintTo};
use crate::events::AmUSDMinted;
use crate::reentrancy::ReentrancyGuard;
use crate::state::*;
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;

pub fn handler(
  ctx: Context<MintAmUSD>,
  lst_amount: u64, 
  min_amusd_out: u64,
) -> Result<()> {

  let new_lst_amount: u64;
  let new_amusd_supply: u64;
  let amusd_net: u64;
  let fee: u64;
  let new_tvl: u64;
  let new_cr: u64;

  {
    // LOCK ACQUIRED
    let mut guard = ReentrancyGuard::new(&mut ctx.accounts.global_state)?;
    
    // CHECK: Validations
    require!(!guard.state.mint_paused, LaminarError::MintPaused);
    require!(lst_amount > 0, LaminarError::ZeroAmount);

    // COMPUTE: All math logic
    let sol_value = compute_tvl_sol(lst_amount, guard.state.mock_lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    msg!("LST deposited: {}", lst_amount);
    msg!("SOL value: {}", sol_value);

    let amusd_gross = mul_div_down(sol_value, guard.state.mock_sol_price_usd, SOL_PRECISION)
      .ok_or(LaminarError::MathOverflow)?;

    msg!("amUSD gross (before fee): {}", amusd_gross);

    const BASE_FEE_BPS: u64 = 50;
    let fee_result = apply_fee(amusd_gross, BASE_FEE_BPS)
      .ok_or(LaminarError::MathOverflow)?;
    
    amusd_net = fee_result.0;
    fee = fee_result.1;

    msg!("Fee: {} amUSD", fee);
    msg!("amUSD net (to User): {}", amusd_net);

    require!(amusd_net >= min_amusd_out, LaminarError::SlippageExceeded);

    new_lst_amount = guard.state.total_lst_amount
      .checked_add(lst_amount)
      .ok_or(LaminarError::MathOverflow)?;

    new_tvl = compute_tvl_sol(new_lst_amount, guard.state.mock_lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    new_amusd_supply = guard.state.amusd_supply
      .checked_add(amusd_gross)
      .ok_or(LaminarError::MathOverflow)?;

    let new_liability = compute_liability_sol(new_amusd_supply, guard.state.mock_sol_price_usd)
      .ok_or(LaminarError::MathOverflow)?;

    let new_equity = compute_equity_sol(new_tvl, new_liability);
    new_cr = compute_cr_bps(new_tvl, new_liability);

    msg!("Post-mint CR: {}bps ({}%)", new_cr, new_cr / 100);

    // INVARIANTS: Check before committing
    assert_cr_above_minimum(new_cr, guard.state.min_cr_bps)?;
    assert_balance_sheet_holds(new_tvl, new_liability, new_equity)?;

    // EFFECT: Update state atomically
    guard.state.total_lst_amount = new_lst_amount;
    guard.state.amusd_supply = new_amusd_supply;

  } 
  
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

  msg!("Mint complete!");
  msg!("New LST amount: {} lamports", new_lst_amount);
  msg!("New amUSD supply: {} (user {} + treasury {})", new_amusd_supply, amusd_net, fee);

  emit!(AmUSDMinted {
    user: ctx.accounts.user.key(),
    lst_deposited: lst_amount,
    amusd_minted: amusd_net,
    fee,
    new_tvl,
    new_cr_bps: new_cr,
  });

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
  pub global_state: Box<Account<'info, GlobalState>>,

  /// amUSD mint
  #[account(
    mut,
    constraint = amusd_mint.mint_authority == anchor_lang::solana_program::program_option::COption::Some(global_state.key()) 
      @ LaminarError::InvalidMintAuthority,
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  /// User's amUSD token account (receives minted amUSD)
  #[account(
    mut,
    token::mint = amusd_mint,
    token::authority = user,
  )]
  pub user_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Treasury's amUSD token account (receives protocol fees)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = amusd_mint,
    associated_token::authority = treasury,
  )]
  pub treasury_amusd_account: Box<InterfaceAccount<'info, TokenAccount>>,

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

  /// LST mint
  #[account(
    constraint = lst_mint.key() == global_state.supported_lst_mint @ LaminarError::UnsupportedLST
  )]
  pub lst_mint: Box<InterfaceAccount<'info, Mint>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,
}