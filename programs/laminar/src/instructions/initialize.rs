//! Initialize instruction - sets up the protocol
//! Creates GlobalState, token mints, and collateral vault

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface}};
use crate::state::*;
// use crate::math::{SOL_PRECISION, USD_PRECISION};

pub fn handler(
  ctx: Context<Initialize>,
  min_cr_bps: u64,
  target_cr_bps: u64,
  mock_sol_price_usd: u64,
  mock_lst_to_sol_rate: u64,
) -> Result<()> {
  let global_state = &mut ctx.accounts.global_state;

  global_state.authority = ctx.accounts.authority.key();
  global_state.amusd_mint = ctx.accounts.amusd_mint.key();
  global_state.asol_mint = ctx.accounts.asol_mint.key();

  global_state.treasury = ctx.accounts.authority.key();

  global_state.supported_lst_mint = ctx.accounts.lst_mint.key();

  global_state.total_collateral_lamports = 0;
  global_state.amusd_supply = 0;
  global_state.asol_supply = 0;

  global_state.min_cr_bps = min_cr_bps;
  global_state.target_cr_bps = target_cr_bps;

  global_state.mint_paused = false;
  global_state.redeem_paused = false;

  global_state.mock_sol_price_usd = mock_sol_price_usd;
  global_state.mock_lst_to_sol_rate = mock_lst_to_sol_rate;

  global_state._reserved = [0; 3];

  msg!("Protocol initialized!");
  msg!("amUSD mint: {}", global_state.amusd_mint);
  msg!("aSOL mint: {}", global_state.asol_mint);
  msg!("Supported LST: {}", global_state.supported_lst_mint);
  msg!("Min CR: {}bps", min_cr_bps);
  msg!("Target CR: {}bps", target_cr_bps);

  Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
  #[account(mut)]
  pub authority: Signer<'info>,

  /// GlobalState PDA - stores the balance sheet
  #[account(
    init,
    payer = authority,
    space = GlobalState::LEN,
    seeds = [GLOBAL_STATE_SEED],
    bump
  )]
  pub global_state: Account<'info, GlobalState>,

  /// amUSD token mint (senior tranche)
  #[account(
    init,
    payer = authority,
    mint::decimals = 6, // USD_PRECISION = 1e6
    mint::authority = global_state,
    mint::token_program = token_program
  )]
  pub amusd_mint: InterfaceAccount<'info, Mint>,

  /// aSOL token mint (Junior tranche)
  #[account(
    init,
    payer = authority,
    mint::decimals = 9,
    mint::authority = global_state,
    mint::token_program = token_program,
  )]
  pub asol_mint: InterfaceAccount<'info, Mint>,

  /// Collateral vault - holds LST tokens
  /// Deterministic ATA owned by vault_authority PDA
  #[account(
    init,
    payer = authority,
    associated_token::mint = lst_mint,
    associated_token::authority = vault_authority,
  )]
  pub vault: InterfaceAccount<'info, TokenAccount>,

  /// The LST mint being used as collateral (e.g., JitoSOL, mSOL)
  pub lst_mint: InterfaceAccount<'info, Mint>,

  /// CHECK: PDA will be validated by the seeds
  #[account(
    seeds = [VAULT_AUTHORITY_SEED],
    bump
  )]
  pub vault_authority: UncheckedAccount<'info>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,

}