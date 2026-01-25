//! NeighOrch - Neighbor (ARP/NDP) orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - NeighborKey composite key (interface + IP)
//! - std::net::IpAddr for type-safe IPv4/IPv6
//! - Validated MAC address parsing
//! - Type-safe neighbor types (Dynamic/Static)
//! - HashMap for O(1) neighbor lookups

mod ffi;
mod orch;
mod types;

pub use ffi::{register_neigh_orch, unregister_neigh_orch};
pub use orch::{NeighOrch, NeighOrchCallbacks, NeighOrchConfig, NeighOrchError, NeighOrchStats};
pub use types::{
    MacAddress, NeighborConfig, NeighborEntry, NeighborKey, NeighborStats, NeighborType,
};
