//! FdbOrch - Forwarding database (MAC learning) orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Safe MAC address parsing with Result types
//! - Type-safe FDB entry types (Dynamic/Static)
//! - Atomic counters for flush statistics
//! - HashMap for O(1) lookups without iterator invalidation
//! - Generic callbacks for SAI integration
//! - Full CRUD operations with statistics tracking

mod ffi;
mod orch;
mod types;

pub use ffi::{register_fdb_orch, unregister_fdb_orch};
pub use orch::{FdbOrch, FdbOrchCallbacks, FdbOrchConfig, FdbOrchError, FdbOrchStats, Result};
pub use types::{
    FdbEntry, FdbEntryType, FdbFlushStats, FdbKey, FdbOrigin, MacAddress, RawSaiObjectId,
    VlanMemberEntry, VlanTaggingMode,
};
