use anchor_lang::prelude::*;

#[event]
pub struct ProtocolInitialized {
  pub authority: Pubkey,
  pub amusd_mint: Pubkey,
  pub asol_mint: Pubkey,
  pub supported_lst_mint: Pubkey,
  pub min_cr_bps: u64,
  pub target_cr_bps: u64,
  pub timestamp: i64,
}

#[event]
pub struct AmUSDMinted {
  pub user: Pubkey,
  pub lst_deposited: u64,
  pub amusd_minted: u64,
  pub fee: u64,
  pub old_tvl: u64,
  pub new_tvl: u64,
  pub old_cr_bps: u64,
  pub new_cr_bps: u64,
  pub sol_price_used: u64,
  pub timestamp: i64,
}


#[event]
pub struct AmUSDRedeemed {
  pub user: Pubkey,
  pub amusd_burned: u64,
  pub lst_received: u64,
  pub fee: u64,
  pub old_tvl: u64,
  pub new_tvl: u64,
  pub old_cr_bps: u64,
  pub new_cr_bps: u64,
  pub sol_price_used: u64,
  pub timestamp: i64,
}

#[event]
pub struct AsolMinted {
  pub user: Pubkey,
  pub lst_deposited: u64,
  pub asol_minted: u64,
  pub fee: u64,
  pub nav: u64,
  pub old_tvl: u64,
  pub new_tvl: u64,
  pub old_equity: u64,
  pub new_equity: u64,
  pub leverage_multiple: u64,
  pub timestamp: i64,
}

#[event]
pub struct AsolRedeemed {
  pub user: Pubkey,
  pub asol_burned: u64,
  pub lst_received: u64,
  pub fee: u64,
  pub nav: u64,
  pub old_tvl: u64,
  pub new_tvl: u64,
  pub old_equity: u64,
  pub new_equity: u64,
  pub timestamp: i64,
}

#[event]
pub struct EmergencyPause {
  pub authority: Pubkey,
  pub mint_paused: bool,
  pub redeem_paused: bool,
  pub timestamp: i64,
}

#[event]
pub struct OraclePriceUpdated {
  pub authority: Pubkey,
  pub old_sol_price: u64,
  pub new_sol_price: u64,
  pub old_lst_rate: u64,
  pub new_lst_rate: u64,
  pub timestamp: i64,
}

#[event]
pub struct ParametersUpdated {
  pub authority: Pubkey,
  pub old_min_cr_bps: u64,
  pub new_min_cr_bps: u64,
  pub old_target_cr_bps: u64,
  pub new_target_cr_bps: u64,
  pub timestamp: i64,
}

#[event]
pub struct OracleConfigUpdated {
  pub authority: Pubkey,
  pub oracle_backend: u8,
  pub pyth_sol_usd_price_account: Pubkey,
  pub lst_rate_backend: u8,
  pub lst_stake_pool: Pubkey,
  pub timestamp: i64,
}

#[event]
pub struct OracleSnapshotUpdated {
  pub updater: Pubkey,
  pub oracle_backend: u8,
  pub ema_price_usd: u64,
  pub safe_price_usd: u64,
  pub confidence_usd: u64,
  pub uncertainty_index_bps: u64,
  pub slot: u64,
  pub timestamp: i64,
}

#[event]
pub struct SafePriceQuoted {
  pub requester: Pubkey,
  pub oracle_backend: u8,
  pub ema_price_usd: u64,
  pub safe_price_usd: u64,
  pub confidence_usd: u64,
  pub uncertainty_index_bps: u64,
  pub slot: u64,
  pub timestamp: i64,
}

#[event]
pub struct StabilityPoolInitialized {
  pub authority: Pubkey,
  pub samusd_mint: Pubkey,
  pub pool_amusd_vault: Pubkey,
  pub pool_asol_vault: Pubkey,
  pub timestamp: i64,
}

#[event]
pub struct StabilityPoolDeposited {
  pub user: Pubkey,
  pub amusd_in: u64,
  pub samusd_minted: u64,
  pub total_amusd: u64,
  pub total_asol: u64,
  pub total_samusd: u64,
  pub timestamp: i64,
}

#[event]
pub struct StabilityPoolWithdrawn {
  pub user: Pubkey,
  pub samusd_burned: u64,
  pub amusd_out : u64,
  pub asol_out: u64,
  pub total_amusd: u64,
  pub total_asol: u64,
  pub total_samusd: u64,
  pub timestamp: i64,
}

#[event]
pub struct StabilityYieldHarvested {
  pub harvester: Pubkey,
  pub old_rate: u64,
  pub new_rate: u64,
  pub yield_delta_sol: u64,
  pub amsud_minted: u64,
  pub negative_yield: bool,
  pub total_amusd: u64,
  pub timestamp: i64,
}

#[event]
pub struct DebtEquitySwapExecuted {
  pub executor: Pubkey,
  pub amusd_burned: u64,
  pub asol_minted: u64,
  pub nav_conv: u64,
  pub cr_befrore_bps: u64,
  pub cr_after_bps: u64,
  pub price_safe_usd: u64,
  pub pool_amusd_after: u64,
  pub pool_asol_after: u64,
  pub timestamp: i64,
}