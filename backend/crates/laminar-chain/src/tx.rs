use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{
    AccountMeta as AnchorAccountMeta, Instruction as AnchorInstruction,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use solana_hash::Hash;
use solana_instruction::{AccountMeta as TxAccountMeta, Instruction as TxInstruction};
use solana_message::{v0, AddressLookupTableAccount, CompileError, VersionedMessage};
use solana_pubkey::Pubkey as TxPubkey;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;
use thiserror::Error;

use crate::{
    ComputeBudgetConfig, LaminarInstructionEnvelope, RecentBlockhash, RecentBlockhashMetadata,
};

const COMPUTE_BUDGET_PROGRAM_ID: &str = "ComputeBudget111111111111111111111111111111";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaminarAddressLookupTable {
    pub key: Pubkey,
    pub addresses: Vec<Pubkey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionSummary {
    pub program_id: String,
    pub account_count: usize,
    pub signer_count: usize,
    pub writable_count: usize,
    pub data_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedLaminarTransaction {
    pub transaction: VersionedTransaction,
    pub serialized: Vec<u8>,
    pub base64: String,
    pub recent_blockhash: RecentBlockhashMetadata,
    pub required_signers: Vec<Pubkey>,
    pub instruction_summaries: Vec<InstructionSummary>,
    pub address_lookup_table_count: usize,
}

#[derive(Debug, Error)]
pub enum TransactionBuildError {
    #[error("message compile failed: {0}")]
    Compile(#[from] CompileError),
    #[error("failed to serialize versioned transaction: {0}")]
    Serialize(String),
}

pub fn build_unsigned_v0_transaction(
    payer: &Pubkey,
    envelope: &LaminarInstructionEnvelope,
    recent_blockhash: &RecentBlockhash,
    address_lookup_tables: &[LaminarAddressLookupTable],
) -> Result<UnsignedLaminarTransaction, TransactionBuildError> {
    let instructions = build_tx_instruction_stack(envelope);
    let instruction_summaries = instructions.iter().map(summarize_instruction).collect();

    let lookup_tables = address_lookup_tables
        .iter()
        .map(convert_lookup_table_account)
        .collect::<Vec<_>>();

    let message = VersionedMessage::V0(v0::Message::try_compile(
        &to_tx_pubkey(payer),
        &instructions,
        &lookup_tables,
        recent_blockhash.hash,
    )?);

    let required_signers = message.static_account_keys()
        [..usize::from(message.header().num_required_signatures)]
        .iter()
        .map(to_anchor_pubkey)
        .collect::<Vec<_>>();

    let transaction = VersionedTransaction {
        signatures: vec![Signature::default(); required_signers.len()],
        message,
    };

    let serialized = bincode::serialize(&transaction)
        .map_err(|err| TransactionBuildError::Serialize(err.to_string()))?;
    let base64 = BASE64_STANDARD.encode(&serialized);

    Ok(UnsignedLaminarTransaction {
        transaction,
        serialized,
        base64,
        recent_blockhash: recent_blockhash.metadata(),
        required_signers,
        instruction_summaries,
        address_lookup_table_count: address_lookup_tables.len(),
    })
}

pub fn build_tx_instruction_stack(envelope: &LaminarInstructionEnvelope) -> Vec<TxInstruction> {
    let mut instructions = compute_budget_instructions(&envelope.compute_budget);
    instructions.push(convert_anchor_instruction(&envelope.full_instruction()));
    instructions
}

pub fn compute_budget_instructions(config: &ComputeBudgetConfig) -> Vec<TxInstruction> {
    let mut instructions = Vec::new();
    let program_id = compute_budget_program_id();

    if let Some(unit_limit) = config.unit_limit {
        let mut data = vec![2u8];
        data.extend_from_slice(&unit_limit.to_le_bytes());
        instructions.push(TxInstruction {
            program_id,
            accounts: Vec::new(),
            data,
        });
    }

    if let Some(unit_price_micro_lamports) = config.unit_price_micro_lamports {
        let mut data = vec![3u8];
        data.extend_from_slice(&unit_price_micro_lamports.to_le_bytes());
        instructions.push(TxInstruction {
            program_id,
            accounts: Vec::new(),
            data,
        });
    }

    instructions
}

fn summarize_instruction(instruction: &TxInstruction) -> InstructionSummary {
    InstructionSummary {
        program_id: instruction.program_id.to_string(),
        account_count: instruction.accounts.len(),
        signer_count: instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_signer)
            .count(),
        writable_count: instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_writable)
            .count(),
        data_len: instruction.data.len(),
    }
}

fn convert_anchor_instruction(instruction: &AnchorInstruction) -> TxInstruction {
    TxInstruction {
        program_id: to_tx_pubkey(&instruction.program_id),
        accounts: instruction
            .accounts
            .iter()
            .map(convert_anchor_account_meta)
            .collect(),
        data: instruction.data.clone(),
    }
}

fn convert_anchor_account_meta(account_meta: &AnchorAccountMeta) -> TxAccountMeta {
    if account_meta.is_writable {
        TxAccountMeta::new(to_tx_pubkey(&account_meta.pubkey), account_meta.is_signer)
    } else {
        TxAccountMeta::new_readonly(to_tx_pubkey(&account_meta.pubkey), account_meta.is_signer)
    }
}

fn convert_lookup_table_account(
    address_lookup_table: &LaminarAddressLookupTable,
) -> AddressLookupTableAccount {
    AddressLookupTableAccount {
        key: to_tx_pubkey(&address_lookup_table.key),
        addresses: address_lookup_table
            .addresses
            .iter()
            .map(to_tx_pubkey)
            .collect(),
    }
}

fn to_tx_pubkey(pubkey: &Pubkey) -> TxPubkey {
    TxPubkey::new_from_array(pubkey.to_bytes())
}

fn to_anchor_pubkey(pubkey: &TxPubkey) -> Pubkey {
    Pubkey::new_from_array(pubkey.to_bytes())
}

fn compute_budget_program_id() -> TxPubkey {
    TxPubkey::try_from(COMPUTE_BUDGET_PROGRAM_ID).expect("valid compute budget program id")
}

#[allow(dead_code)]
fn _blockhash_to_string(hash: Hash) -> String {
    hash.to_string()
}
