//! PfcwdOrch - Priority Flow Control Watchdog orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (pfcwdorch.cpp, 1,118 lines + pfcactionhandler.cpp, 856 lines) has critical issues:
//! - Destructor throws exceptions (SWSS_LOG_THROW in ACL handler)
//! - Race conditions on shared m_entryMap from multiple doTask contexts
//! - Unchecked array access (port.m_queue_ids[i] without bounds check)
//! - Memory leaks from raw new without RAII
//! - Platform-specific Broadcom DLR state without synchronization
//!
//! The Rust implementation uses:
//! - Result-based error handling (no throws in Drop)
//! - Arc<Mutex<T>> for thread-safe shared state
//! - Bounds-checked vector access
//! - RAII for all resources
//! - Type-safe action handlers via traits

mod ffi;
mod orch;
mod types;

pub use ffi::{register_pfcwd_orch, unregister_pfcwd_orch};
pub use orch::{PfcWdOrch, PfcWdOrchCallbacks, PfcWdOrchConfig, PfcWdOrchError, PfcWdOrchStats};
pub use types::{DetectionTime, PfcWdAction, PfcWdConfig, PfcWdEntry, PfcWdHwStats, PfcWdQueueEntry, PfcWdStats, RestorationTime};
