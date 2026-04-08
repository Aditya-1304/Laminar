#[allow(unused_imports)]
use anchor_lang::prelude::borsh;
use anchor_lang::{declare_id, prelude::Pubkey, AnchorDeserialize, AnchorSerialize};
use sha2::{Digest, Sha256};

declare_id!("DNJkHdH2tzCG9V8RX2bKRZKHxZccYBkBjqqSsG9midvc");

pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault_authority";
pub const STABILITY_POOL_STATE_SEED: &[u8] = b"stability_pool_state";
pub const STABILITY_POOL_AUTHORITY_SEED: &[u8] = b"stability_pool_authority";

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct GlobalStateWire {
    pub version: u8,
    pub bump: u8,
    pub vault_authority_bump: u8,
    pub operation_counter: u64,
    pub authority: Pubkey,
    pub amusd_mint: Pubkey,
    pub asol_mint: Pubkey,
    pub treasury: Pubkey,
    pub supported_lst_mint: Pubkey,
    pub total_lst_amount: u64,
    pub amusd_supply: u64,
    pub asol_supply: u64,
    pub min_cr_bps: u64,
    pub target_cr_bps: u64,
    pub mint_paused: bool,
    pub redeem_paused: bool,
    pub mock_sol_price_usd: u64,
    pub mock_lst_to_sol_rate: u64,
    pub fee_amusd_mint_bps: u64,
    pub fee_amusd_redeem_bps: u64,
    pub fee_asol_mint_bps: u64,
    pub fee_asol_redeem_bps: u64,
    pub fee_min_multiplier_bps: u64,
    pub fee_max_multiplier_bps: u64,
    pub rounding_reserve_lamports: u64,
    pub max_rounding_reserve_lamports: u64,
    pub uncertainty_index_bps: u64,
    pub flash_loan_utilization_bps: u64,
    pub flash_outstanding_lamports: u64,
    pub max_oracle_staleness_slots: u64,
    pub max_conf_bps: u64,
    pub uncertainty_max_bps: u64,
    pub max_lst_stale_epochs: u64,
    pub nav_floor_lamports: u64,
    pub max_asol_mint_per_round: u64,
    pub last_tvl_update_slot: u64,
    pub last_oracle_update_slot: u64,
    pub mock_oracle_confidence_usd: u64,
    pub oracle_backend: u8,
    pub lst_rate_backend: u8,
    pub pyth_sol_usd_price_account: Pubkey,
    pub lst_stake_pool: Pubkey,
    pub last_lst_update_epoch: u64,
    pub _reserved: [u64; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct StabilityPoolStateWire {
    pub version: u8,
    pub bump: u8,
    pub pool_authority_bump: u8,
    pub global_state: Pubkey,
    pub samusd_mint: Pubkey,
    pub pool_amusd_vault: Pubkey,
    pub pool_asol_vault: Pubkey,
    pub total_amusd: u64,
    pub total_asol: u64,
    pub total_samusd: u64,
    pub withdrawls_paused: bool,
    pub last_harvest_lst_to_sol_rate: u64,
    pub _reserved: [u64; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct MintAmusdArgs {
    pub lst_amount: u64,
    pub min_amusd_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct RedeemAmusdArgs {
    pub amusd_amount: u64,
    pub min_lst_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct MintAsolArgs {
    pub lst_amount: u64,
    pub min_asol_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct RedeemAsolArgs {
    pub asol_amount: u64,
    pub min_lst_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct DepositAmusdArgs {
    pub amusd_amount: u64,
    pub min_samusd_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawUnderlyingArgs {
    pub samusd_amount: u64,
    pub min_amusd_out: u64,
    pub min_asol_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct InitializeArgs {
    pub min_cr_bps: u64,
    pub target_cr_bps: u64,
    pub mock_sol_price_usd: u64,
    pub mock_lst_to_sol_rate: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct EmergencyPauseArgs {
    pub mint_paused: bool,
    pub redeem_paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct UpdateParametersArgs {
    pub new_min_cr_bps: u64,
    pub new_target_cr_bps: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct SetOracleSourcesArgs {
    pub oracle_backend: u8,
    pub pyth_sol_usd_price_account: Pubkey,
    pub lst_rate_backend: u8,
    pub lst_stake_pool: Pubkey,
}

#[derive(Debug, Clone, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct SetStabilityWithdrawalsPausedArgs {
    pub withdrawals_paused: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("failed to serialize instruction args for `{ix_name}`")]
    SerializeFailed { ix_name: &'static str },
}

pub fn anchor_instruction_discriminator(ix_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(b"global:");
    hasher.update(ix_name.as_bytes());

    let hash = hasher.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&hash[..8]);
    out
}

pub fn build_anchor_instruction_data<T: AnchorSerialize>(
    ix_name: &'static str,
    args: &T,
) -> std::result::Result<Vec<u8>, WireError> {
    let mut data = anchor_instruction_discriminator(ix_name).to_vec();
    args.serialize(&mut data)
        .map_err(|_| WireError::SerializeFailed { ix_name })?;
    Ok(data)
}

pub fn build_anchor_instruction_data_no_args(ix_name: &str) -> Vec<u8> {
    anchor_instruction_discriminator(ix_name).to_vec()
}
