//! CounterCheckOrch - Port counter validation orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Composite key (port_name + counter_type)
//! - Tolerance-based validation method
//! - Safe arithmetic for difference calculation

mod ffi;
mod orch;
mod types;

pub use ffi::{register_countercheck_orch, unregister_countercheck_orch};
pub use orch::{CounterCheckOrch, CounterCheckOrchCallbacks, CounterCheckOrchConfig, CounterCheckOrchError, CounterCheckOrchStats};
pub use types::{CounterCheckConfig, CounterCheckEntry, CounterCheckKey, CounterCheckStats};
