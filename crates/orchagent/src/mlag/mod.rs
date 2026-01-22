//! MlagOrch - Multi-Chassis Link Aggregation orchestration for SONiC.
//!
//! This module manages MLAG (Multi-Chassis Link Aggregation) configuration,
//! allowing two switches to act as a single logical unit to connected
//! downstream devices.
//!
//! # Architecture
//!
//! ```text
//! CONFIG_DB:MCLAG_DOMAIN
//!      │
//!      ▼
//!   MlagOrch ───> Observers (FdbOrch, etc.)
//!      │
//!      ├──> ISL (peer-link) tracking
//!      └──> MLAG interface membership
//!
//! CONFIG_DB:MCLAG_INTERFACE
//!      │
//!      └──> MlagOrch (interface add/remove)
//! ```
//!
//! # Observer Pattern
//!
//! MlagOrch notifies registered observers about two types of changes:
//! - `MlagSubjectType::IslChange` - Peer-link changed
//! - `MlagSubjectType::IntfChange` - MLAG interface membership changed
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation has:
//! - Silent error handling for duplicate adds and unknown deletes
//! - String parsing without explicit bounds checking
//!
//! The Rust implementation uses:
//! - `Result` types with explicit error variants
//! - Safe string parsing with `Option` returns
//! - `HashSet` for O(1) interface lookups

mod ffi;
mod orch;
mod types;

pub use ffi::{register_mlag_orch, unregister_mlag_orch};
pub use orch::{MlagOrch, MlagOrchCallbacks, MlagOrchConfig, MlagOrchError, MlagOrchStats};
pub use types::{MlagIfUpdate, MlagIslUpdate, MlagSubjectType, MlagUpdate};
