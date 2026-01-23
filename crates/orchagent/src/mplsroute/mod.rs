//! MplsRouteOrch - MPLS route orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type alias for MPLS labels (u32)
//! - Validated label range (0-1048575)
//! - Type-safe MPLS actions (Pop/Swap/Push)
//! - Vec for push label stack
//! - Generic callbacks for SAI integration
//! - Full CRUD operations with error handling

mod ffi;
mod orch;
mod types;

pub use ffi::{register_mplsroute_orch, unregister_mplsroute_orch};
pub use orch::{MplsRouteOrch, MplsRouteOrchCallbacks, MplsRouteOrchConfig, MplsRouteOrchError, MplsRouteOrchStats, Result};
pub use types::{MplsAction, MplsLabel, MplsRouteConfig, MplsRouteEntry, MplsRouteKey, MplsRouteStats, RawSaiObjectId};
