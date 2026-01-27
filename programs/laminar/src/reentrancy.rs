// //! Reentrancy guard using RAII (Resource Acquisition Is Initialization) pattern
// //! 
// //! This module provides a safe reentrancy protection mechanism using Rust's Drop trait.
// //! The lock is automatically released when the guard goes out of scope, even if:
// //! - An early return occurs
// //! - A panic happens
// //! - An error is returned
// //! This implementation uses the "Proxy Pattern"- all state access goes through the guard

// use anchor_lang::prelude::*;
// use crate::state::GlobalState;
// use crate::error::LaminarError;

// /// RAII-based reentrancy guard with proxy access to state
// /// 
// /// The lock is acquired when constructed and automatically realeased when dropped.
// /// All state access Must go through guard.state to ensure the lock is held
// pub struct ReentrancyGuard<'a> {
//   /// Public field for proxy access
//   pub state: &'a mut GlobalState,
// }

// impl <'a> ReentrancyGuard<'a> {
//   /// Acquire the reentrancy lock
//   /// 
//   /// # Arguments
//   ///  * `state` - Mutable reference to GlobalState
//   /// 
//   /// # Returns
//   /// * `Ok(ReentrancyGuard)` - Lock acquired successfully
//   /// * `Err(LaminarError::Reentrancy)` - Lock already held (reentrancy detected)
//   /// 
//   /// # Security
//   /// This function MUST be called at the start of every state changing instruction.
//   /// The returned guard MUST be kept alive for the entire function scope
//   pub fn new(state: &'a mut GlobalState) -> Result<Self> {
//     // check if already locked (reentrancy attack)
//     require!(!state.locked, LaminarError::Reentrancy);

//     state.locked = true;
//     msg!("Reentrancy lock acquired");

//     Ok(Self { state })
//   }
// }

// impl <'a> Drop for ReentrancyGuard<'a> {
//   /// Automatically release the lock when the guard goes out of scope
//   /// 
//   /// This is called by Rust's runtime in all exit paths:
//   /// - Normal function return
//   /// - Early return (return Ok(()) or return Err(...))
//   /// - Panic (though panics should never happen in production)
//   fn drop(&mut self) {
//     self.state.locked = false;
//     msg!("Reentrancy lock released")
//   }
// }

// #[cfg(test)]
// mod tests {
//   use super::*;

//   fn mock_state() -> GlobalState {
//     GlobalState {
//       version: 1,
//       operation_counter: 0,
//       authority: Pubkey::default(),
//       amusd_mint: Pubkey::default(),
//       asol_mint: Pubkey::default(),
//       treasury: Pubkey::default(),
//       supported_lst_mint: Pubkey::default(),
//       total_lst_amount: 0,
//       amusd_supply: 0,
//       asol_supply: 0,
//       min_cr_bps: 13_000,
//       target_cr_bps: 15_000,
//       mint_paused: false,
//       redeem_paused: false,
//       locked: false,
//       mock_sol_price_usd: 100_000_000,
//       mock_lst_to_sol_rate: 1_000_000_000,
//       _reserved: [0; 2],
//     }
//   }
    
//   #[test]
//   fn test_lock_acquired_and_released() {
//     let mut state = mock_state();
//     assert!(!state.locked);
    
//     {
//       let guard = ReentrancyGuard::new(&mut state).unwrap();
//       assert!(guard.state.locked);
//     } // Guard dropped here
    
//     assert!(!state.locked); // Lock released
//   }
    
//   #[test]
//   fn test_proxy_access() {
//     let mut state = mock_state();
    
//     {
//       let guard = ReentrancyGuard::new(&mut state).unwrap();
      
//       // Access via proxy
//       guard.state.total_lst_amount = 1000;
//       assert_eq!(guard.state.total_lst_amount, 1000);
//     }
    
//     // State persists after guard dropped
//     assert_eq!(state.total_lst_amount, 1000);
//     assert!(!state.locked);
//   }
    
//   #[test]
//   fn test_early_return_releases_lock() {
//     let mut state = mock_state();
    
//     fn test_fn(state: &mut GlobalState) -> Result<()> {
//       let guard = ReentrancyGuard::new(state)?;
      
//       // Modify via proxy
//       guard.state.total_lst_amount = 500;
      
//       // Early return
//       return Ok(());
//     }
    
//     test_fn(&mut state).unwrap();
//     assert!(!state.locked); // Lock released
//     assert_eq!(state.total_lst_amount, 500); // State persisted
//   }
// }