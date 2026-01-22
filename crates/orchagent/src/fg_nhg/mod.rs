//! FgNhgOrch - Fine-Grained Next Hop Group orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Vec for next hop collection instead of raw pointers
//! - Type-safe bank selection modes
//! - Validated bucket sizes and weights
//! - HashMap lookups with Option returns

mod ffi;
mod orch;
mod types;

pub use ffi::{register_fg_nhg_orch, unregister_fg_nhg_orch};
pub use orch::{FgNhgOrch, FgNhgOrchCallbacks, FgNhgOrchConfig, FgNhgOrchError, FgNhgOrchStats};
pub use types::{
    BankSelectionMode, FgNhgBankConfig, FgNhgEntry, FgNhgMemberConfig,
    FgNhgMemberEntry, FgNhgPrefix, FgNhgStats, FgNextHop,
};
