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

mod types;

pub use types::{DetectionTime, PfcWdAction, PfcWdConfig, PfcWdHwStats, PfcWdQueueEntry, RestorationTime};
