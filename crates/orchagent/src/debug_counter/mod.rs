//! DebugCounterOrch - Debug counter orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (debugcounterorch.cpp, 831 lines) has critical safety issues:
//! - Unchecked dynamic_cast operations (lines 178, 223, 456)
//! - Complex flex counter integration with shared state
//! - Exception-based error handling with incomplete cleanup
//! - Unchecked map access with `.at()` (lines 289, 312, 401)
//! - Iterator invalidation in drop reason reconciliation (lines 534-567)
//! - No validation of counter type vs drop reason compatibility
//! - String-based type lookups without validation
//! - RAII violations in constructor/destructor
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` returning `Option<T>`
//! - Result-based error propagation
//! - Type-safe DebugCounterType enum with helper methods
//! - Free counter tracking with safe iteration
//! - Proper cleanup via Drop trait
//! - Validated drop reason reconciliation

mod ffi;
mod orch;
mod types;

pub use ffi::{register_debug_counter_orch, unregister_debug_counter_orch};
pub use orch::{DebugCounterOrch, DebugCounterOrchCallbacks, DebugCounterOrchConfig, DebugCounterOrchError, DebugCounterOrchStats};
pub use types::{DebugCounterConfig, DebugCounterEntry, DebugCounterType, DropReason, FreeCounter};
