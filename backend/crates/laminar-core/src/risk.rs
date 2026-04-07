use serde::{Deserialize, Serialize};

use crate::{
    math::{mul_div_up, MathResult, BPS_PRECISION},
    models::{BasisPoints, Epoch, Lamports, MicroUsd, Slot},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollateralizationMode {
    NoDebt,
    Healthy,
    Recovery,
    Insolvent,
}

impl Default for CollateralizationMode {
    fn default() -> Self {
        Self::NoDebt
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolRiskFlags {
    pub mint_paused: bool,
    pub redeem_paused: bool,
    pub stability_withdrawals_paused: bool,
    pub oracle_stale: bool,
    pub lst_stale: bool,
    pub high_confidence: bool,
    pub insolvency_mode: bool,
    pub drawdown_expected: bool,
}

impl ProtocolRiskFlags {
    pub fn any_pause(&self) -> bool {
        self.mint_paused || self.redeem_paused || self.stability_withdrawals_paused
    }

    pub fn write_blocked(&self) -> bool {
        self.any_pause() || self.oracle_stale || self.lst_stale
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolRiskSnapshot {
    pub flags: ProtocolRiskFlags,
    pub collateralization_mode: CollateralizationMode,
    pub collateral_ratio_bps: Option<BasisPoints>,
    pub min_cr_bps: BasisPoints,
    pub target_cr_bps: BasisPoints,
    pub oracle_age_slots: Option<Slot>,
    pub lst_age_epochs: Option<Epoch>,
    pub confidence_bps: Option<BasisPoints>,
    pub tvl_lamports: Lamports,
    pub liability_lamports: Lamports,
}

pub fn classify_collateralization_mode(
    collateral_ratio_bps: Option<BasisPoints>,
    min_cr_bps: BasisPoints,
) -> CollateralizationMode {
    match collateral_ratio_bps {
        None => CollateralizationMode::NoDebt,
        Some(cr_bps) if cr_bps < BPS_PRECISION => CollateralizationMode::Insolvent,
        Some(cr_bps) if cr_bps < min_cr_bps => CollateralizationMode::Recovery,
        Some(_) => CollateralizationMode::Healthy,
    }
}

pub fn derive_oracle_age_slots(current_slot: Slot, last_update_slot: Slot) -> Option<Slot> {
    current_slot.checked_sub(last_update_slot)
}

pub fn derive_lst_age_epochs(current_epoch: Epoch, last_update_epoch: Epoch) -> Option<Epoch> {
    current_epoch.checked_sub(last_update_epoch)
}

pub fn derive_confidence_bps(
    confidence_usd: MicroUsd,
    price_ema_usd: MicroUsd,
) -> MathResult<Option<BasisPoints>> {
    if price_ema_usd == 0 {
        return Ok(None);
    }

    if confidence_usd == 0 {
        return Ok(Some(0));
    }

    Ok(Some(mul_div_up(
        confidence_usd,
        BPS_PRECISION,
        price_ema_usd,
    )?))
}
