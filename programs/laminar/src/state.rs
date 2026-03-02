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

  pub fee_amusd_mint_bps: u64,

  pub fee_amusd_redeem_bps: u64,

  pub fee_asol_mint_bps: u64,

  pub fee_asol_redeem_bps: u64,

  pub fee_min_multiplier_bps: u64,

  pub fee_max_multiplier_bps: u64,

  /// Non-claimable reserve used to absorb determinsitic fixed-point dust.
  pub rounding_reserve_lamports: u64,

  /// Hard cap on rounding reserve growth.
  /// If exceeded, instructions must revert until governance reconcilliation.
  pub max_rounding_reserve_lamports: u64,

  /// Uncertainty signal derived from oracle confidence.
  pub uncertainty_index_bps: u64,

  /// Current flash-loan utilization signal.
  pub flash_loan_utilization_bps: u64,

  /// Flash liquidity currently out during router operations.
  pub flash_outstanding_lamports: u64,

  /// Oracle freshness bound in slots.
  pub max_oracle_staleness_slots: u64,

  /// Oracle confidence bound in bps.
  pub max_conf_bps: u64,

  /// Cap on uncertainty multiplier.
  pub uncertainty_max_bps: u64,

  /// LST staleness bound in epochs.
  pub max_lst_stale_epochs: u64,

  /// NAV floor for conversion safety.
  pub nav_floor_lamports: u64,

  /// Per-round cap on aSOL mint during conversion paths.
  pub max_asol_mint_per_round: u64,

  /// Last slot when cached TVL/LST rate was refreshed.
  pub last_tvl_update_slot: u64,

  /// Last slot when oracle inputs were refreshed.
  pub last_oracle_update_slot: u64,

  pub mock_oracle_confidence_usd: u64,

  /// Oracle backend selector
  /// 0 = mock cache, 1 = pyth push EMA account
  pub oracle_backend: u8,

  /// LST rate backend selector
  /// 0 = mock cache, 1 = Sanctum-style stake-pool intrinsic pricing
  pub lst_rate_backend: u8,

  /// Configured Pyth SOL/USD price account (used when oracle_backend = 1)
  pub pyth_sol_usd_price_account: Pubkey,

  /// Configured stake-pool account for supported LST (used when lst_rate_backend = 1).
  pub lst_stake_pool: Pubkey,

  /// Last epoch when LST exchange rate was refreshed
  pub last_lst_update_epoch: u64,

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
    8 + // fee_amusd_mint_bps
    8 + // fee_amusd_redeem_bps
    8 + // fee_asol_mint_bps
    8 + // fee_asol_redeem_bps
    8 + // fee_min_multiplier_bps
    8 + // fee_max_mutliplier_bps
    8 + // rounding_reserve_lamports
    8 + //max_rounding_reserve_lamports
    8 + // uncertainity_index_bps
    8 + // flash_loan_utilization_bps
    8 + // flash_outstanding_lamports
    8 + // max_oracle_staleness_slots
    8 + // max_conf_bps
    8 + // uncertainity_max_bos
    8 + // max_lst_stale_epochs
    8 + // nav_floor_lamports
    8 + // max_asol_mint_per_round
    8 + // last_tvl_update_slot
    8 + // last_oracle_update_slot
    8 + // mock_oracle_confidence_usd
    1 + // oracle_backend
    1 + // lst_rate_backend
    32 + // pyth_sol_usd_price_account
    32 + // lst_stake_pool
    8 + // last_lst_update_epoch
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

#[account]
pub struct StabilityPoolState {
  /// Protocol version for migrations
  pub version: u8,

  /// PDA bump for this account
  pub bump: u8,

  /// PDA bump for stability pool authority signer
  pub pool_authority_bump: u8,

  /// Associated GlobalState singleton
  pub global_state: Pubkey,

  /// s_amUSD receipt mint
  pub samusd_mint: Pubkey,

  /// pool amusd vault ATA (this will be owned by stability pool authority PDA)
  pub pool_amusd_vault: Pubkey,

  /// Pool aSOL vault ATA (this will be owned by stability pool authority PDA)
  pub pool_asol_vault: Pubkey,

  /// A = total amUSD held by the pool
  pub total_amusd: u64,

  /// J = total aSOL held by the pool
  pub total_asol: u64,

  /// S = total s_amUSD supply tracked by state
  pub total_samusd: u64,

  /// Emergency circuit breaker (governance controls this)
  pub withdrawls_paused: bool,

  /// Last harvested LST->SOL rate snapshot used by harvest_yield
  pub last_harvest_lst_to_sol_rate: u64,

  pub _reserved: [u64; 4],
}

impl StabilityPoolState {
  pub const LEN: usize =
    8 +   // discriminator
    1 +   // version
    1 +   // bump
    1 +   // pool_authority_bump
    32 +  // global_state
    32 +  // samusd_mint
    32 +  // pool_amusd_vault
    32 +  // pool_asol_vault
    8 +   // total_amusd
    8 +   // total_asol
    8 +   // total_samusd
    1 +   // withdrawals_paused
    8 +   // last_harvest_lst_to_sol_rate
    32;   // _reserved

  pub fn validate_version(&self) -> Result<()> {
    require!(self.version == CURRENT_VERSION, LaminarError::InvalidVersion);
    Ok(())
  }
}

pub const STABILITY_POOL_STATE_SEED: &[u8] = b"stability_pool_state";
pub const STABILITY_POOL_AUTHORITY_SEED: &[u8] = b"stability_pool_authority";

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
    fee_amusd_mint_bps: 0,
    fee_amusd_redeem_bps: 0,
    fee_asol_mint_bps: 0,
    fee_asol_redeem_bps: 0,
    fee_min_multiplier_bps: 0,
    fee_max_multiplier_bps: 0,
    rounding_reserve_lamports: 0,
    max_rounding_reserve_lamports: 0,
    uncertainty_index_bps: 0,
    flash_loan_utilization_bps: 0,
    flash_outstanding_lamports: 0,
    max_oracle_staleness_slots: 0,
    max_conf_bps: 0,
    uncertainty_max_bps: 0,
    max_lst_stale_epochs: 0,
    nav_floor_lamports: 0,
    max_asol_mint_per_round: 0,
    last_tvl_update_slot: 0,
    last_oracle_update_slot: 0,
    mock_oracle_confidence_usd: 0,
    oracle_backend: 0,
    lst_rate_backend: 0,
    pyth_sol_usd_price_account: Pubkey::default(),
    lst_stake_pool: Pubkey::default(),
    last_lst_update_epoch: 0,
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