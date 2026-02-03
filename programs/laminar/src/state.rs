//! State accounts for Laminar protocol
//! These accounts hold the global balance sheet and vault configuration

use anchor_lang::prelude::*;

use crate::error::LaminarError;

/// Global protocol state - the single source of truth for the balance sheet and vault configuration
/// This account is a singleton (only one exists per protocol deployment)

#[account]
pub struct GlobalState {
  /// Protocol version for upgrades
  pub version: u8,

  /// Bump seed for this PDA
  pub bump: u8,

  pub vault_authority_bump: u8,

  /// Operation counter - increments on every state change (for debugging/tracing)
  pub operation_counter: u64,

  /// Protocol Authority (Admin)
  pub authority: Pubkey,

  /// SPL token mint for amUSD (senior tranche)
  pub amusd_mint: Pubkey,

  /// SPL token mint for aSOL (junior tranche)
  pub asol_mint: Pubkey,

  /// Treasury wallet - recieves protocol fees
  pub treasury: Pubkey,

  /// Whitelisted LST mint (e.g., jitoSOL, mSOL)
  /// Only this LST can be deposited as collateral
  pub supported_lst_mint: Pubkey,

  /// Total LST tokens held by protocol (raw LST units, NOT SOL-converted)
  /// Use compute_tvl_sol() to get SOL-denominated value
  pub total_lst_amount: u64,

  /// Total amUSD supply (with USD_PRECISION = 1e6)
  /// Represent total dollar-dominated debt
  pub amusd_supply: u64,

  /// Total aSOL supply (with SOL_PRECISION = 1e9)
  /// Represent total equity shares
  pub asol_supply: u64,

  /// Minimum collateral ratio in basis points (e.g., 13000 = 130%)
  /// Protocol will reject amUSD mints that would drop CR below this threshold 
  pub min_cr_bps: u64,

  /// Target collateral ratio in basis points (e.g., 15000 = 150%)
  /// Used for fee skewing and risk signaling
  pub target_cr_bps: u64,

  /// Emergency pause for amUSD minting
  pub mint_paused: bool,

  /// Emergency pause for redemptions
  pub redeem_paused: bool,

  /// Reentrancy lock (solana CPI safety)
  // pub locked: bool,

  pub mock_sol_price_usd: u64,

  pub mock_lst_to_sol_rate: u64,
  
  pub _reserved: [u64; 2],
}

impl GlobalState {
  pub const LEN: usize = 8 + // discrimanator
    1 + // version
    1 + // bump
    1 + // vault_authoruty_bump
    8 + // operation_counter
    32 + // authority
    32 + // amusd_mint
    32 + // asol_mint
    32 + // treasury
    32 + // supported_lst_mint
    8 + // total_lst_amount
    8 + // amusd_supply
    8 + // asol_supply
    8 + // min_cr_bps
    8 + // target_cr_bps
    1 + // mint_paused
    1 + // redeem_paused
    // 1 + // locked
    8 + // mock_sol_price_usd
    8 + // mock_lst_to_sol_rate
    16; // _reserved (2 * 8 = 16)
}

/// Collateral vault metadata - holds LST vault configuration
/// 
/// TODO: FUTURE IMPLEMENTATION
/// Currently unused in MVP-0 (single vault design).
/// This struct will be activated when multi-LST support is added.
/// For now, vault metadata is stored directly in GlobalState.
///
/// One vault account will exist per whitelisted LST type.

#[account]
pub struct CollateralVault {
  /// LST mint that this vault holds
  pub lst_mint: Pubkey,

  /// Vault authority (PDA) - signs transfers from vault
  pub vault_authority: Pubkey,

  /// Bump seed for vault_authority PDA
  pub bump: u8,

  /// Reserved space for future upgrades
  pub _reserved: [u64; 8],
}

impl CollateralVault {
  pub const LEN: usize = 8 + // discriminator
    32 + // lst_mint
    32 + // vault_authority
    1 + // bump
    64; // _reserved
}

pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";

pub const VAULT_SEED: &[u8] = b"vault";

pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault_authority";

pub const CURRENT_VERSION: u8 = 1;

impl GlobalState {
  pub fn validate_version(&self) -> Result<()> {
    require!(
      self.version == CURRENT_VERSION,
      LaminarError::InvalidVersion
    );
    Ok(())
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  use anchor_lang::prelude::borsh;
  
  #[test]
  fn test_global_state_size() {
    // Create a default instance and serialize it to verify size
    let state = GlobalState {
      version: 0,
      bump: 0,
      vault_authority_bump: 0,
      operation_counter: 0,
      authority: Pubkey::default(),
      amusd_mint: Pubkey::default(),
      asol_mint: Pubkey::default(),
      treasury: Pubkey::default(),
      supported_lst_mint: Pubkey::default(),
      total_lst_amount: 0,
      amusd_supply: 0,
      asol_supply: 0,
      min_cr_bps: 0,
      target_cr_bps: 0,
      mint_paused: false,
      redeem_paused: false,
      mock_sol_price_usd: 0,
      mock_lst_to_sol_rate: 0,
      _reserved: [0; 2],
    };
    
    // Verify the manual LEN calculation matches what Borsh would serialize
    // The actual serialized size should be LEN - 8 (discriminator is added by Anchor)
    let serialized = borsh::to_vec(&state).expect("Failed to serialize");
    assert_eq!(
      GlobalState::LEN,
      8 + serialized.len(),
      "GlobalState::LEN ({}) doesn't match 8 + serialized size (8 + {})",
      GlobalState::LEN,
      serialized.len()
    );
  }
}