//! DtelOrch - Data Plane Telemetry orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (dtelorch.cpp, 1,728 lines) has critical safety issues:
//! - Unchecked refCount < 0 (lines 123-125)
//! - Raw pointer output parameters (lines 280-283)
//! - Manual OID list management (line 391)
//! - No synchronization for shared state
//!
//! The Rust implementation uses:
//! - Atomic reference counting preventing negative values
//! - Owned return values instead of output parameters
//! - Arc<RwLock<T>> for thread-safe shared state
//! - Type-safe event enums replacing string lookups

mod ffi;
mod orch;
mod types;

pub use ffi::{register_dtel_orch, unregister_dtel_orch};
pub use orch::{
    DtelEventEntry, DtelOrch, DtelOrchCallbacks, DtelOrchConfig, DtelOrchError, DtelOrchStats,
    Result, WatchlistEntry,
};
pub use types::{DtelEventType, IntSessionConfig, IntSessionEntry};
