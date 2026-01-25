//! BufferOrch - Buffer pool and profile management for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Saturating reference counting preventing underflow
//! - Type-safe buffer pool types and modes
//! - Validated threshold configurations
//! - Result types for ref count operations

mod ffi;
mod orch;
mod types;

pub use ffi::{register_buffer_orch, unregister_buffer_orch};
pub use orch::{
    BufferOrch, BufferOrchCallbacks, BufferOrchConfig, BufferOrchError, BufferOrchStats,
};
pub use types::{
    BufferPoolConfig, BufferPoolEntry, BufferPoolMode, BufferPoolType, BufferProfileConfig,
    BufferProfileEntry, BufferQueueConfig, BufferStats, IngressPriorityGroupEntry,
    PriorityGroupConfig, ThresholdMode,
};
