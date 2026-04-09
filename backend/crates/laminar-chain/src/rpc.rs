use anchor_lang::prelude::Pubkey;
use anyhow::{anyhow, Error as AnyhowError, Result};
use serde::{Deserialize, Serialize};
use solana_hash::Hash;
use thiserror::Error;

use crate::decode::{
    decode_mint_account, decode_protocol_accounts, decode_token_account, ChainDecodeError,
    DecodedMint, DecodedProtocolAccounts, DecodedTokenAccount,
};
use crate::pda::laminar_pdas;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainAccount {
    pub pubkey: Pubkey,
    pub owner: Pubkey,
    pub lamports: u64,
    pub data: Vec<u8>,
    pub executable: bool,
    pub rent_epoch: u64,
}

pub trait LaminarAccountProvider {
    fn get_account(&self, address: &Pubkey) -> Result<Option<ChainAccount>>;

    fn get_multiple_accounts(&self, addresses: &[Pubkey]) -> Result<Vec<Option<ChainAccount>>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedProtocolAccounts {
    pub global_state: ChainAccount,
    pub stability_pool_state: Option<ChainAccount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentBlockhash {
    pub hash: Hash,
    pub last_valid_block_height: u64,
    pub context_slot: Option<u64>,
}

impl RecentBlockhash {
    pub fn new(hash: Hash, last_valid_block_height: u64, context_slot: Option<u64>) -> Self {
        Self {
            hash,
            last_valid_block_height,
            context_slot,
        }
    }

    pub fn metadata(&self) -> RecentBlockhashMetadata {
        RecentBlockhashMetadata {
            blockhash: self.hash.to_string(),
            last_valid_block_height: self.last_valid_block_height,
            context_slot: self.context_slot,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentBlockhashMetadata {
    pub blockhash: String,
    pub last_valid_block_height: u64,
    pub context_slot: Option<u64>,
}

pub trait LaminarBlockhashProvider {
    fn get_latest_blockhash(&self) -> Result<RecentBlockhash>;
}

#[derive(Debug, Error)]
pub enum RpcLoadError {
    #[error("missing account `{label}` at {address}")]
    MissingAccount {
        label: &'static str,
        address: Pubkey,
    },
    #[error(transparent)]
    Provider(#[from] AnyhowError),
    #[error(transparent)]
    Decode(#[from] ChainDecodeError),
}

pub fn load_protocol_accounts<P: LaminarAccountProvider>(
    provider: &P,
) -> Result<LoadedProtocolAccounts> {
    let pdas = laminar_pdas();
    let addresses = [pdas.global_state, pdas.stability_pool_state];

    let mut accounts = provider.get_multiple_accounts(&addresses)?;
    if accounts.len() != addresses.len() {
        return Err(anyhow!(
            "expected {} accounts from provider, got {}",
            addresses.len(),
            accounts.len()
        ));
    }

    let global_state = accounts.remove(0).ok_or_else(|| {
        anyhow!(
            "missing Laminar global_state account at {}",
            pdas.global_state
        )
    })?;

    let stability_pool_state = accounts.remove(0);

    Ok(LoadedProtocolAccounts {
        global_state,
        stability_pool_state,
    })
}

pub fn load_optional_account<P: LaminarAccountProvider>(
    provider: &P,
    address: &Pubkey,
) -> std::result::Result<Option<ChainAccount>, RpcLoadError> {
    provider.get_account(address).map_err(Into::into)
}

pub fn load_required_account<P: LaminarAccountProvider>(
    provider: &P,
    label: &'static str,
    address: &Pubkey,
) -> std::result::Result<ChainAccount, RpcLoadError> {
    load_optional_account(provider, address)?.ok_or(RpcLoadError::MissingAccount {
        label,
        address: *address,
    })
}

pub fn load_required_mint<P: LaminarAccountProvider>(
    provider: &P,
    address: &Pubkey,
) -> std::result::Result<(ChainAccount, DecodedMint), RpcLoadError> {
    let account = load_required_account(provider, "mint", address)?;
    let decoded = decode_mint_account(&account)?;
    Ok((account, decoded))
}

pub fn load_required_token_account<P: LaminarAccountProvider>(
    provider: &P,
    address: &Pubkey,
) -> std::result::Result<(ChainAccount, DecodedTokenAccount), RpcLoadError> {
    let account = load_required_account(provider, "token_account", address)?;
    let decoded = decode_token_account(&account)?;
    Ok((account, decoded))
}

pub fn load_decoded_protocol_accounts<P: LaminarAccountProvider>(
    provider: &P,
    indexed_slot: Option<u64>,
) -> std::result::Result<DecodedProtocolAccounts, RpcLoadError> {
    let loaded = load_protocol_accounts(provider).map_err(RpcLoadError::from)?;
    Ok(decode_protocol_accounts(
        &loaded.global_state,
        loaded.stability_pool_state.as_ref(),
        indexed_slot,
    )?)
}
