use crate::models::{BasisPoints, LstRateBackend, OracleBackend};

pub const RAW_STABILITY_WITHDRAWALS_PAUSED_FIELD: &str = "withdrawls_paused";
pub const NORMALIZED_STABILITY_WITHDRAWALS_PAUSED_FIELD: &str = "stability_withdrawals_paused";

pub fn normalize_field_name(raw: &str) -> &str {
    match raw {
        RAW_STABILITY_WITHDRAWALS_PAUSED_FIELD => NORMALIZED_STABILITY_WITHDRAWALS_PAUSED_FIELD,
        _ => raw,
    }
}

pub fn normalize_stability_withdrawals_paused(raw: bool) -> bool {
    raw
}

pub fn normalize_collateral_ratio_bps(raw: BasisPoints) -> Option<BasisPoints> {
    if raw == u64::MAX {
        None
    } else {
        Some(raw)
    }
}

pub fn denormalize_collateral_ratio_bps(value: Option<BasisPoints>) -> BasisPoints {
    value.unwrap_or(u64::MAX)
}

pub fn normalize_oracle_backend(raw: u8) -> OracleBackend {
    match raw {
        0 => OracleBackend::Mock,
        1 => OracleBackend::PythPush,
        other => OracleBackend::Other(other),
    }
}

pub fn normalize_lst_rate_backend(raw: u8) -> LstRateBackend {
    match raw {
        0 => LstRateBackend::Mock,
        1 => LstRateBackend::SanctumStakePool,
        other => LstRateBackend::Other(other),
    }
}
