//! Srv6Orch - SRv6 (Segment Routing over IPv6) orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe SRv6 SID wrapper with validation
//! - Type-safe endpoint behavior enum
//! - Type-safe encapsulation mode enum
//! - Vec for SID lists instead of raw arrays
//! - Validated IPv6 format for SIDs

mod ffi;
mod orch;
mod types;

pub use ffi::{register_srv6_orch, unregister_srv6_orch};
pub use orch::{Srv6Orch, Srv6OrchCallbacks, Srv6OrchConfig, Srv6OrchError, Srv6OrchStats};
pub use types::{
    Srv6EncapMode, Srv6EndpointBehavior, Srv6LocalSidConfig, Srv6LocalSidEntry,
    Srv6NextHopConfig, Srv6NextHopEntry, Srv6Sid, Srv6SidListConfig,
    Srv6SidListEntry, Srv6Stats,
};
