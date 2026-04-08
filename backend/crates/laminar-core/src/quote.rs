use serde::{Deserialize, Serialize};

use crate::{
    models::{
        Address, AmusdBaseUnits, AsolBaseUnits, BasisPoints, Lamports, LstBaseUnits,
        LstRateBackend, LstToSolRate, MicroUsd, NavLamports, OracleBackend, ProjectionMetadata,
        SamusdBaseUnits,
    },
    risk::ProtocolRiskFlags,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteRoute {
    MintAmusd,
    RedeemAmusd,
    MintAsol,
    RedeemAsol,
    StabilityDepositAmusd,
    StabilityWithdrawUnderlying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteMode {
    Indicative,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteAsset {
    Lst,
    Amusd,
    Asol,
    Samusd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteTokenAmount {
    pub asset: QuoteAsset,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub client_quote_id: Option<String>,
    pub route: QuoteRoute,
    pub mode: QuoteMode,
    pub owner: Option<Address>,
    pub input: QuoteTokenAmount,
    pub min_outputs: Vec<QuoteTokenAmount>,
    pub slippage_bps: Option<BasisPoints>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteAmounts {
    pub gross_outputs: Vec<QuoteTokenAmount>,
    pub net_outputs: Vec<QuoteTokenAmount>,
    pub fee: Option<QuoteTokenAmount>,
    pub fee_bps: Option<BasisPoints>,
    pub haircut_bps: Option<BasisPoints>,
    pub min_suggested_outputs: Vec<QuoteTokenAmount>,
    pub drawdown_rounds_expected: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotePricingContext {
    pub safe_price_usd: Option<MicroUsd>,
    pub redeem_price_usd: Option<MicroUsd>,
    pub lst_to_sol_rate: Option<LstToSolRate>,
    pub nav_amusd_lamports: Option<NavLamports>,
    pub nav_asol_before_lamports: Option<NavLamports>,
    pub nav_asol_after_lamports: Option<NavLamports>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteBalanceSheetDelta {
    pub collateral_ratio_before_bps: Option<BasisPoints>,
    pub collateral_ratio_after_bps: Option<BasisPoints>,
    pub liability_before_lamports: Lamports,
    pub liability_after_lamports: Lamports,
    pub accounting_equity_before_lamports: i128,
    pub accounting_equity_after_lamports: i128,
    pub claimable_equity_before_lamports: Lamports,
    pub claimable_equity_after_lamports: Lamports,
    pub rounding_reserve_before_lamports: Lamports,
    pub rounding_reserve_after_lamports: Lamports,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteSourceMetadata {
    pub projection: ProjectionMetadata,
    pub oracle_backend: OracleBackend,
    pub lst_rate_backend: LstRateBackend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaminarQuote {
    pub route: QuoteRoute,
    pub mode: QuoteMode,
    pub input: QuoteTokenAmount,
    pub amounts: QuoteAmounts,
    pub pricing: QuotePricingContext,
    pub balance_sheet: QuoteBalanceSheetDelta,
    pub risk_flags: ProtocolRiskFlags,
    pub source: QuoteSourceMetadata,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultQuoteContext {
    pub current_lst_amount: LstBaseUnits,
    pub current_amusd_supply: AmusdBaseUnits,
    pub current_asol_supply: AsolBaseUnits,
    pub current_rounding_reserve_lamports: Lamports,
    pub max_rounding_reserve_lamports: Lamports,

    pub lst_to_sol_rate: LstToSolRate,
    pub safe_price_usd: MicroUsd,
    pub redeem_price_usd: MicroUsd,

    pub min_cr_bps: BasisPoints,
    pub target_cr_bps: BasisPoints,

    pub uncertainty_index_bps: BasisPoints,
    pub uncertainty_max_bps: BasisPoints,

    pub fee_amusd_mint_bps: BasisPoints,
    pub fee_amusd_redeem_bps: BasisPoints,
    pub fee_asol_mint_bps: BasisPoints,
    pub fee_asol_redeem_bps: BasisPoints,
    pub fee_min_multiplier_bps: BasisPoints,
    pub fee_max_multiplier_bps: BasisPoints,

    pub mint_paused: bool,
    pub redeem_paused: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBalanceSheet {
    pub lst_amount: LstBaseUnits,
    pub amusd_supply: AmusdBaseUnits,
    pub asol_supply: AsolBaseUnits,

    pub tvl_lamports: Lamports,
    pub liability_lamports: Lamports,
    pub accounting_equity_lamports: i128,
    pub claimable_equity_lamports: Lamports,
    pub collateral_ratio_bps: Option<BasisPoints>,

    pub nav_amusd_lamports: NavLamports,
    pub nav_asol_lamports: Option<NavLamports>,

    pub rounding_reserve_lamports: Lamports,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintAmusdPreviewInput {
    pub context: VaultQuoteContext,
    pub lst_amount: LstBaseUnits,
    pub min_amusd_out: AmusdBaseUnits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemAmusdPreviewInput {
    pub context: VaultQuoteContext,
    pub amusd_amount: AmusdBaseUnits,
    pub min_lst_out: LstBaseUnits,

    pub stability_pool_amusd_available: AmusdBaseUnits,

    pub effective_lst_amount: Option<LstBaseUnits>,
    pub effective_amusd_supply: Option<AmusdBaseUnits>,
    pub effective_asol_supply: Option<AsolBaseUnits>,
    pub effective_rounding_reserve_lamports: Option<Lamports>,

    pub drawdown_rounds_executed: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintAsolPreviewInput {
    pub context: VaultQuoteContext,
    pub lst_amount: LstBaseUnits,
    pub min_asol_out: AsolBaseUnits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemAsolPreviewInput {
    pub context: VaultQuoteContext,
    pub asol_amount: AsolBaseUnits,
    pub min_lst_out: LstBaseUnits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintAmusdPreview {
    pub lst_in: LstBaseUnits,
    pub sol_value_lamports: Lamports,
    pub gross_amusd_out: AmusdBaseUnits,
    pub net_amusd_out: AmusdBaseUnits,
    pub fee_amusd: AmusdBaseUnits,
    pub fee_bps: BasisPoints,
    pub reserve_credit_lamports: Lamports,
    pub rounding_bound_lamports: Lamports,
    pub pre_mint_balance_sheet: VaultBalanceSheet,
    pub post_mint_balance_sheet: VaultBalanceSheet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemAmusdPreview {
    pub amusd_in: AmusdBaseUnits,
    pub amusd_net_burn: AmusdBaseUnits,
    pub amusd_fee_in: AmusdBaseUnits,
    pub sol_value_gross_lamports: Lamports,
    pub lst_out: LstBaseUnits,
    pub reserve_debit_lamports: Lamports,
    pub rounding_bound_lamports: Lamports,
    pub haircut_bps: Option<BasisPoints>,
    pub solvent_mode: bool,
    pub used_user_favoring_rounding: bool,
    pub drawdown_expected: bool,
    pub drawdown_rounds_executed: u8,
    pub pre_redemption_balance_sheet: VaultBalanceSheet,
    pub post_redemption_balance_sheet: VaultBalanceSheet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MintAsolPreview {
    pub lst_in: LstBaseUnits,
    pub sol_value_lamports: Lamports,
    pub gross_asol_out: AsolBaseUnits,
    pub net_asol_out: AsolBaseUnits,
    pub fee_asol: AsolBaseUnits,
    pub fee_bps: BasisPoints,
    pub nav_before_lamports: NavLamports,
    pub bootstrap_mode: bool,
    pub orphan_equity_swept_lamports: Lamports,
    pub effective_rounding_reserve_before_mint_lamports: Lamports,
    pub reserve_credit_lamports: Lamports,
    pub rounding_bound_lamports: Lamports,
    pub pre_mint_balance_sheet: VaultBalanceSheet,
    pub post_mint_balance_sheet: VaultBalanceSheet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemAsolPreview {
    pub asol_in: AsolBaseUnits,
    pub asol_net_burn: AsolBaseUnits,
    pub asol_fee_in: AsolBaseUnits,
    pub sol_value_gross_lamports: Lamports,
    pub lst_out: LstBaseUnits,
    pub nav_before_lamports: NavLamports,
    pub solvent_mode: bool,
    pub used_user_favoring_rounding: bool,
    pub reserve_debit_lamports: Lamports,
    pub rounding_bound_lamports: Lamports,
    pub pre_redemption_balance_sheet: VaultBalanceSheet,
    pub post_redemption_balance_sheet: VaultBalanceSheet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StabilityPoolInventory {
    pub total_amusd: AmusdBaseUnits,
    pub total_asol: AsolBaseUnits,
    pub total_samusd: SamusdBaseUnits,
    pub pool_value_lamports: Lamports,
    pub nav_asol_lamports: NavLamports,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StabilityPoolQuoteContext {
    pub total_amusd: AmusdBaseUnits,
    pub total_asol: AsolBaseUnits,
    pub total_samusd: SamusdBaseUnits,
    pub stability_withdrawals_paused: bool,
    pub last_harvest_lst_to_sol_rate: LstToSolRate,

    pub price_safe_usd: MicroUsd,
    pub lst_to_sol_rate: LstToSolRate,
    pub nav_asol_lamports: NavLamports,

    pub current_lst_amount: LstBaseUnits,
    pub current_amusd_supply: AmusdBaseUnits,
    pub current_asol_supply: AsolBaseUnits,
    pub current_rounding_reserve_lamports: Lamports,

    pub min_cr_bps: BasisPoints,
    pub nav_floor_lamports: NavLamports,
    pub max_asol_mint_per_round: AsolBaseUnits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositAmusdToStabilityPoolPreviewInput {
    pub context: StabilityPoolQuoteContext,
    pub amusd_amount: AmusdBaseUnits,
    pub min_samusd_out: SamusdBaseUnits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositAmusdToStabilityPoolPreview {
    pub amusd_in: AmusdBaseUnits,
    pub deposit_value_lamports: Lamports,
    pub pool_value_before_lamports: Lamports,
    pub samusd_out: SamusdBaseUnits,
    pub pre_pool_inventory: StabilityPoolInventory,
    pub post_pool_inventory: StabilityPoolInventory,
    pub next_stability_pool_context: StabilityPoolQuoteContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawUnderlyingPreviewInput {
    pub context: StabilityPoolQuoteContext,
    pub samusd_amount: SamusdBaseUnits,
    pub min_amusd_out: AmusdBaseUnits,
    pub min_asol_out: AsolBaseUnits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawUnderlyingPreview {
    pub samusd_in: SamusdBaseUnits,
    pub amusd_out: AmusdBaseUnits,
    pub asol_out: AsolBaseUnits,
    pub pre_pool_inventory: StabilityPoolInventory,
    pub post_pool_inventory: StabilityPoolInventory,
    pub next_stability_pool_context: StabilityPoolQuoteContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarvestYieldPreviewInput {
    pub vault_context: VaultQuoteContext,
    pub context: StabilityPoolQuoteContext,
    pub new_lst_to_sol_rate: LstToSolRate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarvestYieldPreview {
    pub old_rate: LstToSolRate,
    pub new_rate: LstToSolRate,
    pub yield_delta_sol_lamports: Lamports,
    pub amusd_minted: AmusdBaseUnits,
    pub negative_yield: bool,
    pub pre_vault_balance_sheet: VaultBalanceSheet,
    pub post_vault_balance_sheet: VaultBalanceSheet,
    pub pre_pool_inventory: StabilityPoolInventory,
    pub post_pool_inventory: StabilityPoolInventory,
    pub next_vault_context: VaultQuoteContext,
    pub next_stability_pool_context: StabilityPoolQuoteContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebtEquitySwapPreviewInput {
    pub vault_context: VaultQuoteContext,
    pub context: StabilityPoolQuoteContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebtEquitySwapPreview {
    pub burn_amount_amusd: AmusdBaseUnits,
    pub asol_minted: AsolBaseUnits,
    pub nav_pre_lamports: NavLamports,
    pub nav_conv_lamports: NavLamports,
    pub cr_before_bps: Option<BasisPoints>,
    pub cr_after_bps: Option<BasisPoints>,
    pub pre_vault_balance_sheet: VaultBalanceSheet,
    pub post_vault_balance_sheet: VaultBalanceSheet,
    pub pre_pool_inventory: StabilityPoolInventory,
    pub post_pool_inventory: StabilityPoolInventory,
    pub next_vault_context: VaultQuoteContext,
    pub next_stability_pool_context: StabilityPoolQuoteContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawdownPreviewInput {
    pub vault_context: VaultQuoteContext,
    pub context: StabilityPoolQuoteContext,
    pub max_rounds: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawdownPreview {
    pub rounds: Vec<DebtEquitySwapPreview>,
    pub cr_before_bps: Option<BasisPoints>,
    pub cr_after_bps: Option<BasisPoints>,
    pub pre_vault_balance_sheet: VaultBalanceSheet,
    pub post_vault_balance_sheet: VaultBalanceSheet,
    pub pre_pool_inventory: StabilityPoolInventory,
    pub post_pool_inventory: StabilityPoolInventory,
    pub reached_target: bool,
    pub pool_exhausted: bool,
    pub hit_max_rounds: bool,
    pub next_vault_context: VaultQuoteContext,
    pub next_stability_pool_context: StabilityPoolQuoteContext,
}
