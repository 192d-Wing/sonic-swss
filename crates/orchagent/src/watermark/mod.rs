//! WatermarkOrch - Buffer watermark monitoring for SONiC.
//!
//! This module manages watermark statistics for monitoring buffer utilization:
//! - Queue watermarks (unicast, multicast, all)
//! - Priority Group (PG) watermarks (headroom, shared)
//! - Buffer pool watermarks
//!
//! # Architecture
//!
//! ```text
//! CONFIG_DB:WATERMARK
//!      │
//!      ▼
//! WatermarkOrch ───> COUNTERS_DB (periodic/persistent/user watermarks)
//!      │
//!      ├──> Telemetry timer (periodic clearing)
//!      └──> Clear notification handler
//! ```
//!
//! # Features
//!
//! - Configurable telemetry interval for periodic watermark clearing
//! - Support for different watermark tables (periodic, persistent, user)
//! - Clear requests for specific watermark types
//! - Integration with FlexCounter for enabling/disabling collection

mod ffi;
mod orch;
mod types;

pub use ffi::{register_watermark_orch, unregister_watermark_orch};
pub use orch::{WatermarkOrch, WatermarkOrchCallbacks, WatermarkOrchConfig, WatermarkOrchError};
pub use types::{ClearRequest, QueueType, WatermarkGroup, WatermarkStatus, WatermarkTable};
