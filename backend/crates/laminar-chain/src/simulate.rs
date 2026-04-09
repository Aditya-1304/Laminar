use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_transaction::versioned::VersionedTransaction;

use crate::UnsignedLaminarTransaction;

#[derive(Debug, Clone)]
pub struct LaminarSimulationRequest {
    pub transaction: VersionedTransaction,
    pub sig_verify: bool,
    pub replace_recent_blockhash: bool,
}

impl LaminarSimulationRequest {
    pub fn new(transaction: VersionedTransaction) -> Self {
        Self {
            transaction,
            sig_verify: false,
            replace_recent_blockhash: false,
        }
    }

    pub fn from_unsigned_transaction(transaction: &UnsignedLaminarTransaction) -> Self {
        Self::new(transaction.transaction.clone())
    }

    pub fn with_sig_verify(mut self, sig_verify: bool) -> Self {
        self.sig_verify = sig_verify;
        self
    }

    pub fn with_replace_recent_blockhash(mut self, replace_recent_blockhash: bool) -> Self {
        self.replace_recent_blockhash = replace_recent_blockhash;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaminarSimulationResult {
    pub logs: Vec<String>,
    pub units_consumed: Option<u64>,
    pub error: Option<String>,
    pub replacement_blockhash: Option<String>,
    pub slot: Option<u64>,
}

impl LaminarSimulationResult {
    pub fn succeeded(&self) -> bool {
        self.error.is_none()
    }
}

pub trait LaminarSimulator {
    fn simulate_versioned_transaction(
        &self,
        request: LaminarSimulationRequest,
    ) -> Result<LaminarSimulationResult>;
}
