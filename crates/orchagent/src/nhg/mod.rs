//! NhgOrch - Next Hop Group orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (nhgorch.cpp, 1,160 lines) has critical safety issues:
//! - Direct `.at()` on m_syncdNextHopGroups without prior bounds check (HIGH RISK)
//! - Null unique_ptr dereference without validation
//! - Exception handling catches all, logs, continues silently
//! - Reference count underflow (assert only in Debug)
//! - No concurrent access protection on shared maps
//! - Resource leak on exception during construction
//!
//! The Rust implementation uses:
//! - Safe map access with `.entry()` API or `.get()` returning Option
//! - Result-based error propagation instead of exceptions
//! - NonZeroUsize for reference counting with overflow checks
//! - Arc<Mutex<T>> for thread-safe shared state
//! - RAII for automatic resource cleanup
//! - Type-safe SAI object ID wrappers

mod ffi;
mod orch;
mod types;

pub use ffi::{register_nhg_orch, unregister_nhg_orch};
pub use orch::{NhgOrch, NhgOrchCallbacks, NhgOrchConfig, NhgOrchError, NhgOrchStats};
pub use types::{
    LabelStack, NextHopGroupEntry, NextHopGroupKey, NextHopGroupMember, NextHopKey, NhgEntry,
};
