//! SflowOrch - sFlow packet sampling orchestration for SONiC.
//!
//! This module manages sFlow configuration for packet sampling on network ports.
//! sFlow is a standard for monitoring high-speed switched networks.
//!
//! # Architecture
//!
//! ```text
//! APPL_DB:SFLOW_TABLE (global)
//!      │
//!      ▼
//!   SflowOrch
//!      │
//!      ├──> SAI Samplepacket API (create sessions)
//!      └──> SAI Port API (enable/disable sampling)
//!
//! APPL_DB:SFLOW_SESSION_TABLE (per-port)
//!      │
//!      └──> SflowOrch (port configuration)
//! ```
//!
//! # Session Sharing
//!
//! SflowOrch uses a session-sharing model where multiple ports sampling at the
//! same rate share a single SAI samplepacket session object. This is managed
//! via reference counting:
//! - When the first port at rate R is configured, a session is created
//! - Subsequent ports at rate R reuse the existing session
//! - When the last port using rate R is removed, the session is destroyed
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation has several safety issues:
//! - Unchecked iterator dereference outside bounds check (line 402)
//! - Ignored return value from `getPort()` (line 382)
//! - Unhandled `stoul()` exception (line 277)
//! - Iterator invalidation risks with operator[] (lines 93-94)
//! - Missing null checks for SAI API pointers
//! - Linear search for session lookups (O(n))
//!
//! The Rust implementation uses:
//! - Option/Result types for all fallible operations
//! - Safe string parsing with `.parse()` returning `Result`
//! - Checked map access with `.get()` instead of `[]`
//! - Reverse index (session_id → rate) for O(1) lookups
//! - Type-safe enums for SampleDirection (vs strings)
//! - NonZeroU32 for sample rates (enforces valid rates)
//! - Saturating arithmetic for ref counts (prevents underflow)

mod ffi;
mod orch;
mod types;

pub use ffi::{register_sflow_orch, unregister_sflow_orch};
pub use orch::{SflowOrch, SflowOrchCallbacks, SflowOrchConfig, SflowOrchError, SflowOrchStats};
pub use types::{PortSflowInfo, SampleDirection, SflowConfig, SflowSession};
