//! VnetOrch - Virtual Network orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Composite keys for VNET and route lookups
//! - Type-safe route types (Direct/Tunnel/Vnet)
//! - std::net::IpAddr for endpoint addresses
//! - Vec for peer lists instead of raw arrays
//! - Option types for optional configuration

mod ffi;
mod orch;
mod types;

pub use ffi::{register_vnet_orch, unregister_vnet_orch};
pub use orch::{VnetOrch, VnetOrchCallbacks, VnetOrchConfig, VnetOrchError, VnetOrchStats};
pub use types::{
    Vni, VnetBridgePortEntry, VnetBridgePortKey, VnetConfig, VnetEntry,
    VnetKey, VnetRouteConfig, VnetRouteEntry, VnetRouteKey, VnetRouteType, VnetStats,
};
