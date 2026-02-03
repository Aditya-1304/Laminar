use anchor_lang::prelude::*;

#[error_code]
#[derive(PartialEq,Eq)]
pub enum LaminarError {
  #[msg("Minting is currently paused by protocol administrator")]
  MintPaused,
  
  #[msg("Redemptions are currently paused by protocol administrator")]
  RedeemPaused,
  
  #[msg("Amount must be greater than zero")]
  ZeroAmount,
  
  #[msg("Math overflow occurred - values exceeded u64 bounds")]
  MathOverflow,
  
  #[msg("Insufficient collateral in vault to complete this operation")]
  InsufficientCollateral,
  
  #[msg("Insufficient token supply to burn - check your balance")]
  InsufficientSupply,

  #[msg("LST mint not whitelisted - only the supported LST type is accepted as collateral")]
  UnsupportedLST,

  #[msg("Protocol is insolvent - aSOL NAV is zero, equity redemptions are frozen")]
  InsolventProtocol,

  #[msg("Slippage tolerance exceeded - actual output is below your minimum")]
  SlippageExceeded,  
  
  #[msg("Reentrancy attack detected - operation blocked")]
  Reentrancy,

  #[msg("Invalid mint authority - mint must be controlled by global_state PDA")]
  InvalidMintAuthority,

  #[msg("Invalid account state - unexpected account configuration")]
  InvalidAccountState,

  #[msg("Amount too small - below minimum threshold of 0.0001 SOL equivalent")]
  AmountTooSmall,

  #[msg("Invalid account owner - account is not owned by this program")]
  InvalidAccountOwner,

  #[msg("LST mint must have 9 decimals to match SOL precision")]
  InvalidDecimals,

  #[msg("Invalid protocol version - state account needs migration")]
  InvalidVersion,

  #[msg("Invalid CPI context - instruction must be called directly, not via CPI")]
  InvalidCPIContext,

  #[msg("Invalid mint address - does not match expected protocol mint")]
  InvalidMint,

  #[msg("Invalid freeze authority configuration")]
  InvalidFreezeAuthority,

  #[msg("Operation would reduce TVL below minimum protocol threshold")]
  BelowMinimumTVL,

  #[msg("Balance sheet invariant violated: TVL must equal Liability plus Equity")]
  BalanceSheetViolation,
  
  #[msg("Collateral ratio would fall below minimum safety threshold")]
  CollateralRatioTooLow,
  
  #[msg("Negative equity detected - protocol is in distressed state")]
  NegativeEquity,
  
  #[msg("Cannot perform operation when supply is zero")]
  ZeroSupply,

  #[msg("Arithmetic overflow in safety check computation")]
  ArithmeticOverflow,

  #[msg("Invalid parameter value provided")]
  InvalidParameter,

  #[msg("aSOL supply is zero while equity exists; bootstrap required before minting")]
  EquityWithoutAsolSupply,
} 