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
}