use serde::{Deserialize, Serialize};

use crate::{
    models::{
        Address, BasisPoints, Lamports, LstRateBackend, LstToSolRate, MicroUsd, NavLamports,
        OracleBackend, ProjectionMetadata,
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
