//! Core protocol instructions 
//! Each instruction enforces invariants and updates the balance

pub mod initialize;
pub mod mint_amusd;
pub mod redeem_amusd;
pub mod mint_asol;
pub mod redeem_asol;

#[allow(ambiguous_glob_reexports)]
pub use initialize::*;
#[allow(ambiguous_glob_reexports)]
pub use mint_amusd::*;
#[allow(ambiguous_glob_reexports)]
pub use redeem_amusd::*;
#[allow(ambiguous_glob_reexports)]
pub use mint_asol::*;
#[allow(ambiguous_glob_reexports)]
pub use redeem_asol::*;