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
use crate::reentrancy::ReentrancyGuard;

pub fn handler(
  ctx: Context<MintAsol>,
  lst_amount: u64,
  min_asol_out: u64,
) -> Result<()> {

  let new_lst_amount: u64;
  let new_asol_supply: u64;
  let asol_net: u64;
  let fee: u64;
  let new_tvl: u64;
  let new_equity: u64;
  let current_nav: u64;

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

    let current_tvl = compute_tvl_sol(guard.state.total_lst_amount, guard.state.mock_lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    let current_liability = if guard.state.amusd_supply > 0 {
      compute_liability_sol(guard.state.amusd_supply, guard.state.mock_sol_price_usd)
        .ok_or(LaminarError::MathOverflow)?
    } else {
      0
    };

    current_nav = if guard.state.asol_supply == 0 {
      SOL_PRECISION
    } else {
      nav_asol(current_tvl, current_liability, guard.state.asol_supply)
        .ok_or(LaminarError::MathOverflow)?
    };

    let asol_gross = if guard.state.asol_supply == 0 {
      sol_value
    } else {
      if current_nav == 0 {
        return Err(LaminarError::InsolventProtocol.into());
      }
      mul_div_down(sol_value, SOL_PRECISION, current_nav)
        .ok_or(LaminarError::MathOverflow)?
    };

    msg!("aSOL gross (before fee): {}", asol_gross);

    const BASE_FEE_BPS: u64 = 30;
    let fee_result = apply_fee(asol_gross, BASE_FEE_BPS)
      .ok_or(LaminarError::MathOverflow)?;

    asol_net = fee_result.0;
    fee = fee_result.1;

    msg!("Fee: {} aSOL", fee);
    msg!("aSOL net (to user): {}", asol_net);

    require!(asol_net >= min_asol_out, LaminarError::SlippageExceeded);

    new_lst_amount = guard.state.total_lst_amount
      .checked_add(lst_amount)
      .ok_or(LaminarError::MathOverflow)?;

    new_tvl = compute_tvl_sol(new_lst_amount, guard.state.mock_lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    new_asol_supply = guard.state.asol_supply
      .checked_add(asol_gross)
      .ok_or(LaminarError::MathOverflow)?;

    let new_liability = current_liability;
    new_equity = compute_equity_sol(new_tvl, new_liability);

    let new_cr = if new_liability > 0 {
      compute_cr_bps(new_tvl, new_liability)
    } else {
      u64::MAX
    };

    if new_liability > 0 {
      msg!("Post-mint CR: {}bps ({}%)", new_cr, new_cr / 100);
    } else {
      msg!("Post-mint CR: infinite (no debt exists)");
    }

    if new_liability > 0 && new_equity == 0 {
      return Err(LaminarError::InsolventProtocol.into());
    }

    // INVARIANTS
    assert_balance_sheet_holds(new_tvl, new_liability, new_equity)?;

    // EFFECT: Update state atomically
    guard.state.total_lst_amount = new_lst_amount;
    guard.state.asol_supply = new_asol_supply;

  } // lock released

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

  // Mint aSOL to user
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
  #[account(
    mut,
    constraint = asol_mint.mint_authority == anchor_lang::solana_program::program_option::COption::Some(global_state.key()) 
      @ LaminarError::InvalidMintAuthority,
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  /// User's aSOL token account (receives minted aSOL)
  #[account(
    mut,
    token::mint = asol_mint,
    token::authority = user,
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
  )]
  pub user_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Treasury's aSOL token account (receives protocol fees)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = asol_mint,
    associated_token::authority = treasury,
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
  )]
  pub treasury_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// User's LST token account (source of collateral)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (receives LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
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