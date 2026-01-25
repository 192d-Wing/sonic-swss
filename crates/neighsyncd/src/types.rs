//! Core types for neighbor synchronization
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - CM-8: System Component Inventory - Neighbor entries as network components
//! - SI-4: System Monitoring - Neighbor state tracking for security monitoring
//! - SC-7: Boundary Protection - Network boundary neighbor awareness

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv6Addr};

/// MAC address representation
///
/// # NIST Controls
/// - IA-3: Device Identification - MAC addresses for device identification
/// - AU-3: Content of Audit Records - MAC included in neighbor audit records
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    /// Zero MAC address (used for unresolved neighbors on dual-ToR)
    /// NIST: SC-7 - Boundary protection for unresolved neighbors
    pub const ZERO: Self = Self([0, 0, 0, 0, 0, 0]);

    /// Broadcast MAC address (filtered out)
    /// NIST: SC-5 - Denial of service protection (filter broadcast)
    pub const BROADCAST: Self = Self([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

    /// Check if this is a zero MAC
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0 == Self::ZERO.0
    }

    /// Check if this is a broadcast MAC
    #[inline]
    pub fn is_broadcast(&self) -> bool {
        self.0 == Self::BROADCAST.0
    }

    /// Parse MAC from colon-separated string (e.g., "00:11:22:33:44:55")
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return None;
        }
        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16).ok()?;
        }
        Some(Self(bytes))
    }
}

impl std::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

/// Kernel neighbor state (NUD_* values from linux/neighbour.h)
///
/// # NIST Controls
/// - SI-4: System Monitoring - Track neighbor reachability states
/// - IR-4: Incident Handling - State changes may indicate security incidents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum NeighborState {
    /// Neighbor is incomplete (resolution in progress)
    Incomplete = 0x01,
    /// Neighbor is reachable
    Reachable = 0x02,
    /// Neighbor reachability is stale
    Stale = 0x04,
    /// Neighbor resolution delayed
    Delay = 0x08,
    /// Neighbor probe in progress
    Probe = 0x10,
    /// Neighbor resolution failed
    Failed = 0x20,
    /// No ARP needed (static or local)
    NoArp = 0x40,
    /// Permanent entry
    Permanent = 0x80,
    /// Unknown state
    Unknown = 0x00,
}

impl NeighborState {
    /// Create from kernel NUD_* value
    pub fn from_kernel(state: u16) -> Self {
        match state {
            0x01 => Self::Incomplete,
            0x02 => Self::Reachable,
            0x04 => Self::Stale,
            0x08 => Self::Delay,
            0x10 => Self::Probe,
            0x20 => Self::Failed,
            0x40 => Self::NoArp,
            0x80 => Self::Permanent,
            _ => Self::Unknown,
        }
    }

    /// Check if this state indicates the neighbor is resolvable
    #[inline]
    pub fn is_resolved(&self) -> bool {
        matches!(
            self,
            Self::Reachable | Self::Stale | Self::Delay | Self::Probe | Self::Permanent
        )
    }
}

/// Neighbor table entry
///
/// # NIST Controls
/// - CM-8: System Component Inventory - Track network neighbors
/// - AU-3: Content of Audit Records - Full neighbor information for logging
/// - IA-3: Device Identification - Neighbor identification via IP/MAC
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeighborEntry {
    /// Interface index
    pub ifindex: u32,
    /// Interface name (resolved from ifindex)
    pub interface: String,
    /// Neighbor IP address (IPv6 only by default, IPv4 with feature)
    pub ip: IpAddr,
    /// Neighbor MAC address
    pub mac: MacAddress,
    /// Kernel neighbor state
    pub state: NeighborState,
    /// Whether this is an externally learned neighbor (e.g., VXLAN EVPN)
    /// NIST: SC-7 - Track externally learned entries for boundary protection
    pub externally_learned: bool,
}

impl NeighborEntry {
    /// Create Redis key for NEIGH_TABLE
    /// Format: "NEIGH_TABLE:{interface}:{ip}"
    pub fn redis_key(&self) -> String {
        format!("{}:{}", self.interface, self.ip)
    }

    /// Get address family string for Redis
    pub fn family_str(&self) -> &'static str {
        match self.ip {
            IpAddr::V6(_) => "IPv6",
            #[cfg(feature = "ipv4")]
            IpAddr::V4(_) => "IPv4",
            #[cfg(not(feature = "ipv4"))]
            IpAddr::V4(_) => unreachable!("IPv4 support disabled"),
        }
    }

    /// Check if this is an IPv6 link-local address
    /// NIST: SC-7 - Link-local filtering for boundary protection
    pub fn is_ipv6_link_local(&self) -> bool {
        match self.ip {
            IpAddr::V6(addr) => is_ipv6_link_local(&addr),
            IpAddr::V4(_) => false,
        }
    }

    /// Check if this is an IPv6 multicast link-local address
    /// NIST: SC-5 - Filter multicast to prevent DoS
    pub fn is_ipv6_multicast_link_local(&self) -> bool {
        match self.ip {
            IpAddr::V6(addr) => is_ipv6_multicast_link_local(&addr),
            IpAddr::V4(_) => false,
        }
    }

    /// Check if this is an IPv4 link-local address (169.254.x.x)
    #[cfg(feature = "ipv4")]
    pub fn is_ipv4_link_local(&self) -> bool {
        match self.ip {
            IpAddr::V4(addr) => addr.is_link_local(),
            IpAddr::V6(_) => false,
        }
    }
}

/// Check if IPv6 address is link-local (fe80::/10)
#[inline]
fn is_ipv6_link_local(addr: &Ipv6Addr) -> bool {
    let segments = addr.segments();
    (segments[0] & 0xffc0) == 0xfe80
}

/// Check if IPv6 address is multicast link-local (ff02::/16)
#[inline]
fn is_ipv6_multicast_link_local(addr: &Ipv6Addr) -> bool {
    let segments = addr.segments();
    segments[0] == 0xff02
}

/// Netlink message type for neighbor operations
///
/// # NIST Controls
/// - AU-12: Audit Record Generation - Log neighbor changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeighborMessageType {
    /// New neighbor or update (RTM_NEWNEIGH)
    New,
    /// Neighbor deleted (RTM_DELNEIGH)
    Delete,
    /// Neighbor dump response (RTM_GETNEIGH)
    Get,
}

/// Neighbor flags from kernel (NTF_* values)
#[derive(Debug, Clone, Copy, Default)]
pub struct NeighborFlags {
    /// NTF_EXT_LEARNED - Externally learned (e.g., VXLAN EVPN)
    pub ext_learned: bool,
    /// NTF_ROUTER - Neighbor is a router
    pub router: bool,
}

impl NeighborFlags {
    /// Parse from kernel NTF_* flags
    pub fn from_kernel(flags: u8) -> Self {
        Self {
            ext_learned: (flags & 0x10) != 0, // NTF_EXT_LEARNED
            router: (flags & 0x80) != 0,      // NTF_ROUTER
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_address_display() {
        let mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(mac.to_string(), "00:11:22:33:44:55");
    }

    #[test]
    fn test_mac_address_parse() {
        let mac = MacAddress::parse("00:11:22:33:44:55").unwrap();
        assert_eq!(mac.0, [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    }

    #[test]
    fn test_mac_address_special() {
        assert!(MacAddress::ZERO.is_zero());
        assert!(MacAddress::BROADCAST.is_broadcast());
        assert!(!MacAddress::ZERO.is_broadcast());
    }

    #[test]
    fn test_neighbor_state_resolved() {
        assert!(NeighborState::Reachable.is_resolved());
        assert!(NeighborState::Stale.is_resolved());
        assert!(!NeighborState::Incomplete.is_resolved());
        assert!(!NeighborState::Failed.is_resolved());
    }

    #[test]
    fn test_ipv6_link_local_detection() {
        let link_local: Ipv6Addr = "fe80::1".parse().unwrap();
        let global: Ipv6Addr = "2001:db8::1".parse().unwrap();
        assert!(is_ipv6_link_local(&link_local));
        assert!(!is_ipv6_link_local(&global));
    }

    #[test]
    fn test_ipv6_multicast_link_local_detection() {
        let mcast_ll: Ipv6Addr = "ff02::1".parse().unwrap();
        let mcast_global: Ipv6Addr = "ff0e::1".parse().unwrap();
        assert!(is_ipv6_multicast_link_local(&mcast_ll));
        assert!(!is_ipv6_multicast_link_local(&mcast_global));
    }
}
