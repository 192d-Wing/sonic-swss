//! MuxOrch - MUX cable orchestration for SONiC dual ToR.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe MUX states (Active/Standby/Unknown)
//! - Type-safe cable types (ActiveActive/ActiveStandby)
//! - Safe state transition tracking
//! - Option types for optional IPv4/IPv6 addresses

mod ffi;
mod orch;
pub mod types;

pub use ffi::{register_mux_orch, unregister_mux_orch};
pub use orch::{MuxOrch, MuxOrchCallbacks, MuxOrchConfig, MuxOrchError, MuxOrchStats};
pub use types::{
    MuxCableType, MuxNeighborConfig, MuxNeighborEntry, MuxPortConfig, MuxPortEntry, MuxState,
    MuxStateChange, MuxStats,
};
