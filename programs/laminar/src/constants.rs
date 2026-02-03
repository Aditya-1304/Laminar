//! Protocol-wide constants
//! Centralized location for all configuration values

// PRECISION CONSTANTS
pub const SOL_PRECISION: u64 = 1_000_000_000;  // 1e9 lamports
pub const USD_PRECISION: u64 = 1_000_000;       // 1e6 (6 decimals)
pub const BPS_PRECISION: u64 = 10_000;          // 100% = 10000 bps

// MINIMUM AMOUNTS 
pub const MIN_LST_DEPOSIT: u64 = 100_000;       // 0.0001 SOL (100k lamports)
pub const MIN_AMUSD_MINT: u64 = 1_000;          // 0.001 USD (1k micro-USD)
pub const MIN_ASOL_MINT: u64 = 1_000_000;       // 0.001 SOL (1M lamports)
pub const MIN_PROTOCOL_TVL: u64 = 1_000_000;    // 0.001 SOL minimum TVL
pub const MIN_NAV_LAMPORTS: u64 = 1_000;        // Minimum NAV for safe operations

// FEE CONFIGURATION 
pub const AMUSD_MINT_FEE_BPS: u64 = 50;         // 0.5%
pub const AMUSD_REDEEM_FEE_BPS: u64 = 25;       // 0.25%
pub const ASOL_MINT_FEE_BPS: u64 = 30;          // 0.3%
pub const ASOL_REDEEM_FEE_BPS: u64 = 15;        // 0.15%

// Dynamic fee multiplier cap when CR < target (1x = 10_000 bps)
pub const MAX_FEE_MULTIPLIER_BPS: u64 = 40_000; // 4x max

// SLIPPAGE LIMITS 
pub const MAX_SLIPPAGE_BPS: u64 = 500;          // 5% max slippage

// RISK PARAMETERS 
pub const DEFAULT_MIN_CR_BPS: u64 = 13_000;     // 130%
pub const DEFAULT_TARGET_CR_BPS: u64 = 15_000;  // 150%

pub const MIN_TOLERANCE: u64 = 1_000;
pub const TOLERANCE_BPS: u64 = 1;
