use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    build_unsigned_v0_transaction, LaminarAddressLookupTable, LaminarBlockhashProvider,
    LaminarInstructionEnvelope, LaminarSimulationRequest, LaminarSimulationResult,
    LaminarSimulator, TransactionBuildError, UnsignedLaminarTransaction,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaminarPreflightConfig {
    pub address_lookup_tables: Vec<LaminarAddressLookupTable>,
    pub sig_verify: bool,
    pub replace_recent_blockhash: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountMetaSummary {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpiryMetadata {
    pub blockhash: String,
    pub last_valid_block_height: u64,
    pub context_slot: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaminarSimulationSummary {
    pub ok: bool,
    pub error: Option<String>,
    pub units_consumed: Option<u64>,
    pub log_count: usize,
    pub replacement_blockhash: Option<String>,
    pub slot: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedLaminarTransaction {
    pub unsigned_transaction: UnsignedLaminarTransaction,
    pub simulation_result: LaminarSimulationResult,
    pub simulation_summary: LaminarSimulationSummary,
    pub account_metas: Vec<AccountMetaSummary>,
    pub expiry_metadata: ExpiryMetadata,
}

#[derive(Debug, Error)]
pub enum PreflightError {
    #[error("failed to fetch recent blockhash: {source}")]
    Blockhash {
        #[source]
        source: anyhow::Error,
    },
    #[error(transparent)]
    Build(#[from] TransactionBuildError),
    #[error("failed to simulate versioned transaction: {source}")]
    Simulate {
        #[source]
        source: anyhow::Error,
    },
}

pub fn prepare_simulated_unsigned_v0_transaction<B, S>(
    payer: &Pubkey,
    envelope: &LaminarInstructionEnvelope,
    blockhash_provider: &B,
    simulator: &S,
    config: &LaminarPreflightConfig,
) -> Result<PreparedLaminarTransaction, PreflightError>
where
    B: LaminarBlockhashProvider,
    S: LaminarSimulator,
{
    let recent_blockhash = blockhash_provider
        .get_latest_blockhash()
        .map_err(|source| PreflightError::Blockhash { source })?;

    let unsigned_transaction = build_unsigned_v0_transaction(
        payer,
        envelope,
        &recent_blockhash,
        &config.address_lookup_tables,
    )?;

    let simulation_request =
        LaminarSimulationRequest::from_unsigned_transaction(&unsigned_transaction)
            .with_sig_verify(config.sig_verify)
            .with_replace_recent_blockhash(config.replace_recent_blockhash);

    let simulation_result = simulator
        .simulate_versioned_transaction(simulation_request)
        .map_err(|source| PreflightError::Simulate { source })?;

    Ok(PreparedLaminarTransaction {
        expiry_metadata: ExpiryMetadata {
            blockhash: unsigned_transaction.recent_blockhash.blockhash.clone(),
            last_valid_block_height: unsigned_transaction
                .recent_blockhash
                .last_valid_block_height,
            context_slot: unsigned_transaction.recent_blockhash.context_slot,
        },
        account_metas: summarize_account_metas(&envelope.full_instruction().accounts),
        simulation_summary: summarize_simulation_result(&simulation_result),
        simulation_result,
        unsigned_transaction,
    })
}

pub fn summarize_account_metas(account_metas: &[AccountMeta]) -> Vec<AccountMetaSummary> {
    account_metas
        .iter()
        .map(|account_meta| AccountMetaSummary {
            pubkey: account_meta.pubkey.to_string(),
            is_signer: account_meta.is_signer,
            is_writable: account_meta.is_writable,
        })
        .collect()
}

pub fn summarize_simulation_result(
    simulation_result: &LaminarSimulationResult,
) -> LaminarSimulationSummary {
    LaminarSimulationSummary {
        ok: simulation_result.succeeded(),
        error: simulation_result.error.clone(),
        units_consumed: simulation_result.units_consumed,
        log_count: simulation_result.logs.len(),
        replacement_blockhash: simulation_result.replacement_blockhash.clone(),
        slot: simulation_result.slot,
    }
}
