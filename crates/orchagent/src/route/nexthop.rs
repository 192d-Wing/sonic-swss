//! Next-hop key and related types.
//!
//! A next-hop represents a single forwarding destination, identified by
//! an IP address and optionally an interface alias.

use sonic_types::{IpAddress, Ipv4Address};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// Flags indicating next-hop state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NextHopFlags(u32);

impl NextHopFlags {
    /// No flags set.
    pub const NONE: Self = Self(0);
    /// Interface is down.
    pub const IF_DOWN: Self = Self(1 << 0);
    /// This is a label next-hop.
    pub const LABEL: Self = Self(1 << 1);
    /// This is a VxLAN tunnel next-hop.
    pub const VXLAN_TUNNEL: Self = Self(1 << 2);
    /// This is a mux tunnel next-hop.
    pub const MUX_TUNNEL: Self = Self(1 << 3);
    /// This is an SRv6 next-hop.
    pub const SRV6: Self = Self(1 << 4);

    /// Returns true if interface is down.
    pub fn is_if_down(&self) -> bool {
        self.0 & Self::IF_DOWN.0 != 0
    }

    /// Returns true if this is a label next-hop.
    pub fn is_label(&self) -> bool {
        self.0 & Self::LABEL.0 != 0
    }

    /// Returns true if this is a VxLAN tunnel next-hop.
    pub fn is_vxlan_tunnel(&self) -> bool {
        self.0 & Self::VXLAN_TUNNEL.0 != 0
    }

    /// Returns true if this is a mux tunnel next-hop.
    pub fn is_mux_tunnel(&self) -> bool {
        self.0 & Self::MUX_TUNNEL.0 != 0
    }

    /// Returns true if this is an SRv6 next-hop.
    pub fn is_srv6(&self) -> bool {
        self.0 & Self::SRV6.0 != 0
    }

    /// Sets the interface down flag.
    pub fn set_if_down(&mut self, down: bool) {
        if down {
            self.0 |= Self::IF_DOWN.0;
        } else {
            self.0 &= !Self::IF_DOWN.0;
        }
    }
}

impl std::ops::BitOr for NextHopFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for NextHopFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A key identifying a single next-hop.
///
/// Next-hops can be:
/// - IP-based: An IP address with an interface alias
/// - Interface-only: Just an interface (for directly connected routes)
/// - MPLS label: A label value with an interface
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextHopKey {
    /// The IP address of the next-hop (may be unspecified for interface routes).
    ip_address: IpAddress,
    /// The interface alias (e.g., "Ethernet0", "Vlan100").
    alias: String,
    /// VNI for VxLAN tunnels (0 if not applicable).
    vni: u32,
    /// MPLS label (0 if not applicable).
    label: u32,
    /// Weight for weighted ECMP (default 1).
    weight: u32,
    /// Source MAC for overlay routes.
    src_mac: Option<sonic_types::MacAddress>,
    /// Destination MAC for overlay routes.
    dst_mac: Option<sonic_types::MacAddress>,
}

impl NextHopKey {
    /// Creates a new IP-based next-hop key.
    pub fn new(ip_address: IpAddress, alias: impl Into<String>) -> Self {
        Self {
            ip_address,
            alias: alias.into(),
            vni: 0,
            label: 0,
            weight: 1,
            src_mac: None,
            dst_mac: None,
        }
    }

    /// Creates a new interface-only next-hop key.
    pub fn interface_only(alias: impl Into<String>) -> Self {
        Self {
            ip_address: IpAddress::V4(Ipv4Address::UNSPECIFIED),
            alias: alias.into(),
            vni: 0,
            label: 0,
            weight: 1,
            src_mac: None,
            dst_mac: None,
        }
    }

    /// Creates a next-hop with VNI for VxLAN.
    pub fn with_vni(mut self, vni: u32) -> Self {
        self.vni = vni;
        self
    }

    /// Creates a next-hop with MPLS label.
    pub fn with_label(mut self, label: u32) -> Self {
        self.label = label;
        self
    }

    /// Creates a next-hop with weight.
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    /// Creates a next-hop with overlay MACs.
    pub fn with_overlay_macs(
        mut self,
        src_mac: sonic_types::MacAddress,
        dst_mac: sonic_types::MacAddress,
    ) -> Self {
        self.src_mac = Some(src_mac);
        self.dst_mac = Some(dst_mac);
        self
    }

    /// Returns the IP address.
    pub fn ip_address(&self) -> &IpAddress {
        &self.ip_address
    }

    /// Returns the interface alias.
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Returns the VNI (0 if not set).
    pub fn vni(&self) -> u32 {
        self.vni
    }

    /// Returns the MPLS label (0 if not set).
    pub fn label(&self) -> u32 {
        self.label
    }

    /// Returns the weight.
    pub fn weight(&self) -> u32 {
        self.weight
    }

    /// Returns the source MAC for overlay routes.
    pub fn src_mac(&self) -> Option<&sonic_types::MacAddress> {
        self.src_mac.as_ref()
    }

    /// Returns the destination MAC for overlay routes.
    pub fn dst_mac(&self) -> Option<&sonic_types::MacAddress> {
        self.dst_mac.as_ref()
    }

    /// Returns true if this is an interface-only next-hop.
    pub fn is_interface_nexthop(&self) -> bool {
        match &self.ip_address {
            IpAddress::V4(addr) => *addr == Ipv4Address::UNSPECIFIED,
            IpAddress::V6(addr) => *addr == sonic_types::Ipv6Address::UNSPECIFIED,
        }
    }

    /// Returns true if this is an overlay (VxLAN) next-hop.
    pub fn is_overlay(&self) -> bool {
        self.vni > 0
    }

    /// Returns true if this is an MPLS next-hop.
    pub fn is_mpls(&self) -> bool {
        self.label > 0
    }
}

impl Hash for NextHopKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ip_address.hash(state);
        self.alias.hash(state);
        self.vni.hash(state);
        self.label.hash(state);
        // Note: weight is NOT included in hash (same NH with different weights is same NH)
    }
}

impl fmt::Display for NextHopKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_interface_nexthop() {
            write!(f, "{}", self.alias)
        } else if self.vni > 0 {
            write!(f, "{}@{}|{}", self.ip_address, self.alias, self.vni)
        } else if self.label > 0 {
            write!(f, "{}@{}+{}", self.ip_address, self.alias, self.label)
        } else {
            write!(f, "{}@{}", self.ip_address, self.alias)
        }
    }
}

/// Error when parsing a NextHopKey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNextHopKeyError {
    pub message: String,
}

impl fmt::Display for ParseNextHopKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid next-hop key: {}", self.message)
    }
}

impl std::error::Error for ParseNextHopKeyError {}

impl FromStr for NextHopKey {
    type Err = ParseNextHopKeyError;

    /// Parses a next-hop key from string.
    ///
    /// Formats supported:
    /// - `ip@alias` - Standard next-hop
    /// - `ip@alias|vni` - VxLAN tunnel next-hop
    /// - `ip@alias+label` - MPLS next-hop
    /// - `alias` - Interface-only next-hop
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Check for ip@alias format
        if let Some((ip_part, rest)) = s.split_once('@') {
            // Parse VNI: ip@alias|vni
            if let Some((alias, vni_str)) = rest.split_once('|') {
                let ip = ip_part
                    .parse()
                    .map_err(|_| ParseNextHopKeyError {
                        message: format!("Invalid IP address: {}", ip_part),
                    })?;
                let vni = vni_str.parse().map_err(|_| ParseNextHopKeyError {
                    message: format!("Invalid VNI: {}", vni_str),
                })?;
                return Ok(Self::new(ip, alias).with_vni(vni));
            }

            // Parse label: ip@alias+label
            if let Some((alias, label_str)) = rest.split_once('+') {
                let ip = ip_part
                    .parse()
                    .map_err(|_| ParseNextHopKeyError {
                        message: format!("Invalid IP address: {}", ip_part),
                    })?;
                let label = label_str.parse().map_err(|_| ParseNextHopKeyError {
                    message: format!("Invalid label: {}", label_str),
                })?;
                return Ok(Self::new(ip, alias).with_label(label));
            }

            // Standard: ip@alias
            let ip = ip_part
                .parse()
                .map_err(|_| ParseNextHopKeyError {
                    message: format!("Invalid IP address: {}", ip_part),
                })?;
            return Ok(Self::new(ip, rest));
        }

        // Interface-only next-hop
        Ok(Self::interface_only(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use sonic_types::Ipv4Address;

    #[test]
    fn test_nexthop_key_new() {
        let nh = NextHopKey::new(
            IpAddress::V4(Ipv4Addr::new(192, 168, 1, 1).into()),
            "Ethernet0",
        );
        assert_eq!(nh.ip_address(), &IpAddress::V4(Ipv4Addr::new(192, 168, 1, 1).into()));
        assert_eq!(nh.alias(), "Ethernet0");
        assert!(!nh.is_interface_nexthop());
        assert!(!nh.is_overlay());
    }

    #[test]
    fn test_nexthop_key_interface_only() {
        let nh = NextHopKey::interface_only("Vlan100");
        assert!(nh.is_interface_nexthop());
        assert_eq!(nh.alias(), "Vlan100");
    }

    #[test]
    fn test_nexthop_key_vxlan() {
        let nh = NextHopKey::new(
            IpAddress::V4(Ipv4Addr::new(10, 0, 0, 1).into()),
            "Vxlan1",
        )
        .with_vni(1000);
        assert!(nh.is_overlay());
        assert_eq!(nh.vni(), 1000);
    }

    #[test]
    fn test_nexthop_key_mpls() {
        let nh = NextHopKey::new(
            IpAddress::V4(Ipv4Addr::new(10, 0, 0, 1).into()),
            "Ethernet0",
        )
        .with_label(100);
        assert!(nh.is_mpls());
        assert_eq!(nh.label(), 100);
    }

    #[test]
    fn test_nexthop_key_display() {
        let nh = NextHopKey::new(
            IpAddress::V4(Ipv4Addr::new(192, 168, 1, 1).into()),
            "Ethernet0",
        );
        assert_eq!(nh.to_string(), "192.168.1.1@Ethernet0");

        let nh_vni = nh.clone().with_vni(1000);
        assert_eq!(nh_vni.to_string(), "192.168.1.1@Ethernet0|1000");

        let nh_intf = NextHopKey::interface_only("Vlan100");
        assert_eq!(nh_intf.to_string(), "Vlan100");
    }

    #[test]
    fn test_nexthop_key_parse() {
        let nh: NextHopKey = "192.168.1.1@Ethernet0".parse().unwrap();
        assert_eq!(nh.ip_address(), &IpAddress::V4(Ipv4Addr::new(192, 168, 1, 1).into()));
        assert_eq!(nh.alias(), "Ethernet0");

        let nh_vni: NextHopKey = "10.0.0.1@Vxlan1|1000".parse().unwrap();
        assert_eq!(nh_vni.vni(), 1000);

        let nh_label: NextHopKey = "10.0.0.1@Ethernet0+100".parse().unwrap();
        assert_eq!(nh_label.label(), 100);

        let nh_intf: NextHopKey = "Vlan100".parse().unwrap();
        assert!(nh_intf.is_interface_nexthop());
    }

    #[test]
    fn test_nexthop_flags() {
        let mut flags = NextHopFlags::NONE;
        assert!(!flags.is_if_down());

        flags.set_if_down(true);
        assert!(flags.is_if_down());

        let combined = NextHopFlags::IF_DOWN | NextHopFlags::LABEL;
        assert!(combined.is_if_down());
        assert!(combined.is_label());
    }

    #[test]
    fn test_nexthop_key_hash_excludes_weight() {
        use std::collections::hash_map::DefaultHasher;

        let nh1 = NextHopKey::new(
            IpAddress::V4(Ipv4Addr::new(192, 168, 1, 1).into()),
            "Ethernet0",
        )
        .with_weight(1);

        let nh2 = NextHopKey::new(
            IpAddress::V4(Ipv4Addr::new(192, 168, 1, 1).into()),
            "Ethernet0",
        )
        .with_weight(5);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        nh1.hash(&mut hasher1);
        nh2.hash(&mut hasher2);

        // Same hash despite different weights
        assert_eq!(hasher1.finish(), hasher2.finish());
    }
}
