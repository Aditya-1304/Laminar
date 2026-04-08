use anchor_lang::prelude::Pubkey;
use anyhow::{anyhow, Result};

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
