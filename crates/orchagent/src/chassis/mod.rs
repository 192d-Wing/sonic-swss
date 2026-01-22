//! ChassisOrch - Chassis management for modular SONiC systems.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe system port and fabric port keys
//! - Validated switch/core IDs
//! - HashMap for O(1) port lookups

mod ffi;
mod orch;
mod types;

pub use ffi::{register_chassis_orch, unregister_chassis_orch};
pub use orch::{ChassisOrch, ChassisOrchCallbacks, ChassisOrchConfig, ChassisOrchError, ChassisOrchStats};
pub use types::{ChassisStats, FabricPortEntry, FabricPortKey, SystemPortConfig, SystemPortEntry, SystemPortKey};
