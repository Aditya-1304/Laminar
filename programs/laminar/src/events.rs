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
