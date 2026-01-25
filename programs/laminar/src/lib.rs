use anchor_lang::prelude::*;

pub mod math;
pub mod invariants;
pub mod state;
pub mod instructions;
pub mod error;

use instructions::*;

declare_id!("DNJkHdH2tzCG9V8RX2bKRZKHxZccYBkBjqqSsG9midvc");

#[program]
pub mod laminar {
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
    ) -> Result<()> {
        instructions::mint_amusd::handler(ctx, lst_amount)
    }

    /// Redeem amUSD by burning debt and receiving LST
    pub fn redeem_amusd(
        ctx: Context<RedeemAmUSD>,
        amusd_amount: u64,
    ) -> Result<()> {
        instructions::redeem_amusd::handler(ctx, amusd_amount)
    }

    pub fn mint_asol(
        ctx: Context<MintAsol>,
        lst_amount: u64,
    ) -> Result<()> {
        instructions::mint_asol::handler(ctx, lst_amount)
    }
}
