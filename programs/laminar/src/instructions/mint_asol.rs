//! Mint aSOL instruction - Creates leveraged equity position
//! User deposits LST collateral and receives aSOL at current NAV
use anchor_lang::prelude::*;
use anchor_spl::{
  associated_token::AssociatedToken,
  token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked, MintTo}
};
use crate::{ events::AsolMinted, instructions::sync_exchange_rate_in_place, state::*};
use crate::math::*;
use crate::invariants::*;
use crate::error::LaminarError;


pub fn handler(
  ctx: Context<MintAsol>,
  lst_amount: u64,
  min_asol_out: u64,
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
  let fee_asol_mint_bps = global_state.fee_asol_mint_bps;
  let fee_min_multiplier_bps = global_state.fee_min_multiplier_bps;
  let fee_max_multiplier_bps = global_state.fee_max_multiplier_bps;
  let uncertainty_index_bps = global_state.uncertainty_index_bps;
  let uncertainty_max_bps = global_state.uncertainty_max_bps;

  let current_rounding_reserve = global_state.rounding_reserve_lamports;

  // Configured hard cap for reserve growth.
  let max_rounding_reserve = global_state.max_rounding_reserve_lamports;

  // Input validations
  require!(!global_state.mint_paused, LaminarError::MintPaused);
  require!(lst_amount > 0, LaminarError::ZeroAmount);
  require!(lst_amount >= MIN_LST_DEPOSIT, LaminarError::AmountTooSmall);

  require!(
    ctx.accounts.user_lst_account.amount >= lst_amount,
    LaminarError::InsufficientCollateral
  );

  // All math logic

  let old_tvl = compute_tvl_sol(current_lst_amount, lst_to_sol_rate).ok_or(LaminarError::MathOverflow)?;

  let current_liability = if current_amusd_supply > 0 {
    compute_liability_sol(current_amusd_supply, sol_price_used)
      .ok_or(LaminarError::MathOverflow)?
  } else {
    0
  };

  let old_claimable_equity = compute_claimable_equity_sol(old_tvl, current_liability, current_rounding_reserve).ok_or(LaminarError::MathOverflow)?;
  let old_cr_bps = compute_cr_bps(old_tvl, current_liability);

  // Determinstic rounding bound for mint_asol path:
  // (LST-> SOL, SOL-> aSOL) => (k_lamports=2, k_usd=0)
  let rounding_bound_lamports = derive_rounding_bound_lamports(2, 0, sol_price_used)?;

  // May be increased bt orphan-equity dust sweep in bootstrap mode.
  let mut effective_rounding_reserve = current_rounding_reserve;

  if current_asol_supply == 0 {
    // Bootstrap must be solvent
    require!(old_tvl >= current_liability, LaminarError::InsolventProtocol);

    // Bootstrap requires TVL -= L + R (within deterministic rounding bound).
    let lhs = old_tvl as i128;
    let rhs = (current_liability as i128)
      .checked_add(effective_rounding_reserve as i128)
      .ok_or(LaminarError::MathOverflow)?;

    let bootstrap_diff: u128 = if lhs > rhs {
      (lhs - rhs) as u128
    } else {
      (rhs - lhs) as u128
    };

    require!(
      bootstrap_diff <= rounding_bound_lamports as u128,
      LaminarError::EquityWithoutAsolSupply
    );

    // Orphan-equity dust sweep:
    // if claimable equity is dust-only, reclassify it into rounding reserve.
    if old_claimable_equity > 0 {
      effective_rounding_reserve = effective_rounding_reserve
        .checked_add(old_claimable_equity)
        .ok_or(LaminarError::MathOverflow)?;

      require!(
        effective_rounding_reserve <= max_rounding_reserve,
        LaminarError::EquityWithoutAsolSupply
      );

      msg!(
        "Bootstrap orphan-equity dust sweep: {} lamports -> rounding reserve",
        old_claimable_equity
      );
    }

  }

  let sol_value = compute_tvl_sol(lst_amount, lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let sol_value_up = mul_div_up(lst_amount, lst_to_sol_rate, SOL_PRECISION)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("LST deposited: {}", lst_amount);
  msg!("SOL value: {}", sol_value);

  let current_nav = if current_asol_supply == 0 {
    // First mint bootstrap price
    SOL_PRECISION  // 1 aSOL = 1 SOL
  } else {
    nav_asol_with_reserve(old_tvl, current_liability, effective_rounding_reserve, current_asol_supply)
      .ok_or(LaminarError::MathOverflow)?
  };

  // Calculate aSOL to mint
  let asol_gross = if current_asol_supply == 0 {
    sol_value
  } else {
    if current_nav == 0 {
      return Err(LaminarError::InsolventProtocol.into());
    }
    mul_div_down(sol_value, SOL_PRECISION, current_nav)
      .ok_or(LaminarError::MathOverflow)?
  };

  let asol_reference_up = if current_asol_supply == 0 {
    sol_value_up
  } else {
    mul_div_up(sol_value_up, SOL_PRECISION, current_nav)
      .ok_or(LaminarError::MathOverflow)?
  };

  let mint_rounding_delta_asol = compute_rounding_delta_units(asol_gross, asol_reference_up)
    .ok_or(LaminarError::MathOverflow)?;

  let reserve_credit_from_mint = if current_asol_supply == 0 {
    mint_rounding_delta_asol
  } else {
    asol_dust_to_lamports_up(mint_rounding_delta_asol, current_nav)
      .ok_or(LaminarError::MathOverflow)?
  };
  msg!("aSOL gross (before fee): {}", asol_gross);

  // Apply fee
  let fee_bps = compute_dynamic_fee_bps(fee_asol_mint_bps, FeeAction::AsolMint, old_cr_bps, min_cr_bps, target_cr_bps, fee_min_multiplier_bps, fee_max_multiplier_bps, uncertainty_index_bps, uncertainty_max_bps).ok_or(LaminarError::InvalidParameter)?;

  let (asol_net, fee) = apply_fee(asol_gross, fee_bps)
    .ok_or(LaminarError::MathOverflow)?;

  msg!("Fee: {} aSOL", fee);
  msg!("aSOL net (to user): {}", asol_net);

  require!(asol_net >= min_asol_out, LaminarError::SlippageExceeded);
  require!(asol_net >= MIN_ASOL_MINT, LaminarError::AmountTooSmall);

  // Calculate new state values
  let new_lst_amount = current_lst_amount
    .checked_add(lst_amount)
    .ok_or(LaminarError::MathOverflow)?;

  let new_tvl = compute_tvl_sol(new_lst_amount, lst_to_sol_rate)
    .ok_or(LaminarError::MathOverflow)?;

  let new_asol_supply = current_asol_supply
    .checked_add(asol_gross)
    .ok_or(LaminarError::MathOverflow)?;

  let new_liability = current_liability;  // aSOL mint doesn't change liability
  
  let new_rounding_reserve = credit_rounding_reserve(effective_rounding_reserve, reserve_credit_from_mint, max_rounding_reserve)?;

  // Signed accounting equity for invariant checking
  let new_accounting_equity = compute_accounting_equity_sol(new_tvl, new_liability, new_rounding_reserve).ok_or(LaminarError::MathOverflow)?;

  // Claimable equity for user-facing events
  let new_claimable_equity = compute_claimable_equity_sol(new_tvl, new_liability, new_rounding_reserve).ok_or(LaminarError::MathOverflow)?;

  let leverage_multiple = if new_claimable_equity > 0 {
    mul_div_down(new_tvl, 100, new_claimable_equity).unwrap_or(0)
  } else {
    0
  };

  // Invariant checks
  assert_rounding_reserve_within_cap(new_rounding_reserve, max_rounding_reserve)?;
  assert_balance_sheet_holds(
    new_tvl,
    new_liability,
    new_accounting_equity,
    new_rounding_reserve,
    rounding_bound_lamports,
  )?;
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

  // Transfer LST from user to vault
  let transfer_accounts = TransferChecked {
    from: ctx.accounts.user_lst_account.to_account_info(),
    mint: ctx.accounts.lst_mint.to_account_info(),
    to: ctx.accounts.vault.to_account_info(),
    authority: ctx.accounts.user.to_account_info(),
  };

  let cpi_ctx = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
    transfer_accounts,
  );

  token_interface::transfer_checked(cpi_ctx, lst_amount, ctx.accounts.lst_mint.decimals)?;
  msg!("Transferred {} LST to vault", lst_amount);

  // Mint aSOL to user
  let seeds = &[GLOBAL_STATE_SEED, &[ctx.bumps.global_state]];
  let signer = &[&seeds[..]];

  let mint_to_user = MintTo {
    mint: ctx.accounts.asol_mint.to_account_info(),
    to: ctx.accounts.user_asol_account.to_account_info(),
    authority: ctx.accounts.global_state.to_account_info(),
  };

  let cpi_ctx_user = CpiContext::new_with_signer(
    ctx.accounts.token_program.to_account_info(),
    mint_to_user,
    signer,
  );

  token_interface::mint_to(cpi_ctx_user, asol_net)?;
  msg!("Minted {} aSOL to user", asol_net);

  // Mint fee to treasury
  if fee > 0 {
    let mint_to_treasury = MintTo {
      mint: ctx.accounts.asol_mint.to_account_info(),
      to: ctx.accounts.treasury_asol_account.to_account_info(),
      authority: ctx.accounts.global_state.to_account_info(),
    };

    let cpi_ctx_treasury = CpiContext::new_with_signer(
      ctx.accounts.token_program.to_account_info(),
      mint_to_treasury,
      signer,
    );

    token_interface::mint_to(cpi_ctx_treasury, fee)?;
    msg!("Minted {} aSOL fee to treasury", fee);
  }

  ctx.accounts.vault.reload()?;
  ctx.accounts.asol_mint.reload()?;

  let expected_vault_balance = ctx.accounts.global_state.total_lst_amount;
  require!(
    ctx.accounts.vault.amount == expected_vault_balance,
    LaminarError::BalanceSheetViolation
  );

  require!(
    ctx.accounts.asol_mint.supply == ctx.accounts.global_state.asol_supply,
    LaminarError::BalanceSheetViolation
  );

  msg!("Mint complete!");
  msg!("New TVL: {} lamports", new_tvl);
  msg!("New aSOL supply: {} (user {} + treasury {})", new_asol_supply, asol_net, fee);
  

  emit!(AsolMinted {
    user: ctx.accounts.user.key(),
    lst_deposited: lst_amount,
    asol_minted: asol_net,
    fee,
    nav: current_nav,
    old_tvl,
    new_tvl,
    old_equity: old_claimable_equity,
    new_equity: new_claimable_equity,
    leverage_multiple,
    timestamp: ctx.accounts.clock.unix_timestamp,
  });


  Ok(())
}

#[derive(Accounts)]
pub struct MintAsol<'info> {
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

  /// User's aSOL token account (receives minted aSOL)
  #[account(
    mut,
    token::mint = asol_mint,
    token::authority = user,
    constraint = user_asol_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Treasury's aSOL token account (receives protocol fees)
  #[account(
    init_if_needed,
    payer = user,
    associated_token::mint = asol_mint,
    associated_token::authority = treasury,
    associated_token::token_program = token_program,
    // constraint = treasury_asol_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub treasury_asol_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// CHECK: Verified by has_one constraint on global_state
  pub treasury: UncheckedAccount<'info>,

  /// User's LST token account (source of collateral)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = user,
    constraint = user_lst_account.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub user_lst_account: Box<InterfaceAccount<'info, TokenAccount>>,

  /// Protocol vault (receives LST)
  #[account(
    mut,
    token::mint = lst_mint,
    token::authority = vault_authority,
    constraint = vault.close_authority == anchor_lang::solana_program::program_option::COption::None @ LaminarError::InvalidAccountState,
  )]
  pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

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
