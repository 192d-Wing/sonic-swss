//! QosOrch - Quality of Service orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe QoS map types (DscpToTc, TcToQueue, etc.)
//! - Type-safe scheduler types (Strict, DWRR, WRR)
//! - Validated mapping ranges for TC, Queue, DSCP
//! - Option types for optional WRED thresholds
//! - HashMap for O(1) map lookups

mod ffi;
mod orch;
mod types;

pub use ffi::{register_qos_orch, unregister_qos_orch};
pub use orch::{QosOrch, QosOrchCallbacks, QosOrchConfig, QosOrchError, QosOrchStats};
pub use types::{
    MeterType, QosMapEntry, QosMapType, QosStats, SchedulerConfig, SchedulerEntry, SchedulerType,
    TcToQueueMapEntry, WredProfile,
};
