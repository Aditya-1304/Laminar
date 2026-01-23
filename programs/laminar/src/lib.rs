use anchor_lang::prelude::*;

mod math;
mod invariants;
mod state;

declare_id!("DNJkHdH2tzCG9V8RX2bKRZKHxZccYBkBjqqSsG9midvc");

#[program]
pub mod laminar {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
