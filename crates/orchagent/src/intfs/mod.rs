//! IntfsOrch - Router interface orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Checked arithmetic for reference counting
//! - IP overlap validator with clear error messages
//! - Transactional VRF updates
//! - Type-safe RIF type enum

mod ffi;
mod orch;
mod types;

pub use ffi::{register_intfs_orch, unregister_intfs_orch};
pub use orch::{IntfsOrch, IntfsOrchCallbacks, IntfsOrchConfig, IntfsOrchError, IntfsOrchStats};
pub use types::{IntfsEntry, RifType};
