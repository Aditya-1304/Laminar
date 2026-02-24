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

  #[msg("Rounding reserve exceeded configured cap")]
  RoundingReserveExceeded,

  #[msg("Rounding reserve underflow while paying user-favoring rounding delta")]
  RoundingReserveUnderflow,

  #[msg("Oracle snapshot is stale - refresh oracle before pricing actions")]
  OraclePriceStale,

  #[msg("Oracle confidence is above configured max_conf_bps")]
  OracleConfidenceTooHigh,

  #[msg("LST exchange-rate snapshot is stale - refresh exchange rate before pricing actions")]
  LstRateStale,

  #[msg("Unsupported oracle backend")]
  UnsupportedOracleBackend,

  #[msg("Unsupported LST rate backend")]
  UnsupportedLstRateBackend,

  #[msg("Pyth SOL/USD price account is not configured")]
  OracleFeedNotSet,

  #[msg("Configured oracle feed account was not provided in remaining accounts")]
  OracleFeedAccountMissing,

  #[msg("Provided oracle feed account does not match configured feed")]
  OracleFeedMismatch,

  #[msg("Unable to decode Pyth price feed account")]
  OracleFeedLoadFailed,

  #[msg("Oracle price is invalid (non-positive or malformed)")]
  OraclePriceInvalid,

  #[msg("Oracle exponent is out of supported range")]
  OracleExponentOutOfRange,

  #[msg("Safe oracle price is invalid (EMA <= confidence)")]
  SafePriceInvalid,

  #[msg("LST stake-pool account is not configured")]
  LstStakePoolNotSet,

  #[msg("Configured LST stake-pool account was not provided in remaining accounts")]
  LstStakePoolAccountMissing,

  #[msg("Provided stake-pool account does not match configured source")]
  LstStakePoolMismatch,

  #[msg("Unable to decode stake-pool account state")]
  LstStateLoadFailed,

  #[msg("Derived LST->SOL rate is invalid")]
  LstRateInvalid,
} 