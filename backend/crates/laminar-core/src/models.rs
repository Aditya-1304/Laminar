use serde::{Deserialize, Serialize};

pub type Lamports = u64;
pub type MicroUsd = u64;
pub type BasisPoints = u64;
pub type Slot = u64;
pub type Epoch = u64;
pub type OperationCounter = u64;

pub type LstBaseUnits = u64;
pub type AmusdBaseUnits = u64;
pub type AsolBaseUnits = u64;
pub type SamusdBaseUnits = u64;

pub type LstToSolRate = u64;
pub type NavLamports = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Address(pub String);

impl Address {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for Address {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Address {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for Address {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleBackend {
    Mock,
    PythPush,
    Other(u8),
}

impl Default for OracleBackend {
    fn default() -> Self {
        Self::Mock
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LstRateBackend {
    Mock,
    SanctumStakePool,
    Other(u8),
}

impl Default for LstRateBackend {
    fn default() -> Self {
        Self::Mock
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalStateModel {
    pub version: u8,
    pub bump: u8,
    pub vault_authority_bump: u8,
    pub operation_counter: OperationCounter,

    pub authority: Address,
    pub amusd_mint: Address,
    pub asol_mint: Address,
    pub treasury: Address,
    pub supported_lst_mint: Address,

    pub total_lst_amount: LstBaseUnits,
    pub amusd_supply: AmusdBaseUnits,
    pub asol_supply: AsolBaseUnits,

    pub min_cr_bps: BasisPoints,
    pub target_cr_bps: BasisPoints,

    pub mint_paused: bool,
    pub redeem_paused: bool,

    pub mock_sol_price_usd: MicroUsd,
    pub mock_lst_to_sol_rate: LstToSolRate,

    pub fee_amusd_mint_bps: BasisPoints,
    pub fee_amusd_redeem_bps: BasisPoints,
    pub fee_asol_mint_bps: BasisPoints,
    pub fee_asol_redeem_bps: BasisPoints,
    pub fee_min_multiplier_bps: BasisPoints,
    pub fee_max_multiplier_bps: BasisPoints,

    pub rounding_reserve_lamports: Lamports,
    pub max_rounding_reserve_lamports: Lamports,

    pub uncertainty_index_bps: BasisPoints,
    pub flash_loan_utilization_bps: BasisPoints,
    pub flash_outstanding_lamports: Lamports,

    pub max_oracle_staleness_slots: Slot,
    pub max_conf_bps: BasisPoints,
    pub uncertainty_max_bps: BasisPoints,

    pub max_lst_stale_epochs: Epoch,
    pub nav_floor_lamports: NavLamports,
    pub max_asol_mint_per_round: AsolBaseUnits,

    pub last_tvl_update_slot: Slot,
    pub last_oracle_update_slot: Slot,
    pub mock_oracle_confidence_usd: MicroUsd,

    pub oracle_backend: OracleBackend,
    pub lst_rate_backend: LstRateBackend,

    pub pyth_sol_usd_price_account: Address,
    pub lst_stake_pool: Address,
    pub last_lst_update_epoch: Epoch,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleSnapshot {
    pub backend: OracleBackend,
    pub price_safe_usd: MicroUsd,
    pub price_redeem_usd: MicroUsd,
    pub price_ema_usd: MicroUsd,
    pub confidence_usd: MicroUsd,
    pub confidence_bps: BasisPoints,
    pub uncertainty_index_bps: BasisPoints,
    pub last_update_slot: Slot,
    pub max_staleness_slots: Slot,
    pub max_conf_bps: BasisPoints,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LstRateSnapshot {
    pub backend: LstRateBackend,
    pub supported_lst_mint: Address,
    pub lst_to_sol_rate: LstToSolRate,
    pub stake_pool: Address,
    pub last_tvl_update_slot: Slot,
    pub last_lst_update_epoch: Epoch,
    pub max_lst_stale_epochs: Epoch,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StabilityPoolSnapshot {
    pub version: u8,
    pub bump: u8,
    pub pool_authority_bump: u8,

    pub global_state: Address,
    pub samusd_mint: Address,
    pub pool_amusd_vault: Address,
    pub pool_asol_vault: Address,

    pub total_amusd: AmusdBaseUnits,
    pub total_asol: AsolBaseUnits,
    pub total_samusd: SamusdBaseUnits,

    pub stability_withdrawals_paused: bool,
    pub last_harvest_lst_to_sol_rate: LstToSolRate,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceSheetSnapshot {
    pub tvl_lamports: Lamports,
    pub liability_lamports: Lamports,
    pub accounting_equity_lamports: i128,
    pub claimable_equity_lamports: Lamports,
    pub collateral_ratio_bps: Option<BasisPoints>,
    pub nav_amusd_lamports: NavLamports,
    pub nav_asol_lamports: Option<NavLamports>,
    pub rounding_reserve_lamports: Lamports,
    pub max_rounding_reserve_lamports: Lamports,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StabilityPoolClaim {
    pub withdrawable_amusd: AmusdBaseUnits,
    pub withdrawable_asol: AsolBaseUnits,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletPosition {
    pub owner: Address,
    pub amusd_balance: AmusdBaseUnits,
    pub asol_balance: AsolBaseUnits,
    pub samusd_balance: SamusdBaseUnits,
    pub stability_pool_claims: Option<StabilityPoolClaim>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionMetadata {
    pub indexed_slot: Option<Slot>,
    pub simulated_slot: Option<Slot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaminarProtocolSnapshot {
    pub global: GlobalStateModel,
    pub oracle: OracleSnapshot,
    pub lst_rate: LstRateSnapshot,
    pub stability_pool: StabilityPoolSnapshot,
    pub balance_sheet: BalanceSheetSnapshot,
    pub metadata: ProjectionMetadata,
}
