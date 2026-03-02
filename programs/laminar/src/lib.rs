use anchor_lang::prelude::*;

pub mod math;
pub mod invariants;
pub mod state;
pub mod instructions;
pub mod error;
pub mod events;
pub mod constants;
pub mod oracle;
// pub mod reentrancy;

use instructions::*;

use crate::state::GLOBAL_STATE_SEED;
use crate::constants::{
    LST_RATE_BACKEND_MOCK, LST_RATE_BACKEND_SANCTUM_STAKE_POOL, ORACLE_BACKEND_MOCK,
    ORACLE_BACKEND_PYTH_PUSH,
};
use crate::math::{mul_div_up, BPS_PRECISION};


declare_id!("DNJkHdH2tzCG9V8RX2bKRZKHxZccYBkBjqqSsG9midvc");

#[program]
pub mod laminar {
    // use crate::reentrancy::ReentrancyGuard;

    use crate::error::LaminarError;

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
        let global_state = &mut ctx.accounts.global_state;
        global_state.mint_paused = mint_paused;
        global_state.redeem_paused = redeem_paused;
        global_state.operation_counter = global_state.operation_counter.saturating_add(1);

        emit!(crate::events::EmergencyPause {
            authority: ctx.accounts.authority.key(),
            mint_paused,
            redeem_paused,
            timestamp: ctx.accounts.clock.unix_timestamp,
        });
        Ok(())
    }

    pub fn update_mock_prices(
        ctx: Context<UpdateMockPrices>,
        new_sol_price_usd: u64,
        new_lst_to_sol_rate: u64,
        new_oracle_confidence_usd: u64,
    ) -> Result<()> {
        
        let global_state = &mut ctx.accounts.global_state;

        require!(
            global_state.oracle_backend == ORACLE_BACKEND_MOCK,
            LaminarError::UnsupportedOracleBackend
        );
        require!(
            global_state.lst_rate_backend == LST_RATE_BACKEND_MOCK,
            LaminarError::UnsupportedLstRateBackend
        );

        require!(
            new_oracle_confidence_usd < new_sol_price_usd,
            LaminarError::SafePriceInvalid
        );

        let uncertainty_index_bps = mul_div_up(
            new_oracle_confidence_usd,
            BPS_PRECISION,
            new_sol_price_usd,
        )
        .ok_or(LaminarError::ArithmeticOverflow)?;
        require!(
            uncertainty_index_bps <= global_state.max_conf_bps,
            LaminarError::OracleConfidenceTooHigh
        );
        
        require!(new_sol_price_usd > 0, LaminarError::ZeroAmount);
        require!(new_lst_to_sol_rate > 0, LaminarError::ZeroAmount);
        
        let old_sol_price = global_state.mock_sol_price_usd;
        let old_lst_rate = global_state.mock_lst_to_sol_rate;
        
        global_state.mock_sol_price_usd = new_sol_price_usd;
        global_state.mock_lst_to_sol_rate = new_lst_to_sol_rate;
        global_state.operation_counter = global_state.operation_counter.saturating_add(1);
        global_state.mock_oracle_confidence_usd = new_oracle_confidence_usd;
        global_state.last_oracle_update_slot = ctx.accounts.clock.slot;
        global_state.last_tvl_update_slot = ctx.accounts.clock.slot;
        global_state.uncertainty_index_bps = uncertainty_index_bps;
        global_state.last_lst_update_epoch = ctx.accounts.clock.epoch;


        msg!(
            "Oracle snapshot updated: slot={}, price={}, conf={}, lst_rate={}",
            ctx.accounts.clock.slot,
            new_sol_price_usd,
            new_oracle_confidence_usd,
            new_lst_to_sol_rate
        );

        
        emit!(crate::events::OraclePriceUpdated {
            authority: ctx.accounts.authority.key(),
            old_sol_price,
            new_sol_price: new_sol_price_usd,
            old_lst_rate,
            new_lst_rate: new_lst_to_sol_rate,
            timestamp: ctx.accounts.clock.unix_timestamp,
        });
        
        Ok(())
    }
    
    /// Update risk parameters (admin only)
    pub fn update_parameters(
        ctx: Context<UpdateParameters>,
        new_min_cr_bps: u64,
        new_target_cr_bps: u64,
    ) -> Result<()> {
        require!(new_min_cr_bps >= 10_000, LaminarError::InvalidParameter);
        require!(new_target_cr_bps > new_min_cr_bps, LaminarError::InvalidParameter);
        
        let global_state = &mut ctx.accounts.global_state;
        
        let old_min = global_state.min_cr_bps;
        let old_target = global_state.target_cr_bps;
        
        global_state.min_cr_bps = new_min_cr_bps;
        global_state.target_cr_bps = new_target_cr_bps;
        global_state.operation_counter = global_state.operation_counter.saturating_add(1);
        
        emit!(crate::events::ParametersUpdated {
            authority: ctx.accounts.authority.key(),
            old_min_cr_bps: old_min,
            new_min_cr_bps,
            old_target_cr_bps: old_target,
            new_target_cr_bps,
            timestamp: ctx.accounts.clock.unix_timestamp,
        });
        
        Ok(())
    }

    pub fn sync_exchange_rate(ctx: Context<SyncExchangeRate>) -> Result<()> {
        instructions::sync_exchange_rate::handler(ctx)
    }

    /// Configure oracle and LST rate backends
    /// Admin-only and intended to switch between local mock mode and production
    pub fn set_oracle_sources(
        ctx: Context<SetOracleSources>,
        oracle_backend: u8,
        pyth_sol_usd_price_account: Pubkey,
        lst_rate_backend: u8,
        lst_stake_pool: Pubkey,
    ) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;

        match oracle_backend {
            ORACLE_BACKEND_MOCK => {}
            ORACLE_BACKEND_PYTH_PUSH => {
                require!(
                    pyth_sol_usd_price_account != Pubkey::default(),
                    LaminarError::OracleFeedNotSet
                );
            }
            _=> return err!(LaminarError::UnsupportedOracleBackend),
        }

        match lst_rate_backend {
            LST_RATE_BACKEND_MOCK => {}
            LST_RATE_BACKEND_SANCTUM_STAKE_POOL => {
                require!(lst_stake_pool != Pubkey::default(), LaminarError::LstStakePoolNotSet);
            }
            _ => return err!(LaminarError::UnsupportedLstRateBackend),
        }

        global_state.oracle_backend = oracle_backend;
        global_state.pyth_sol_usd_price_account = pyth_sol_usd_price_account;
        global_state.lst_rate_backend = lst_rate_backend;
        global_state.lst_stake_pool = lst_stake_pool;
        global_state.operation_counter = global_state.operation_counter.saturating_add(1);

        emit!(crate::events::OracleConfigUpdated {
            authority: ctx.accounts.authority.key(),
            oracle_backend,
            pyth_sol_usd_price_account,
            lst_rate_backend,
            lst_stake_pool,
            timestamp: ctx.accounts.clock.unix_timestamp,
        });

        Ok(())
    }

    /// Refresh uncertainty index from current oracle confidence snapshot.
    /// Permissionless: anyone may pay to keep oracle snapshot warm.
    pub fn update_uncertainty_index(ctx: Context<UpdateUncertaintyIndex>) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.validate_version()?;

        let pricing = crate::oracle::load_oracle_pricing_in_place(
            global_state,
            &ctx.accounts.clock,
            ctx.remaining_accounts,
        )?;

        global_state.operation_counter = global_state.operation_counter.saturating_add(1);

        emit!(crate::events::OracleSnapshotUpdated {
            updater: ctx.accounts.updater.key(),
            oracle_backend: global_state.oracle_backend,
            ema_price_usd: pricing.price_ema_usd,
            safe_price_usd: pricing.price_safe_usd,
            confidence_usd: pricing.confidence_usd,
            uncertainty_index_bps: pricing.uncertainty_index_bps,
            slot: ctx.accounts.clock.slot,
            timestamp: ctx.accounts.clock.unix_timestamp,
        });

        Ok(())
    }

    /// Read-only safe-price quote.
    pub fn get_safe_price(ctx: Context<GetSafePrice>) -> Result<()> {
        let global_state = &ctx.accounts.global_state;
        global_state.validate_version()?;

        let pricing = crate::oracle::quote_safe_price(
            global_state,
            &ctx.accounts.clock,
            ctx.remaining_accounts,
        )?;

        emit!(crate::events::SafePriceQuoted {
            requester: ctx.accounts.caller.key(),
            oracle_backend: global_state.oracle_backend,
            ema_price_usd: pricing.price_ema_usd,
            safe_price_usd: pricing.price_safe_usd,
            confidence_usd: pricing.confidence_usd,
            uncertainty_index_bps: pricing.uncertainty_index_bps,
            slot: ctx.accounts.clock.slot,
            timestamp: ctx.accounts.clock.unix_timestamp,
        });

        Ok(())
    }

        pub fn initialize_stability_pool(ctx: Context<InitializeStabilityPool>) -> Result<()> {
        instructions::stability_pool::initialize_stability_pool_handler(ctx)
    }

    pub fn deposit_amusd(
        ctx: Context<DepositAmUSD>,
        amusd_amount: u64,
        min_samusd_out: u64,
    ) -> Result<()> {
        instructions::stability_pool::deposit_amusd_handler(ctx, amusd_amount, min_samusd_out)
    }

    pub fn withdraw_underlying(
        ctx: Context<WithdrawUnderlying>,
        samusd_amount: u64,
        min_amusd_out: u64,
        min_asol_out: u64,
    ) -> Result<()> {
        instructions::stability_pool::withdraw_underlying_handler(
            ctx,
            samusd_amount,
            min_amusd_out,
            min_asol_out,
        )
    }

    pub fn harvest_yield(ctx: Context<HarvestYield>) -> Result<()> {
        instructions::stability_pool::harvest_yield_handler(ctx)
    }

    pub fn execute_debt_equity_swap(ctx: Context<ExecuteDebtEquitySwap>) -> Result<()> {
        instructions::stability_pool::execute_debt_equity_swap_handler(ctx)
    }

    pub fn set_stability_withdrawals_paused(
        ctx: Context<SetStabilityWithdrawalsPaused>,
        withdrawals_paused: bool,
    ) -> Result<()> {
        instructions::stability_pool::set_stability_withdrawals_paused_handler(ctx, withdrawals_paused)
    }

}

#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority,
        seeds = [GLOBAL_STATE_SEED],
        bump
    )]
    pub global_state: Account<'info, state::GlobalState>,
    pub clock: Sysvar<'info, Clock>,
}


#[derive(Accounts)]
pub struct UpdateMockPrices<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority,
        seeds = [GLOBAL_STATE_SEED],
        bump
    )]
    pub global_state: Account<'info, state::GlobalState>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdateParameters<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority,
        seeds = [GLOBAL_STATE_SEED],
        bump
    )]
    pub global_state: Account<'info, state::GlobalState>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct SetOracleSources<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority,
        seeds = [GLOBAL_STATE_SEED],
        bump
    )]
    pub global_state: Account<'info, state::GlobalState>,

    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdateUncertaintyIndex<'info> {
    #[account(mut)]
    pub updater: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_STATE_SEED],
        bump,
        constraint = global_state.to_account_info().owner == &crate::ID @ crate::error::LaminarError::InvalidAccountOwner,
    )]
    pub global_state: Account<'info, state::GlobalState>,

    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct GetSafePrice<'info> {
    pub caller: Signer<'info>,

    #[account(
        seeds = [GLOBAL_STATE_SEED],
        bump,
        constraint = global_state.to_account_info().owner == &crate::ID @ crate::error::LaminarError::InvalidAccountOwner,
    )]
    pub global_state: Account<'info, state::GlobalState>,

    pub clock: Sysvar<'info, Clock>,
}
