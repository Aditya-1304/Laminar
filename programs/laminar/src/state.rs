//! State accounts for Laminar protocol
//! These accounts hold the global balance sheet and vault configuration

use anchor_lang::prelude::*;

/// Global protocol state - the single sorce of truth for the balance sheet and vault configuration
/// This account is a singleton (only one exists per protocol deployment)

#[account]
pub struct GlobalState {
  /// Protocol Authority (Admin)
  pub authority: Pubkey,

  pub amusd_mint: Pubkey,

  pub asol_mint: Pubkey,

  pub total_collateral_lamports: u64,

  pub amusd_supply: u64,

  pub asol_supply: u64,

  pub min_cr_bps: u64,

  pub target_cr_bps: u64,

  pub mint_paused: bool,

  pub redeem_paused: bool,

  pub mock_sol_price_usd: u64,

  pub mock_lst_to_sol_rate: u64,

  pub reserved: [u64; 8],
}

impl GlobalState {
  pub const LEN: usize = 8 + // discrimanator
    32 + // authority
    32 + // amusd_mint
    32 + // asol_mint
    8 + // total_collateral_lamports
    8 + // amusd_supply
    8 + // asol_supply
    8 + // min_cr_bps
    8 + // target_cr_bps
    1 + // mint_paused
    1 + // redeem_paused
    8 + // mock_sol_price_usd
    8 + // mock_lst_to_sol_rate
    64; // _reserved
}

/// Collateral vault - holds LST tokens
/// One vault exists per whitelisted LST type

#[account]
pub struct CollateralVault {
  pub lst_mint: Pubkey,

  pub vault_authority: Pubkey,

  pub bump: u8,

  pub _reserved: [u64; 8],
}

impl CollateralVault {
  pub const LEN: usize = 8 +
    32 +
    32 +
    1 +
    64;
}

pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";

pub const VAULT_SEED: &[u8] = b"vault";

pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault_authority";