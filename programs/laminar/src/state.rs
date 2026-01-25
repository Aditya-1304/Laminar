//! State accounts for Laminar protocol
//! These accounts hold the global balance sheet and vault configuration

use anchor_lang::prelude::*;

/// Global protocol state - the single sorce of truth for the balance sheet and vault configuration
/// This account is a singleton (only one exists per protocol deployment)

#[account]
pub struct GlobalState {
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

  /// Collateral held by protocol in lamports
  /// This is the TVL in sol based units
  pub total_collateral_lamports: u64,

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

  pub mock_sol_price_usd: u64,

  pub mock_lst_to_sol_rate: u64,

  pub _reserved: [u64; 3],
}

impl GlobalState {
  pub const LEN: usize = 8 + // discrimanator
    32 + // authority
    32 + // amusd_mint
    32 + // asol_mint
    32 + // treasury
    32 + // supported_lst_mint
    8 + // total_collateral_lamports
    8 + // amusd_supply
    8 + // asol_supply
    8 + // min_cr_bps
    8 + // target_cr_bps
    1 + // mint_paused
    1 + // redeem_paused
    8 + // mock_sol_price_usd
    8 + // mock_lst_to_sol_rate
    24; // _reserved (3 * 8 = 24)
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