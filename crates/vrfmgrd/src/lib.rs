//! VRF configuration manager daemon for SONiC
//!
//! Manages Virtual Routing and Forwarding (VRF) instances, enabling multi-tenancy
//! and EVPN (Ethernet VPN) functionality.

mod commands;
mod tables;
mod types;
mod vrf_mgr;

pub use commands::*;
pub use tables::*;
pub use types::*;
pub use vrf_mgr::VrfMgr;
