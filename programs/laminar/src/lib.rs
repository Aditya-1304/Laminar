use anchor_lang::prelude::*;

pub mod math;
pub mod invariants;
pub mod state;
pub mod instructions;
pub mod error;
pub mod events;
pub mod reentrancy;

use instructions::*;

declare_id!("DNJkHdH2tzCG9V8RX2bKRZKHxZccYBkBjqqSsG9midvc");

#[program]
pub mod laminar {
    use crate::reentrancy::ReentrancyGuard;

    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        min_cr_bps: u64,
        target_cr_bps: u64,
        mock_sol_price_usd: u64,
        mock_lst_to_sol_rate: u64,
    ) -> Result<()> {
        instructions::initialize::handler(
            ctx,
            min_cr_bps,
            target_cr_bps,
            mock_sol_price_usd,
            mock_lst_to_sol_rate,
        )
    }

    /// Mint amUSD by depositing LST collateral
    pub fn mint_amusd(
        ctx: Context<MintAmUSD>,
        lst_amount: u64,
        min_amusd_out: u64,
    ) -> Result<()> {
        instructions::mint_amusd::handler(ctx, lst_amount, min_amusd_out)
    }

    /// Redeem amUSD by burning debt and receiving LST
    pub fn redeem_amusd(
        ctx: Context<RedeemAmUSD>,
        amusd_amount: u64,
        min_lst_out: u64,
    ) -> Result<()> {
        instructions::redeem_amusd::handler(ctx, amusd_amount, min_lst_out)
    }

    /// Mint aSOL by depositing LST collateral at NAV
    pub fn mint_asol(
        ctx: Context<MintAsol>,
        lst_amount: u64,
        min_asol_out: u64,
    ) -> Result<()> {
        instructions::mint_asol::handler(ctx, lst_amount, min_asol_out)
    }

    /// Redeem aSOL by burning equity and receiving LST at NAV
    pub fn redeem_asol(
        ctx: Context<RedeemAsol>,
        asol_amount: u64,
        min_lst_out: u64
    ) -> Result<()> {
        instructions::redeem_asol::handler(ctx, asol_amount, min_lst_out)
    }

    /// Emergency pause control (admin only)
    pub fn emergency_pause(
        ctx: Context<EmergencyPause>,
        mint_paused: bool,
        redeem_paused: bool,
    ) -> Result<()> {
        let guard = ReentrancyGuard::new(&mut ctx.accounts.global_state)?;
        guard.state.mint_paused = mint_paused;
        guard.state.redeem_paused = redeem_paused;

        emit!(crate::events::EmergencyPause {
            authority: ctx.accounts.authority.key(),
            mint_paused,
            redeem_paused,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority,
        seeds = [b"global_state"],
        bump
    )]
    pub global_state: Account<'info, state::GlobalState>
}

