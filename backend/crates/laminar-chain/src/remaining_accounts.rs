use std::str::FromStr;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::AccountMeta;
use laminar_core::{GlobalStateModel, LstRateBackend, OracleBackend};
use thiserror::Error;

#[derive(Debug, Clone, Default)]
pub struct RemainingAccountsConfig {
    pub pyth_sol_usd_price_account: Option<Pubkey>,
    pub lst_stake_pool: Option<Pubkey>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedRemainingAccounts {
    pub pubkeys: Vec<Pubkey>,
    pub account_metas: Vec<AccountMeta>,
}

impl ResolvedRemainingAccounts {
    pub fn is_empty(&self) -> bool {
        self.pubkeys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.pubkeys.len()
    }
}

#[derive(Debug, Error)]
pub enum RemainingAccountsError {
    #[error("missing Pyth SOL/USD price account for oracle backend")]
    MissingPythSolUsdPriceAccount,
    #[error("missing LST stake-pool account for LST backend")]
    MissingLstStakePool,
    #[error("invalid pubkey in field `{field}`: {value}")]
    InvalidPubkey { field: &'static str, value: String },
}

pub fn resolve_remaining_accounts(
    oracle_backend: OracleBackend,
    lst_rate_backend: LstRateBackend,
    config: &RemainingAccountsConfig,
) -> Result<ResolvedRemainingAccounts, RemainingAccountsError> {
    let mut pubkeys = Vec::new();

    match oracle_backend {
        OracleBackend::Mock => {}
        OracleBackend::PythPush => {
            let price_feed = config
                .pyth_sol_usd_price_account
                .ok_or(RemainingAccountsError::MissingPythSolUsdPriceAccount)?;
            push_unique(&mut pubkeys, price_feed);
        }
        OracleBackend::Other(_) => {}
    }

    match lst_rate_backend {
        LstRateBackend::Mock => {}
        LstRateBackend::SanctumStakePool => {
            let stake_pool = config
                .lst_stake_pool
                .ok_or(RemainingAccountsError::MissingLstStakePool)?;
            push_unique(&mut pubkeys, stake_pool);
        }
        LstRateBackend::Other(_) => {}
    }

    let account_metas = pubkeys
        .iter()
        .map(|pubkey| AccountMeta::new_readonly(*pubkey, false))
        .collect();

    Ok(ResolvedRemainingAccounts {
        pubkeys,
        account_metas,
    })
}

pub fn resolve_remaining_accounts_from_global_state(
    global: &GlobalStateModel,
    fallback: &RemainingAccountsConfig,
) -> Result<ResolvedRemainingAccounts, RemainingAccountsError> {
    let config = RemainingAccountsConfig {
        pyth_sol_usd_price_account: parse_model_pubkey(
            "pyth_sol_usd_price_account",
            global.pyth_sol_usd_price_account.as_str(),
        )?
        .or(fallback.pyth_sol_usd_price_account),
        lst_stake_pool: parse_model_pubkey("lst_stake_pool", global.lst_stake_pool.as_str())?
            .or(fallback.lst_stake_pool),
    };

    resolve_remaining_accounts(global.oracle_backend, global.lst_rate_backend, &config)
}

fn parse_model_pubkey(
    field: &'static str,
    value: &str,
) -> Result<Option<Pubkey>, RemainingAccountsError> {
    if value.is_empty() {
        return Ok(None);
    }

    let pubkey = Pubkey::from_str(value).map_err(|_| RemainingAccountsError::InvalidPubkey {
        field,
        value: value.to_owned(),
    })?;

    if pubkey == Pubkey::default() {
        Ok(None)
    } else {
        Ok(Some(pubkey))
    }
}

fn push_unique(target: &mut Vec<Pubkey>, pubkey: Pubkey) {
    if !target.iter().any(|existing| existing == &pubkey) {
        target.push(pubkey);
    }
}
