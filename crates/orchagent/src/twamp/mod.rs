//! TwampOrch - Two-Way Active Measurement Protocol orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (twamporch.cpp, 1,053 lines) has critical safety issues:
//! - Unchecked pointer dereferences after find() operations
//! - No validation of SAI deserialization success
//! - Array index without bounds checking in notification handler
//! - Exception-based control flow with unclear error types
//! - Manual memory management with potential leaks
//! - Order-dependent configuration parsing (tx_mode conflicts)
//! - VRF reference counting without try-finally
//! - Database race conditions with state divergence
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` returning `Option<T>`
//! - Result-based error propagation
//! - Type-safe enums for modes, roles, and states
//! - Validated newtypes for DSCP, timeout, UDP ports
//! - RAII for resource cleanup
//! - Atomic state transitions

mod types;

pub use types::{
    Dscp, SessionTimeout, TimestampFormat, TwampMode, TwampRole, TwampSessionConfig,
    TwampSessionEntry, TwampSessionStatus, TwampStats, TwampUdpPort, TxMode,
};
