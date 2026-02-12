//! Initialize instruction - sets up the protocol
//! Creates GlobalState, token mints, and collateral vault

use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, TokenAccount, TokenInterface}};
use crate::{constants::{AMUSD_MINT_FEE_BPS, AMUSD_REDEEM_FEE_BPS, ASOL_MINT_FEE_BPS, ASOL_REDEEM_FEE_BPS, DEFAULT_FEE_MAX_MULTIPLIER_BPS, DEFAULT_FEE_MIN_MULTIPLIER_BPS, DEFAULT_MAX_ASOL_MINT_PER_ROUND, DEFAULT_MAX_CONF_BPS, DEFAULT_MAX_LST_STALE_EPOCHS, DEFAULT_MAX_ORACLE_STALENESS_SLOTS, DEFAULT_NAV_FLOOR_LAMPORTS, DEFAULT_UNCERTAINTY_MAX_BPS}, error::LaminarError, state::*};
use crate::math::{SOL_PRECISION};
use crate::constants::DEFAULT_MAX_ROUNDING_RESERVE_LAMPORTS;

pub fn handler(
  ctx: Context<Initialize>,
  min_cr_bps: u64,
  target_cr_bps: u64,
  mock_sol_price_usd: u64,
  mock_lst_to_sol_rate: u64,
) -> Result<()> {
  require!(min_cr_bps >= 10_000, LaminarError::InvalidParameter);  // Min 100%
  require!(target_cr_bps > min_cr_bps, LaminarError::InvalidParameter);
  require!(mock_sol_price_usd > 0, LaminarError::ZeroAmount);
  require!(mock_lst_to_sol_rate > 0, LaminarError::ZeroAmount);
  require!(mock_lst_to_sol_rate >= SOL_PRECISION / 2, LaminarError::InvalidParameter);  // LST can't be worth less than half SOL


  // Validate LST decimals  
  require!(
    ctx.accounts.lst_mint.decimals == 9,
    LaminarError::InvalidDecimals
  );
  
  let global_state = &mut ctx.accounts.global_state;

  global_state.version = 1;
  global_state.bump = ctx.bumps.global_state;
  global_state.vault_authority_bump = ctx.bumps.vault_authority;
  global_state.operation_counter = 0;
  global_state.authority = ctx.accounts.authority.key();
  global_state.amusd_mint = ctx.accounts.amusd_mint.key();
  global_state.asol_mint = ctx.accounts.asol_mint.key();

  global_state.treasury = ctx.accounts.authority.key();

  global_state.supported_lst_mint = ctx.accounts.lst_mint.key();

  global_state.total_lst_amount = 0;
  global_state.amusd_supply = 0;
  global_state.asol_supply = 0;

  global_state.min_cr_bps = min_cr_bps;
  global_state.target_cr_bps = target_cr_bps;

  global_state.mint_paused = false;
  global_state.redeem_paused = false;

  // global_state.locked = false;

  global_state.mock_sol_price_usd = mock_sol_price_usd;
  global_state.mock_lst_to_sol_rate = mock_lst_to_sol_rate;
  global_state.rounding_reserve_lamports = 0;
  global_state.max_rounding_reserve_lamports = DEFAULT_MAX_ROUNDING_RESERVE_LAMPORTS;

  global_state.fee_amusd_mint_bps = AMUSD_MINT_FEE_BPS;
  global_state.fee_amusd_redeem_bps = AMUSD_REDEEM_FEE_BPS;
  global_state.fee_asol_mint_bps = ASOL_MINT_FEE_BPS;
  global_state.fee_asol_redeem_bps = ASOL_REDEEM_FEE_BPS;
  global_state.fee_min_multiplier_bps = DEFAULT_FEE_MIN_MULTIPLIER_BPS;
  global_state.fee_max_multiplier_bps = DEFAULT_FEE_MAX_MULTIPLIER_BPS;

  global_state.uncertainty_index_bps = 0;
  global_state.flash_loan_utilization_bps = 0;
  global_state.flash_outstanding_lamports = 0;
  global_state.max_oracle_staleness_slots = DEFAULT_MAX_ORACLE_STALENESS_SLOTS;
  global_state.max_conf_bps = DEFAULT_MAX_CONF_BPS;
  global_state.uncertainty_max_bps = DEFAULT_UNCERTAINTY_MAX_BPS;
  global_state.max_lst_stale_epochs = DEFAULT_MAX_LST_STALE_EPOCHS;
  global_state.nav_floor_lamports = DEFAULT_NAV_FLOOR_LAMPORTS;
  global_state.max_asol_mint_per_round = DEFAULT_MAX_ASOL_MINT_PER_ROUND;
  global_state.last_tvl_update_slot = ctx.accounts.clock.slot;
  global_state.last_oracle_update_slot = ctx.accounts.clock.slot;

  global_state._reserved = [0; 2];

  msg!("Protocol initialized!");
  msg!("amUSD mint: {}", global_state.amusd_mint);
  msg!("aSOL mint: {}", global_state.asol_mint);
  msg!("Supported LST: {}", global_state.supported_lst_mint);
  msg!("Min CR: {}bps", min_cr_bps);
  msg!("Target CR: {}bps", target_cr_bps);

  emit!(crate::events::ProtocolInitialized {
    authority: ctx.accounts.authority.key(),
    amusd_mint: global_state.amusd_mint,
    asol_mint: global_state.asol_mint,
    supported_lst_mint: global_state.supported_lst_mint,
    min_cr_bps,
    target_cr_bps,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

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
  pub global_state: Box<Account<'info, GlobalState>>,

  /// amUSD token mint (senior tranche)
  #[account(
    init,
    payer = authority,
    mint::decimals = 6, // USD_PRECISION = 1e6
    mint::authority = global_state,
    mint::freeze_authority = global_state,
    mint::token_program = token_program
  )]
  pub amusd_mint: Box<InterfaceAccount<'info, Mint>>,

  /// aSOL token mint (Junior tranche)
  #[account(
    init,
    payer = authority,
    mint::decimals = 9,
    mint::authority = global_state,
    mint::freeze_authority = global_state,
    mint::token_program = token_program,
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  /// Collateral vault - holds LST tokens
  /// Deterministic ATA owned by vault_authority PDA
  #[account(
    init,
    payer = authority,
    associated_token::mint = lst_mint,
    associated_token::authority = vault_authority,
    associated_token::token_program = token_program,
  )]
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

  /// The LST mint being used as collateral (e.g., JitoSOL, mSOL)
  pub lst_mint: Box<InterfaceAccount<'info, Mint>>,

  /// CHECK: PDA will be validated by the seeds
  #[account(
    seeds = [VAULT_AUTHORITY_SEED],
    bump
  )]
  pub vault_authority: UncheckedAccount<'info>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,

  pub clock: Sysvar<'info, Clock>,
}