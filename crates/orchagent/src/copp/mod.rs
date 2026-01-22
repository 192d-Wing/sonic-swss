//! CoppOrch - Control Plane Policing orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe trap action enum
//! - Option types for optional policer parameters
//! - HashMap for O(1) trap lookups

mod ffi;
mod orch;
mod types;

pub use ffi::{register_copp_orch, unregister_copp_orch};
pub use orch::{CoppOrch, CoppOrchCallbacks, CoppOrchConfig, CoppOrchError, CoppOrchStats};
pub use types::{CoppStats, CoppTrapAction, CoppTrapConfig, CoppTrapEntry, CoppTrapKey};
