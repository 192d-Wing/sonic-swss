//! FabricPortsOrch - Fabric port monitoring orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (fabricportsorch.cpp, 1,720 lines) has critical safety issues:
//! - Unchecked map accesses (lines 990, 996)
//! - Raw pointer management for timers (lines 44-45, 107-108)
//! - Integer overflow in counter increments (lines 801, 847)
//! - No bounds checking on counter increments
//!
//! The Rust implementation uses:
//! - Type-safe state machine transitions
//! - Saturating arithmetic for counters
//! - RwLock for concurrent access protection
//! - RAII for timer management

mod types;

pub use types::{FabricPortState, IsolationState, LinkStatus, PortHealthState};
