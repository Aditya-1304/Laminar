//! Redeem amUSD instruction - exits stable debt position
//! User burns amUSD and receives LST collateral back

use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, Burn}
};
use crate::{events::AmUSDRedeemed, reentrancy::ReentrancyGuard, state::*};
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;

pub fn handler(
  ctx: Context<RedeemAmUSD>,
  amusd_amount: u64,
  min_lst_out: u64,
) -> Result<()> {

  let new_lst_amount: u64;
  let new_amusd_supply: u64;
  let lst_net: u64;
  let lst_fee: u64;
  let new_tvl: u64;
  let new_cr: u64;

  {
    // lock acquired
    let mut guard = ReentrancyGuard::new(&mut ctx.accounts.global_state)?;

    // Validations
    require!(!guard.state.redeem_paused, LaminarError::RedeemPaused);
    require!(amusd_amount > 0, LaminarError::ZeroAmount);

    msg!("amUSD to redeem: {}", amusd_amount);

    // math logic
    let sol_value_gross = mul_div_down(amusd_amount, SOL_PRECISION, guard.state.mock_sol_price_usd)
      .ok_or(LaminarError::MathOverflow)?;

    msg!("SOL value (before fee): {}", sol_value_gross);

    let lst_gross = mul_div_down(sol_value_gross, SOL_PRECISION, guard.state.mock_lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    const REDEEM_FEE_BPS: u64 = 25;
    let fee_result = apply_fee(lst_gross, REDEEM_FEE_BPS)
      .ok_or(LaminarError::MathOverflow)?;

    lst_net = fee_result.0;
    lst_fee = fee_result.1;

    msg!("LST gross: {}", lst_gross);
    msg!("LST to user: {}", lst_net);
    msg!("LST fee to treasury: {}", lst_fee);

    require!(lst_net >= min_lst_out, LaminarError::SlippageExceeded);

    let total_lst_out = lst_gross;

    new_lst_amount = guard.state.total_lst_amount
      .checked_sub(total_lst_out)
      .ok_or(LaminarError::InsufficientCollateral)?;

    new_tvl = compute_tvl_sol(new_lst_amount, guard.state.mock_lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    new_amusd_supply = guard.state.amusd_supply
      .checked_sub(amusd_amount)
      .ok_or(LaminarError::InsufficientSupply)?;

    let new_liability = if new_amusd_supply > 0 {
      compute_liability_sol(new_amusd_supply, guard.state.mock_sol_price_usd)
        .ok_or(LaminarError::MathOverflow)?
    } else {
      0
    };

    let new_equity = compute_equity_sol(new_tvl, new_liability);

    new_cr = if new_amusd_supply > 0 {
      let cr = compute_cr_bps(new_tvl, new_liability);
      msg!("Post-redeem CR: {}bps ({}%)", cr, cr / 100);
      cr
    } else {
      msg!("All amUSD redeemed - CR check skipped");
      u64::MAX
    };

    require!(
      ctx.accounts.vault.amount >= total_lst_out,
      LaminarError::InsufficientCollateral
    );

    // Invariants check
    assert_balance_sheet_holds(new_tvl, new_liability, new_equity)?;

    // Update state atomically
    guard.state.total_lst_amount = new_lst_amount;
    guard.state.amusd_supply = new_amusd_supply;

  } // lock released

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

  msg!("Redeem complete!");
  msg!("New TVL: {} lamports", new_tvl);
  msg!("New amUSD supply: {}", new_amusd_supply);

  emit!(AmUSDRedeemed {
    user: ctx.accounts.user.key(),
    amusd_burned: amusd_amount,
    lst_received: lst_net,
    fee: lst_fee,
    new_tvl,
    new_cr_bps: new_cr,
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
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
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
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
  )]
  pub treasury_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// User's LST token account (receives redeemed LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (source of LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
    constraint = user_lst_account.close_authority == 
      anchor_lang::solana_program::program_option::COption::None 
      @ LaminarError::InvalidAccountState,
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
}