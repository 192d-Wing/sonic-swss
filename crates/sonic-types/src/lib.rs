//! Common SONiC types for network switch orchestration.
//!
//! This crate provides type-safe representations of common network primitives
//! used throughout the SONiC control plane:
//!
//! - [`MacAddress`]: 48-bit Ethernet MAC addresses
//! - [`IpAddress`]: IPv4 and IPv6 addresses
//! - [`IpPrefix`]: IP network prefixes (CIDR notation)
//! - [`PortType`]: Switch port classifications
//! - [`VlanId`]: IEEE 802.1Q VLAN identifiers

mod ip;
mod mac;
mod port;
mod vlan;

pub use ip::{IpAddress, IpPrefix, Ipv4Address, Ipv6Address};
pub use mac::MacAddress;
pub use port::{AdminState, OperState, PortRole, PortType};
pub use vlan::VlanId;

/// Common error type for parsing failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("invalid MAC address format: {0}")]
    InvalidMacAddress(String),

    #[error("invalid IP address format: {0}")]
    InvalidIpAddress(String),

    #[error("invalid IP prefix format: {0}")]
    InvalidIpPrefix(String),

    #[error("invalid VLAN ID: {0} (must be 1-4094)")]
    InvalidVlanId(u16),

    #[error("invalid port type: {0}")]
    InvalidPortType(String),
}
