use anchor_lang::prelude::*;

#[error_code]
pub enum LaminarError {
  #[msg("Minting is currently paused")]
  MintPaused,
  
  #[msg("Redemptions are currently paused")]
  RedeemPaused,
  
  #[msg("Amount must be greater than zero")]
  ZeroAmount,
  
  #[msg("Math overflow occurred")]
  MathOverflow,
  
  #[msg("Insufficient collateral in vault")]
  InsufficientCollateral,
  
  #[msg("Insufficient supply to burn")]
  InsufficientSupply,

  #[msg("LST mint not whitelisted - only supported_lst_mint is accepted")]
  UnsupportedLST,

  #[msg("Protocol is insolvent - aSOL NAV is zero")]
  InsolventProtocol,

  #[msg("Slippage tolerance exceeded")]
  SlippageExceeded,  
  
  #[msg("Reentrancy detected")]
  Reentrancy,

  #[msg("Invalid mint authority - must be global_state PDA")]
  InvalidMintAuthority,

  #[msg("Invalid account state")]
  InvalidAccountState,

  #[msg("Amount too small - below minimum deposit threshold")]
  AmountTooSmall,

  #[msg("Invalid account owner - account does not belong to this program")]
  InvalidAccountOwner,

  #[msg("LST mint must have 9 decimals (SOL standard)")]
  InvalidDecimals,

  #[msg("Invalid protocol version")]
  InvalidVersion,

  #[msg("Invalid CPI context - must be called directly")]
  InvalidCPIContext,

  #[msg("Invalid mint address")]
  InvalidMint,

  #[msg("Invalid freeze authority")]
  InvalidFreezeAuthority,

  #[msg("TVL would fall below minimum threshold")]
  BelowMinimumTVL,

  #[msg("Balance sheet invariant violated: TVL != Liability + Equity")]
  BalanceSheetViolation,
  
  #[msg("Collateral ratio below minimum threshold")]
  CollateralRatioTooLow,
  
  #[msg("Negative equity detected: TVL < Liability")]
  NegativeEquity,
  
  #[msg("Supply is zero - cannot perform this operation")]
  ZeroSupply,

  #[msg("Arithmetic overflow in invariant check")]
  ArithmeticOverflow,

  #[msg("Invalid parameter value")]
  InvalidParameter,
}