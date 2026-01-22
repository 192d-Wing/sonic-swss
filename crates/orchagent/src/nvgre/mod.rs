//! NvgreOrch - NVGRE tunnel orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (nvgreorch.cpp, 582 lines) has critical safety issues:
//! - 10+ unchecked `.at()` calls that throw std::out_of_range
//! - Exception-based error handling with incomplete cleanup
//! - RAII violations in constructor/destructor
//! - No rollback on partial failures
//! - TOCTOU issues with tunnel existence checks
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` returning `Option<T>`
//! - Result-based error propagation
//! - Proper cleanup via Drop trait
//! - VSID validation (prevents 0 and > max values)
//! - Type-safe MapType enum vs magic constants

mod ffi;
mod orch;
mod types;

pub use ffi::{register_nvgre_orch, unregister_nvgre_orch};
pub use orch::{NvgreOrch, NvgreOrchCallbacks, NvgreOrchConfig, NvgreOrchError, NvgreOrchStats, NvgreTunnel};
pub use types::{MapType, NvgreTunnelConfig, NvgreTunnelMapConfig, NvgreTunnelMapEntry, TunnelSaiIds, NVGRE_VSID_MAX_VALUE};
