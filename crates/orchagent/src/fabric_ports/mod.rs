//! FabricPortsOrch - Fabric port monitoring orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe state machine transitions
//! - Saturating arithmetic for counters
//! - RwLock for concurrent access protection
//! - RAII for timer management

mod ffi;
mod orch;
mod types;

pub use ffi::{register_fabric_ports_orch, unregister_fabric_ports_orch};
pub use orch::{
    FabricPortsOrch, FabricPortsOrchCallbacks, FabricPortsOrchConfig, FabricPortsOrchError,
    FabricPortsOrchStats, Result,
};
pub use types::{FabricPortState, IsolationState, LinkStatus, PortHealthState};
