//! BfdOrch - Bidirectional Forwarding Detection orchestration for SONiC.
//!
//! This module manages BFD sessions for monitoring neighbor reachability,
//! handling both hardware-offloaded and software BFD implementations.
//!
//! # Architecture
//!
//! ```text
//! APPL_DB:BFD_FVS
//!      │
//!      ▼
//!   BfdOrch ───> SAI BFD API (hardware offload)
//!      │
//!      ├──> STATE_DB:BFD_SESSION (session states)
//!      ├──> SAI notifications (state changes)
//!      └──> Observers (state change notifications)
//! ```
//!
//! # BFD Session Types
//!
//! - Async Active/Passive: Asynchronous mode BFD
//! - Demand Active/Passive: Demand mode BFD
//!
//! # TSA Integration
//!
//! Sessions with `shutdown_bfd_during_tsa=true` are automatically removed
//! when Traffic Shift Algorithm (TSA) is enabled, and restored when disabled.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation has several unchecked `.at()` calls:
//! - `session_state_lookup.at(state)` in notification handler (lines 247, 252, 255)
//! - `session_type_lookup.at(bfd_session_type)` in session creation (line 418)
//!
//! These can throw `std::out_of_range` if SAI returns unexpected values.
//!
//! The Rust implementation uses:
//! - `Option` returns for state/type conversions
//! - `Result` types for all fallible operations
//! - Type-safe enums with exhaustive pattern matching

mod ffi;
mod orch;
mod types;

pub use ffi::{register_bfd_orch, unregister_bfd_orch};
pub use orch::{BfdOrch, BfdOrchCallbacks, BfdOrchConfig, BfdOrchError, BfdOrchStats};
pub use types::{
    BfdSessionConfig, BfdSessionInfo, BfdSessionKey, BfdSessionState, BfdSessionType, BfdUpdate,
    BFD_SESSION_DEFAULT_DETECT_MULTIPLIER, BFD_SESSION_DEFAULT_RX_INTERVAL,
    BFD_SESSION_DEFAULT_TOS, BFD_SESSION_DEFAULT_TX_INTERVAL, BFD_SRCPORT_INIT, BFD_SRCPORT_MAX,
    NUM_BFD_SRCPORT_RETRIES,
};
