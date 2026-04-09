use std::convert::TryFrom;

use laminar_core::{Address, LaminarProtocolSnapshot, LstRateBackend, OracleBackend};
use laminar_store::{
    GlobalStateCurrentRecord, GlobalStateCurrentRepository, IngestionCheckpointRecord,
    IngestionCheckpointRepository, LaminarStores, RepositoryError, StabilityPoolCurrentRecord,
    StabilityPoolCurrentRepository,
};
use serde_json::Value as JsonValue;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionWriteContext {
    pub global_state_pubkey: Address,
    pub stability_pool_pubkey: Option<Address>,
    pub tx_signature: Option<String>,
}

impl ProjectionWriteContext {
    pub fn new(global_state_pubkey: impl Into<Address>) -> Self {
        Self {
            global_state_pubkey: global_state_pubkey.into(),
            stability_pool_pubkey: None,
            tx_signature: None,
        }
    }

    pub fn with_stability_pool_pubkey(mut self, stability_pool_pubkey: impl Into<Address>) -> Self {
        self.stability_pool_pubkey = Some(stability_pool_pubkey.into());
        self
    }

    pub fn with_tx_signature(mut self, tx_signature: impl Into<String>) -> Self {
        self.tx_signature = Some(tx_signature.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionCheckpoint {
    pub stream_name: String,
    pub processed_slot: u64,
    pub confirmed_slot: u64,
    pub finalized_slot: u64,
    pub cursor: JsonValue,
    pub metadata: JsonValue,
}

#[derive(Debug, Error)]
pub enum ProjectionError {
    #[error("projection metadata is missing indexed_slot")]
    MissingIndexedSlot,
    #[error("projection slot {slot} does not fit into i64")]
    SlotOutOfRange { slot: u64 },
    #[error("missing required address `{field}`")]
    MissingRequiredAddress { field: &'static str },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[derive(Debug, Clone)]
pub struct LaminarProjectionWriter {
    ingestion_checkpoints: IngestionCheckpointRepository,
    global_state_current: GlobalStateCurrentRepository,
    stability_pool_current: StabilityPoolCurrentRepository,
}

impl LaminarProjectionWriter {
    pub fn new(stores: &LaminarStores) -> Self {
        Self {
            ingestion_checkpoints: stores.ingestion_checkpoints(),
            global_state_current: stores.global_state_current(),
            stability_pool_current: stores.stability_pool_current(),
        }
    }

    pub async fn write_protocol_snapshot(
        &self,
        context: &ProjectionWriteContext,
        snapshot: &LaminarProtocolSnapshot,
    ) -> Result<(), ProjectionError> {
        let global_state_record = global_state_current_record_from_snapshot(context, snapshot)?;
        self.global_state_current
            .upsert(&global_state_record)
            .await?;

        if let Some(stability_pool_record) =
            stability_pool_current_record_from_snapshot(context, snapshot)?
        {
            self.stability_pool_current
                .upsert(&stability_pool_record)
                .await?;
        }

        Ok(())
    }

    pub async fn advance_checkpoint(
        &self,
        checkpoint: &ProjectionCheckpoint,
    ) -> Result<(), ProjectionError> {
        self.ingestion_checkpoints
            .upsert(&ingestion_checkpoint_record(checkpoint)?)
            .await?;
        Ok(())
    }
}

pub fn global_state_current_record_from_snapshot(
    context: &ProjectionWriteContext,
    snapshot: &LaminarProtocolSnapshot,
) -> Result<GlobalStateCurrentRecord, ProjectionError> {
    Ok(GlobalStateCurrentRecord {
        global_state_pubkey: required_address("global_state_pubkey", &context.global_state_pubkey)?,
        projection_slot: projection_slot(snapshot)?,
        tx_signature: context.tx_signature.clone(),
        authority: snapshot.global.authority.as_str().to_owned(),
        amusd_mint: snapshot.global.amusd_mint.as_str().to_owned(),
        asol_mint: snapshot.global.asol_mint.as_str().to_owned(),
        treasury: snapshot.global.treasury.as_str().to_owned(),
        supported_lst_mint: snapshot.global.supported_lst_mint.as_str().to_owned(),
        total_lst_amount: snapshot.global.total_lst_amount,
        amusd_supply: snapshot.global.amusd_supply,
        asol_supply: snapshot.global.asol_supply,
        min_cr_bps: snapshot.global.min_cr_bps,
        target_cr_bps: snapshot.global.target_cr_bps,
        mint_paused: snapshot.global.mint_paused,
        redeem_paused: snapshot.global.redeem_paused,
        oracle_backend: oracle_backend_name(snapshot.global.oracle_backend),
        lst_rate_backend: lst_rate_backend_name(snapshot.global.lst_rate_backend),
        pyth_sol_usd_price_account: optional_address(&snapshot.global.pyth_sol_usd_price_account),
        lst_stake_pool: optional_address(&snapshot.global.lst_stake_pool),
        rounding_reserve_lamports: snapshot.global.rounding_reserve_lamports,
        uncertainty_index_bps: snapshot.global.uncertainty_index_bps,
        nav_floor_lamports: snapshot.global.nav_floor_lamports,
        max_asol_mint_per_round: snapshot.global.max_asol_mint_per_round,
        raw_model: serde_json::to_value(&snapshot.global)?,
    })
}

pub fn stability_pool_current_record_from_snapshot(
    context: &ProjectionWriteContext,
    snapshot: &LaminarProtocolSnapshot,
) -> Result<Option<StabilityPoolCurrentRecord>, ProjectionError> {
    let Some(stability_pool_pubkey) = context.stability_pool_pubkey.as_ref() else {
        return Ok(None);
    };

    Ok(Some(StabilityPoolCurrentRecord {
        stability_pool_pubkey: required_address("stability_pool_pubkey", stability_pool_pubkey)?,
        global_state_pubkey: required_address("global_state_pubkey", &context.global_state_pubkey)?,
        projection_slot: projection_slot(snapshot)?,
        tx_signature: context.tx_signature.clone(),
        samusd_mint: snapshot.stability_pool.samusd_mint.as_str().to_owned(),
        pool_amusd_vault: snapshot.stability_pool.pool_amusd_vault.as_str().to_owned(),
        pool_asol_vault: snapshot.stability_pool.pool_asol_vault.as_str().to_owned(),
        total_amusd: snapshot.stability_pool.total_amusd,
        total_asol: snapshot.stability_pool.total_asol,
        total_samusd: snapshot.stability_pool.total_samusd,
        stability_withdrawals_paused: snapshot.stability_pool.stability_withdrawals_paused,
        last_harvest_lst_to_sol_rate: snapshot.stability_pool.last_harvest_lst_to_sol_rate,
        raw_model: serde_json::to_value(&snapshot.stability_pool)?,
    }))
}

pub fn ingestion_checkpoint_record(
    checkpoint: &ProjectionCheckpoint,
) -> Result<IngestionCheckpointRecord, ProjectionError> {
    Ok(IngestionCheckpointRecord {
        stream_name: checkpoint.stream_name.clone(),
        processed_slot: slot_to_i64(checkpoint.processed_slot)?,
        confirmed_slot: slot_to_i64(checkpoint.confirmed_slot)?,
        finalized_slot: slot_to_i64(checkpoint.finalized_slot)?,
        cursor: checkpoint.cursor.clone(),
        metadata: checkpoint.metadata.clone(),
    })
}

fn projection_slot(snapshot: &LaminarProtocolSnapshot) -> Result<i64, ProjectionError> {
    let slot = snapshot
        .metadata
        .indexed_slot
        .ok_or(ProjectionError::MissingIndexedSlot)?;
    slot_to_i64(slot)
}

fn slot_to_i64(slot: u64) -> Result<i64, ProjectionError> {
    i64::try_from(slot).map_err(|_| ProjectionError::SlotOutOfRange { slot })
}

fn required_address(field: &'static str, address: &Address) -> Result<String, ProjectionError> {
    if address.as_str().is_empty() {
        Err(ProjectionError::MissingRequiredAddress { field })
    } else {
        Ok(address.as_str().to_owned())
    }
}

fn optional_address(address: &Address) -> Option<String> {
    if address.as_str().is_empty() {
        None
    } else {
        Some(address.as_str().to_owned())
    }
}

fn oracle_backend_name(backend: OracleBackend) -> String {
    match backend {
        OracleBackend::Mock => "mock".to_owned(),
        OracleBackend::PythPush => "pyth_push".to_owned(),
        OracleBackend::Other(value) => format!("other:{value}"),
    }
}

fn lst_rate_backend_name(backend: LstRateBackend) -> String {
    match backend {
        LstRateBackend::Mock => "mock".to_owned(),
        LstRateBackend::SanctumStakePool => "sanctum_stake_pool".to_owned(),
        LstRateBackend::Other(value) => format!("other:{value}"),
    }
}
