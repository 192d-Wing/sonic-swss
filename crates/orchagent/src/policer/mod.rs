//! PolicerOrch - Traffic policing and storm control orchestration for SONiC.
//!
//! This module manages policer configuration for rate limiting and QoS enforcement.
//! Policers implement traffic shaping using algorithms like SR-TCM (Single Rate Three
//! Color Marker) and TR-TCM (Two Rate Three Color Marker).
//!
//! # Architecture
//!
//! ```text
//! APPL_DB:POLICER_TABLE
//!      │
//!      ▼
//!   PolicerOrch
//!      │
//!      ├──> SAI Policer API (create policers)
//!      └──> Storm Control Integration
//! ```
//!
//! # Policer Sharing
//!
//! PolicerOrch uses a reference counting model where multiple entities (ACLs, ports,
//! storm control) can reference the same policer object:
//! - When the first reference is created, the SAI policer is created
//! - Subsequent references increment the ref count
//! - When the last reference is removed, the SAI policer is destroyed
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation has several safety issues:
//! - Unchecked `.at()` calls (lines 156, 318, 348)
//! - Unhandled `stoul()` exceptions (line 197)
//! - Map auto-vivification bug with `m_syncdPolicers[name].ref_count++`
//! - Iterator invalidation in doTask loop
//! - Missing null checks for SAI API pointers
//!
//! The Rust implementation uses:
//! - Option/Result types for all fallible operations
//! - Safe string parsing with `.parse()` returning `Result`
//! - Explicit ref count methods (no auto-vivification)
//! - Type-safe enums for MeterType, PolicerMode, ColorSource, PacketAction
//! - Saturating arithmetic for ref counts (prevents underflow)
//! - Storm control integration with type-safe naming

mod ffi;
mod orch;
mod types;

pub use ffi::{register_policer_orch, unregister_policer_orch};
pub use orch::{PolicerOrch, PolicerOrchCallbacks, PolicerOrchConfig, PolicerOrchError, PolicerOrchStats};
pub use types::{ColorSource, MeterType, PacketAction, PolicerConfig, PolicerEntry, PolicerMode, StormType};
