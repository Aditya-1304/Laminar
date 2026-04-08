use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};

use crate::remaining_accounts::ResolvedRemainingAccounts;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputeBudgetConfig {
    pub unit_limit: Option<u32>,
    pub unit_price_micro_lamports: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LaminarInstructionEnvelope {
    pub instruction: Instruction,
    pub compute_budget: ComputeBudgetConfig,
    pub remaining_accounts: ResolvedRemainingAccounts,
}

impl LaminarInstructionEnvelope {
    pub fn new(instruction: Instruction, remaining_accounts: ResolvedRemainingAccounts) -> Self {
        Self {
            instruction,
            compute_budget: ComputeBudgetConfig::default(),
            remaining_accounts,
        }
    }

    pub fn with_compute_budget(mut self, compute_budget: ComputeBudgetConfig) -> Self {
        self.compute_budget = compute_budget;
        self
    }

    pub fn full_instruction(&self) -> Instruction {
        extend_instruction_with_remaining_accounts(
            self.instruction.clone(),
            &self.remaining_accounts,
        )
    }
}

pub fn extend_instruction_with_remaining_accounts(
    mut instruction: Instruction,
    resolved: &ResolvedRemainingAccounts,
) -> Instruction {
    instruction.accounts = merge_account_metas(instruction.accounts, &resolved.account_metas);
    instruction
}

pub fn merge_account_metas(mut base: Vec<AccountMeta>, extras: &[AccountMeta]) -> Vec<AccountMeta> {
    for extra in extras {
        if let Some(existing) = base.iter_mut().find(|meta| meta.pubkey == extra.pubkey) {
            existing.is_signer |= extra.is_signer;
            existing.is_writable |= extra.is_writable;
        } else {
            base.push(extra.clone());
        }
    }

    base
}
