//! PbhOrch - Policy-Based Hashing orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe hash field enums
//! - Composite key (table_name, rule_name) for rule lookup
//! - Validated priority and configuration
//! - Structured packet action types

mod ffi;
mod orch;
mod types;

pub use ffi::{register_pbh_orch, unregister_pbh_orch};
pub use orch::{PbhOrch, PbhOrchCallbacks, PbhOrchConfig, PbhOrchError, PbhOrchStats};
pub use types::{
    PbhHashConfig, PbhHashEntry, PbhHashField, PbhPacketAction, PbhRuleConfig, PbhRuleEntry,
    PbhStats, PbhTableConfig, PbhTableEntry,
};
