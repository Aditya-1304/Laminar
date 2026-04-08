use anchor_lang::prelude::Pubkey;
use spl_associated_token_account::get_associated_token_address_with_program_id;

pub use crate::wire::ID as LAMINAR_PROGRAM_ID;
use crate::wire::{
    GLOBAL_STATE_SEED, STABILITY_POOL_AUTHORITY_SEED, STABILITY_POOL_STATE_SEED,
    VAULT_AUTHORITY_SEED,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaminarPdas {
    pub global_state: Pubkey,
    pub global_state_bump: u8,
    pub vault_authority: Pubkey,
    pub vault_authority_bump: u8,
    pub stability_pool_state: Pubkey,
    pub stability_pool_state_bump: u8,
    pub stability_pool_authority: Pubkey,
    pub stability_pool_authority_bump: u8,
}

pub fn laminar_pdas() -> LaminarPdas {
    let (global_state, global_state_bump) = global_state_pda();
    let (vault_authority, vault_authority_bump) = vault_authority_pda();
    let (stability_pool_state, stability_pool_state_bump) = stability_pool_state_pda();
    let (stability_pool_authority, stability_pool_authority_bump) = stability_pool_authority_pda();

    LaminarPdas {
        global_state,
        global_state_bump,
        vault_authority,
        vault_authority_bump,
        stability_pool_state,
        stability_pool_state_bump,
        stability_pool_authority,
        stability_pool_authority_bump,
    }
}

pub fn global_state_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[GLOBAL_STATE_SEED], &LAMINAR_PROGRAM_ID)
}

pub fn vault_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED], &LAMINAR_PROGRAM_ID)
}

pub fn stability_pool_state_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[STABILITY_POOL_STATE_SEED], &LAMINAR_PROGRAM_ID)
}

pub fn stability_pool_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[STABILITY_POOL_AUTHORITY_SEED], &LAMINAR_PROGRAM_ID)
}

pub fn associated_token_address(
    owner: &Pubkey,
    mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Pubkey {
    get_associated_token_address_with_program_id(owner, mint, token_program_id)
}

pub fn vault_token_account(lst_mint: &Pubkey, token_program_id: &Pubkey) -> Pubkey {
    let (vault_authority, _) = vault_authority_pda();
    associated_token_address(&vault_authority, lst_mint, token_program_id)
}

pub fn stability_pool_amusd_vault(amusd_mint: &Pubkey, token_program_id: &Pubkey) -> Pubkey {
    let (authority, _) = stability_pool_authority_pda();
    associated_token_address(&authority, amusd_mint, token_program_id)
}

pub fn stability_pool_asol_vault(asol_mint: &Pubkey, token_program_id: &Pubkey) -> Pubkey {
    let (authority, _) = stability_pool_authority_pda();
    associated_token_address(&authority, asol_mint, token_program_id)
}

pub fn treasury_token_account(
    treasury: &Pubkey,
    mint: &Pubkey,
    token_program_id: &Pubkey,
) -> Pubkey {
    associated_token_address(treasury, mint, token_program_id)
}

pub fn user_token_account(user: &Pubkey, mint: &Pubkey, token_program_id: &Pubkey) -> Pubkey {
    associated_token_address(user, mint, token_program_id)
}
