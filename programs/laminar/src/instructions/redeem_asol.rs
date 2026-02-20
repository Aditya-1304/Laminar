//! Redeem aSOL instruction - exits leveraged equity position
//! User burns aSOL and receives LST collateral back at current NAV


use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, Burn}
};
use crate::{constants:: MIN_PROTOCOL_TVL, events::AsolRedeemed, instructions::sync_exchange_rate_in_place, state::*};
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;


pub fn handler(
  ctx: Context<RedeemAsol>,
  asol_amount: u64,
  min_lst_out: u64,
) -> Result<()> {
  // All validations before any state changes
  
  assert_not_cpi_context()?;

  // sync first
  {
  let global_state = &mut ctx.accounts.global_state;
  global_state.validate_version()?;
  assert_lst_snapshot_fresh(
    ctx.accounts.clock.slot,
    global_state.last_tvl_update_slot,
    global_state.max_oracle_staleness_slots,
  )?;
  sync_exchange_rate_in_place(global_state, ctx.accounts.clock.slot)?;
  }

  // read only borrow
  let global_state = &ctx.accounts.global_state;

  assert_oracle_freshness_and_confidence(
    ctx.accounts.clock.slot, 
    global_state.last_oracle_update_slot, 
    global_state.max_oracle_staleness_slots, 
    global_state.mock_sol_price_usd, 
    global_state.mock_oracle_confidence_usd, 
    global_state.max_conf_bps
  )?;
  
  // Capture values
  let lst_to_sol_rate = global_state.mock_lst_to_sol_rate;
  let sol_price_used = global_state.mock_sol_price_usd;
  let current_lst_amount = global_state.total_lst_amount;
  let current_amusd_supply = global_state.amusd_supply;
  let current_asol_supply = global_state.asol_supply;
  let target_cr_bps = global_state.target_cr_bps;
  let min_cr_bps = global_state.min_cr_bps;
  let current_rounding_reserve = global_state.rounding_reserve_lamports;
  let fee_asol_redeem_bps = global_state.fee_asol_redeem_bps;
  let fee_min_multiplier_bps = global_state.fee_min_multiplier_bps;
  let fee_max_multiplier_bps = global_state.fee_max_multiplier_bps;
  let uncertainty_index_bps = global_state.uncertainty_index_bps;
  let uncertainty_max_bps = global_state.uncertainty_max_bps;


  // Configured hard cap for reserve growth
  let max_rounding_reserve = global_state.max_rounding_reserve_lamports;

  // Validations
  require!(!global_state.redeem_paused, LaminarError::RedeemPaused);
  require!(asol_amount > 0, LaminarError::ZeroAmount);
  // require!(min_lst_out > 0, LaminarError::ZeroAmount);
  // require!(min_lst_out >= MIN_LST_DEPOSIT, LaminarError::AmountTooSmall);

  msg!("aSOL to redeem: {}", asol_amount);

  // All math logic

  let old_tvl = compute_tvl_sol(current_lst_amount, lst_to_sol_rate).ok_or(LaminarError::MathOverflow)?;

  // let current_tvl = compute_tvl_sol(current_lst_amount, lst_to_sol_rate)
  //   .ok_or(LaminarError::MathOverflow)?;

  let current_liability = if current_amusd_supply > 0 {
    compute_liability_sol(current_amusd_supply, sol_price_used)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let old_claimable_equity = compute_claimable_equity_sol(old_tvl, current_liability, current_rounding_reserve).ok_or(LaminarError::MathOverflow)?;

  let old_cr_bps = compute_cr_bps(old_tvl, current_liability);

  let fee_bps = compute_dynamic_fee_bps(fee_asol_redeem_bps, FeeAction::AsolRedeem, old_cr_bps, min_cr_bps, target_cr_bps, fee_min_multiplier_bps, fee_max_multiplier_bps, uncertainty_index_bps, uncertainty_max_bps).ok_or(LaminarError::InvalidParameter)?;

  let (asol_net_in, asol_fee_in) = apply_fee(asol_amount, fee_bps)
    .ok_or(LaminarError::MathOverflow)?;
  require!(asol_net_in > 0, LaminarError::AmountTooSmall);

  msg!("aSOL input: {}", asol_amount);
  msg!("aSOL fee (to treasury): {}", asol_fee_in);
  msg!("aSOL net burn basis: {}", asol_net_in);

  let solvent_mode = old_cr_bps >= BPS_PRECISION;

  let current_nav = nav_asol_with_reserve(old_tvl, current_liability, current_rounding_reserve, current_asol_supply)
    .ok_or(LaminarError::InsolventProtocol)?;
  require!(current_nav > 0, LaminarError::InsolventProtocol);

  msg!("Current aSOL NAV: {} lamports per aSOL", current_nav);

  require!(min_lst_out > 0, LaminarError::ZeroAmount);
  require!(min_lst_out >= MIN_LST_DEPOSIT, LaminarError::AmountTooSmall);

  let sol_value_down = mul_div_down(asol_net_in, current_nav, SOL_PRECISION)
    .ok_or(LaminarError::MathOverflow)?;
  let lst_gross_down = mul_div_down(sol_value_down, SOL_PRECISION, lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  // - Solvent (CR >= 100%): user-favoring rounding (up, up), reserve debited
  // - Insolvent (CR < 100%): conservative rounding (down, down), no reserve debit
    let (sol_value_gross, lst_gross, reserve_debit_from_redeem) = if solvent_mode {
    let sol_value_up = mul_div_up(asol_net_in, current_nav, SOL_PRECISION)
      .ok_or(LaminarError::MathOverflow)?;
    let lst_gross_up = mul_div_up(sol_value_up, SOL_PRECISION, lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    let redeem_rounding_delta_lst = compute_rounding_delta_units(lst_gross_down, lst_gross_up)
      .ok_or(LaminarError::MathOverflow)?;
    let lamport_debit = lst_dust_to_lamports_up(redeem_rounding_delta_lst, lst_to_sol_rate)
      .ok_or(LaminarError::MathOverflow)?;

    if lamport_debit <= current_rounding_reserve {
      (sol_value_up, lst_gross_up, lamport_debit)
    } else {
      msg!(
        "Rounding reserve insufficient for user-favoring redeem rounding; fallback to conservative path"
      );
      (sol_value_down, lst_gross_down, 0u64)
    }
  } else {
    (sol_value_down, lst_gross_down, 0u64)
  };


  msg!("SOL value (before fee): {}", sol_value_gross);
  msg!("LST gross to user: {}", lst_gross);

  let lst_out = lst_gross;
  require!(lst_out >= min_lst_out, LaminarError::SlippageExceeded);

  let total_lst_out = lst_out;

  // Calculate new state values
  let new_lst_amount = current_lst_amount
    .checked_sub(total_lst_out)
    .ok_or(LaminarError::InsufficientCollateral)?;

  require!(
    new_lst_amount >= MIN_PROTOCOL_TVL || new_lst_amount == 0,
    LaminarError::BelowMinimumTVL
  );

  let new_tvl = compute_tvl_sol(new_lst_amount, lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let new_asol_supply = current_asol_supply
    .checked_sub(asol_net_in)
    .ok_or(LaminarError::InsufficientSupply)?;

  let new_liability = current_liability;  // aSOL redeem doesn't change liability
  let new_rounding_reserve = debit_rounding_reserve(
    current_rounding_reserve,
    reserve_debit_from_redeem,
  )?;

  let new_accounting_equity = compute_accounting_equity_sol(new_tvl, new_liability, new_rounding_reserve).ok_or(LaminarError::MathOverflow)?;

  let new_claimable_equity = compute_claimable_equity_sol(new_tvl, new_liability, new_rounding_reserve).ok_or(LaminarError::MathOverflow)?;

  let new_cr_bps = if new_liability > 0 {
    compute_cr_bps(new_tvl, new_liability)
  } else {
      u64::MAX
  };

  assert_cr_above_minimum(new_cr_bps, min_cr_bps)?;

  if new_cr_bps == u64::MAX {
    msg!("Post-redeem CR: inf (no amUSD liability)");
  } else {
    msg!("Post-redeem CR: {}bps ({}%)", new_cr_bps, new_cr_bps / 100);
  }

  // Deterministic rounding bound for redeem_asol path:
  // (aSOL->SOL, SOL->LST) => (k_lamports=2, k_usd=0)
  let rounding_bound_lamports =
    derive_rounding_bound_lamports(2, 0, sol_price_used)?;

  require!(
    ctx.accounts.user_asol_account.amount >= asol_amount,
    LaminarError::InsufficientSupply
  );

  // Verify vault has enough funds
  require!(
    ctx.accounts.vault.amount >= total_lst_out,
    LaminarError::InsufficientCollateral
  );

  // Invariant checks
  assert_rounding_reserve_within_cap(new_rounding_reserve, max_rounding_reserve)?;
  assert_balance_sheet_holds(new_tvl, new_liability, new_accounting_equity, new_rounding_reserve, rounding_bound_lamports)?;

  // Update state BEFORE external calls

  {
    let global_state = &mut ctx.accounts.global_state;
    global_state.total_lst_amount = new_lst_amount;
    global_state.asol_supply = new_asol_supply;
    global_state.operation_counter = global_state.operation_counter.saturating_add(1);
    global_state.rounding_reserve_lamports = new_rounding_reserve;
    msg!("State updated: LST={}, aSOL={}", new_lst_amount, new_asol_supply);
  }

  // External calls (CPIs)

  // Transfer fee to treasury
  if asol_fee_in > 0 {
    let transfer_treasury_accounts = TransferChecked {
      from: ctx.accounts.user_asol_account.to_account_info(),
      mint: ctx.accounts.asol_mint.to_account_info(),
      to: ctx.accounts.treasury_asol_account.to_account_info(),
      authority: ctx.accounts.user.to_account_info(),
    };

    let cpi_ctx_fee = CpiContext::new(
      ctx.accounts.token_program.to_account_info(),
      transfer_treasury_accounts,
    );

    token_interface::transfer_checked(cpi_ctx_fee, asol_fee_in, ctx.accounts.asol_mint.decimals)?;
    msg!("Transferred {} aSOL fee to treasury", asol_fee_in);
  }

  // Burn aSOL from user
  let burn_accounts = Burn {
    mint: ctx.accounts.asol_mint.to_account_info(),
    from: ctx.accounts.user_asol_account.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  let cpi_ctx_burn = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
    burn_accounts
  );

  token_interface::burn(cpi_ctx_burn, asol_net_in)?;
  msg!("Burned {} aSOL from user", asol_net_in);

  // Transfer LST from vault to user
  let seeds = &[VAULT_AUTHORITY_SEED, &[ctx.accounts.global_state.vault_authority_bump]];
  let signer = &[&seeds[..]];

  let transfer_user_accounts = TransferChecked {
    from: ctx.accounts.vault.to_account_info(),
    mint: ctx.accounts.lst_mint.to_account_info(),
    to: ctx.accounts.user_lst_account.to_account_info(),
    authority: ctx.accounts.vault_authority.to_account_info(),
  };

  let cpi_ctx_user = CpiContext::new_with_signer(
    ctx.accounts.token_program.to_account_info(),
    transfer_user_accounts,
    signer
  );

  token_interface::transfer_checked(cpi_ctx_user, lst_out, ctx.accounts.lst_mint.decimals)?;
  msg!("Transferred {} LST to user", lst_out);


  ctx.accounts.asol_mint.reload()?;
  ctx.accounts.vault.reload()?;

  let expected_vault_balance = ctx.accounts.global_state.total_lst_amount;
  require!(
    ctx.accounts.vault.amount == expected_vault_balance,
    LaminarError::BalanceSheetViolation
  );

  require!(
    ctx.accounts.asol_mint.supply == ctx.accounts.global_state.asol_supply,
    LaminarError::BalanceSheetViolation
  );

  msg!("Redeem complete!");
  msg!("New TVL: {} lamports", new_tvl);
  msg!("New aSOL supply: {}", new_asol_supply);

  emit!(AsolRedeemed {
    user: ctx.accounts.user.key(),
    asol_burned: asol_net_in,
    lst_received: lst_out,
    fee: asol_fee_in,
    nav: current_nav,
    old_tvl,
    new_tvl,
    old_equity: old_claimable_equity,
    new_equity: new_claimable_equity,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });

  Ok(())
}

#[derive(Accounts)]
pub struct RedeemAsol<'info> {
  #[account(mut)]
  pub user: Signer<'info>,

  /// GlobalState PDA
  #[account(
    mut,
    seeds = [GLOBAL_STATE_SEED],
    bump,
    has_one = asol_mint,
    has_one = treasury,
    constraint = global_state.to_account_info().owner == &crate::ID @ LaminarError::InvalidAccountOwner,
  )]
  pub global_state: Box<Account<'info, GlobalState>>,

  /// aSOL mint
  #[account(
    mut,
    constraint = asol_mint.mint_authority == anchor_lang::solana_program::program_option::COption::Some(global_state.key()) @ LaminarError::InvalidMintAuthority,
  )]
  pub asol_mint: Box<InterfaceAccount<'info, Mint>>,

  /// User's aSOL token account (source of burned aSOL)
  #[account(
    mut,
    token::mint = asol_mint,
    token::authority = user,
    constraint = user_asol_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// Treasury's LST token account (receives redemption fee)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = asol_mint,
    associated_token::authority = treasury,
    associated_token::token_program = token_program,
    // constraint = treasury_asol_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub treasury_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// User's LST token account (receives redeemed LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
    constraint = user_lst_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (source of LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
    constraint = vault.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Vault authority PDA - signs transfers from vault
  /// CHECK: PDA validated by seeds
  #[account(
    seeds = [VAULT_AUTHORITY_SEED],
    bump,
  )]
  pub vault_authority: UncheckedAccount<'info>,

  /// LST mint
  #[account(
    constraint = lst_mint.key() == global_state.supported_lst_mint @ LaminarError::UnsupportedLST
  )]
  pub lst_mint: Box<InterfaceAccount<'info, Mint>>,

  pub token_program: Interface<'info, TokenInterface>,
  pub associated_token_program: Program<'info, AssociatedToken>,
  pub system_program: Program<'info, System>,

  pub clock: Sysvar<'info, Clock>,
}
