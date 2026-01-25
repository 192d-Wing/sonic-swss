//! RouteOrch - Route orchestration for SONiC.
//!
//! This module manages IP route programming in SONiC, including:
//! - Route entry creation and deletion
//! - Next-hop group management with safe reference counting
//! - ECMP (Equal-Cost Multi-Path) routing
//! - VRF (Virtual Routing and Forwarding) support
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation fixes the auto-vivification bug present in C++:
//! ```cpp
//! m_syncdNextHopGroups[nexthops].ref_count++;  // Creates entry if missing!
//! ```
//!
//! In Rust, we use `SyncMap` which returns `Err(KeyNotFound)` instead of
//! silently creating entries.

mod ffi;
mod nexthop;
mod nhg;
mod orch;
mod types;

pub use ffi::{register_route_orch, unregister_route_orch};
pub use nexthop::{NextHopFlags, NextHopKey};
pub use nhg::{NextHopGroupEntry, NextHopGroupKey, NextHopGroupTable};
pub use orch::{RouteError, RouteOrch, RouteOrchCallbacks, RouteOrchConfig};
pub use types::{RouteEntry, RouteKey, RouteNhg, RouteTables};
