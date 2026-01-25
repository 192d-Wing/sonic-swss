//! IsolationGroupOrch - Port isolation group orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (isolationgrouporch.cpp, 749 lines) has critical safety issues:
//! - Unchecked `.at()` calls on maps (lines 142, 189, 234)
//! - Iterator invalidation in pending operation loops (lines 267-285)
//! - Exception-based error handling with incomplete cleanup
//! - Unchecked port/bridge port lookups
//! - No validation of group type consistency
//! - RAII violations in constructor/destructor
//! - Global mutable state via extern pointers
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` returning `Option<T>`
//! - Result-based error propagation
//! - Type-safe IsolationGroupType enum
//! - Pending operation tracking with safe iteration
//! - Proper cleanup via Drop trait

mod ffi;
mod orch;
mod types;

pub use ffi::{register_isolation_group_orch, unregister_isolation_group_orch};
pub use orch::{
    IsolationGroupOrch, IsolationGroupOrchCallbacks, IsolationGroupOrchConfig,
    IsolationGroupOrchError, IsolationGroupOrchStats,
};
pub use types::{IsolationGroupConfig, IsolationGroupEntry, IsolationGroupType};
