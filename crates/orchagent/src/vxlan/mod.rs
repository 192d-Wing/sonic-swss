//! VxlanOrch - VXLAN tunnel orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - VxlanTunnelKey composite key (src_ip + dst_ip)
//! - Type-safe VNI (u32)
//! - std::net::IpAddr for IPv4/IPv6 tunnel endpoints
//! - Composite keys for VRF and VLAN mappings
//! - Type-safe encapsulation types (L2/L3)

mod ffi;
mod orch;
mod types;

pub use ffi::{register_vxlan_orch, unregister_vxlan_orch};
pub use orch::{VxlanOrch, VxlanOrchCallbacks, VxlanOrchConfig, VxlanOrchError, VxlanOrchStats};
pub use types::{
    Vni, VxlanEncapType, VxlanStats, VxlanTunnelConfig, VxlanTunnelEntry,
    VxlanTunnelKey, VxlanVlanMapEntry, VxlanVlanMapKey, VxlanVrfMapEntry, VxlanVrfMapKey,
};
