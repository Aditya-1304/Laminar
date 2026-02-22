use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
};
use anchor_lang::InstructionData;
use laminar::program::Laminar;

declare_id!("E5L3jT8u2qEp9UYXPq2DD97fsVvtzhywHXPRohznfhQr");

#[program]
pub mod cpi_tester {
    use super::*;

    /// Direct CPI into Laminar's public mint entrypoint.
    ///
    /// Expected result in tests:
    /// Laminar must reject with `InvalidCPIContext`.
    pub fn cpi_mint_asol(
        ctx: Context<ProxyMintAsol>,
        lst_amount: u64,
        min_asol_out: u64,
    ) -> Result<()> {
        forward_mint_asol_cpi(&ctx, lst_amount, min_asol_out)
    }

    /// client -> cpi_tester(cpi_nested_mint_asol) -> cpi_tester(cpi_mint_asol) -> laminar
    ///
    /// This creates deeper stack depth than direct CPI and should also
    /// be rejected by Laminar's CPI guard.
    pub fn cpi_nested_mint_asol(
        ctx: Context<ProxyMintAsol>,
        lst_amount: u64,
        min_asol_out: u64,
    ) -> Result<()> {
        let ix = Instruction {
            program_id: crate::ID,
            accounts: vec![
                AccountMeta::new(ctx.accounts.user.key(), true),
                AccountMeta::new(ctx.accounts.global_state.key(), false),
                AccountMeta::new(ctx.accounts.asol_mint.key(), false),
                AccountMeta::new(ctx.accounts.user_asol_account.key(), false),
                AccountMeta::new(ctx.accounts.treasury_asol_account.key(), false),
                AccountMeta::new_readonly(ctx.accounts.treasury.key(), false),
                AccountMeta::new(ctx.accounts.user_lst_account.key(), false),
                AccountMeta::new(ctx.accounts.vault.key(), false),
                AccountMeta::new_readonly(ctx.accounts.vault_authority.key(), false),
                AccountMeta::new_readonly(ctx.accounts.lst_mint.key(), false),
                AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
                AccountMeta::new_readonly(ctx.accounts.associated_token_program.key(), false),
                AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
                AccountMeta::new_readonly(ctx.accounts.clock.key(), false),
                AccountMeta::new_readonly(ctx.accounts.cpi_tester_program.key(), false),
                AccountMeta::new_readonly(ctx.accounts.laminar_program.key(), false),
            ],
            data: crate::instruction::CpiMintAsol {
                lst_amount,
                min_asol_out,
            }
            .data(),
        };

        let infos = vec![
            ctx.accounts.user.to_account_info(),
            ctx.accounts.global_state.to_account_info(),
            ctx.accounts.asol_mint.to_account_info(),
            ctx.accounts.user_asol_account.to_account_info(),
            ctx.accounts.treasury_asol_account.to_account_info(),
            ctx.accounts.treasury.to_account_info(),
            ctx.accounts.user_lst_account.to_account_info(),
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.vault_authority.to_account_info(),
            ctx.accounts.lst_mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.associated_token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.clock.to_account_info(),
            ctx.accounts.cpi_tester_program.to_account_info(),
            ctx.accounts.laminar_program.to_account_info(),
        ];

        invoke(&ix, &infos)?;
        Ok(())
    }
}

/// Forwards CPI into Laminar mint_asol public entrypoint.
///
/// Laminar's user-facing entrypoints should reject CPI caller context.
fn forward_mint_asol_cpi(
    ctx: &Context<ProxyMintAsol>,
    lst_amount: u64,
    min_asol_out: u64,
) -> Result<()> {
    let cpi_accounts = laminar::cpi::accounts::MintAsol {
        user: ctx.accounts.user.to_account_info(),
        global_state: ctx.accounts.global_state.to_account_info(),
        asol_mint: ctx.accounts.asol_mint.to_account_info(),
        user_asol_account: ctx.accounts.user_asol_account.to_account_info(),
        treasury_asol_account: ctx.accounts.treasury_asol_account.to_account_info(),
        treasury: ctx.accounts.treasury.to_account_info(),
        user_lst_account: ctx.accounts.user_lst_account.to_account_info(),
        vault: ctx.accounts.vault.to_account_info(),
        vault_authority: ctx.accounts.vault_authority.to_account_info(),
        lst_mint: ctx.accounts.lst_mint.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        clock: ctx.accounts.clock.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(ctx.accounts.laminar_program.to_account_info(), cpi_accounts);
    laminar::cpi::mint_asol(cpi_ctx, lst_amount, min_asol_out)
}

#[derive(Accounts)]
pub struct ProxyMintAsol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// Laminar GlobalState account.
    /// CHECK: Validated by Laminar program during CPI.
    #[account(mut)]
    pub global_state: UncheckedAccount<'info>,

    /// Laminar aSOL mint.
    /// CHECK: Validated by Laminar program during CPI.
    #[account(mut)]
    pub asol_mint: UncheckedAccount<'info>,

    /// User's aSOL ATA.
    /// CHECK: Validated by Laminar program during CPI.
    #[account(mut)]
    pub user_asol_account: UncheckedAccount<'info>,

    /// Treasury aSOL ATA.
    /// CHECK: Validated by Laminar program during CPI.
    #[account(mut)]
    pub treasury_asol_account: UncheckedAccount<'info>,

    /// Laminar treasury authority.
    /// CHECK: Validated by Laminar program during CPI.
    pub treasury: UncheckedAccount<'info>,

    /// User's LST ATA.
    /// CHECK: Validated by Laminar program during CPI.
    #[account(mut)]
    pub user_lst_account: UncheckedAccount<'info>,

    /// Laminar vault ATA.
    /// CHECK: Validated by Laminar program during CPI.
    #[account(mut)]
    pub vault: UncheckedAccount<'info>,

    /// Laminar vault authority PDA.
    /// CHECK: Validated by Laminar program during CPI.
    pub vault_authority: UncheckedAccount<'info>,

    /// Supported LST mint.
    /// CHECK: Validated by Laminar program during CPI.
    pub lst_mint: UncheckedAccount<'info>,

    /// Token program account.
    /// CHECK: Laminar validates expected token program.
    pub token_program: UncheckedAccount<'info>,

    /// ATA program account.
    /// CHECK: Laminar validates expected ATA program.
    pub associated_token_program: UncheckedAccount<'info>,

    /// System program account.
    /// CHECK: Laminar validates expected system program.
    pub system_program: UncheckedAccount<'info>,

    /// Clock sysvar.
    /// CHECK: Laminar reads clock slot for freshness checks.
    pub clock: UncheckedAccount<'info>,

    /// Explicit self-program account for nested self-invoke.
    /// CHECK: Address-constrained to this program ID.
    #[account(address = crate::ID)]
    pub cpi_tester_program: UncheckedAccount<'info>,

    /// Laminar program account for CPI target.
    pub laminar_program: Program<'info, Laminar>,
}
