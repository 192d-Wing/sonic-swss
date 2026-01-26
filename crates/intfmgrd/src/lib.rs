//! Interface Manager Daemon - Network interface configuration manager
//!
//! intfmgrd manages network interface configuration including:
//! - IP addresses (IPv4/IPv6)
//! - VRF binding
//! - Sub-interfaces (VLAN-tagged)
//! - MPLS, proxy ARP, gratuitous ARP
//! - Admin status, MTU, MAC address
//! - Warm restart support

pub mod intf_mgr;
pub mod ip_operations;
pub mod subintf;
pub mod subintf_operations;
pub mod tables;
pub mod types;
pub mod vrf_operations;

pub use intf_mgr::IntfMgr;
pub use types::{IntfType, SubIntfInfo, SwitchType};
