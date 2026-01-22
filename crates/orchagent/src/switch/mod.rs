//! SwitchOrch - Switch-level orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe hash algorithm and field enums
//! - Validated capability structures with default implementations
//! - Option<SwitchState> preventing use-before-init bugs
//! - Structured configuration with sane defaults

mod ffi;
mod orch;
mod types;

pub use ffi::{register_switch_orch, unregister_switch_orch};
pub use orch::{SwitchOrch, SwitchOrchCallbacks, SwitchOrchConfig, SwitchOrchError, SwitchOrchStats};
pub use types::{SwitchCapabilities, SwitchConfig, SwitchHashAlgorithm, SwitchHashField, SwitchState};
