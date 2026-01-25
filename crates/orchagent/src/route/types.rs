//! Route types and data structures.
//!
//! This module defines the route entry types and storage structures.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpPrefix;
use std::collections::HashMap;
use std::fmt;

use super::nhg::NextHopGroupKey;

/// A key identifying a route (VRF ID + IP prefix).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteKey {
    /// VRF ID (SAI object ID, 0 for default VRF).
    pub vrf_id: RawSaiObjectId,
    /// IP prefix (e.g., 10.0.0.0/24).
    pub prefix: IpPrefix,
}

impl RouteKey {
    /// Creates a new route key.
    pub fn new(vrf_id: RawSaiObjectId, prefix: IpPrefix) -> Self {
        Self { vrf_id, prefix }
    }

    /// Creates a route key in the default VRF.
    pub fn default_vrf(prefix: IpPrefix) -> Self {
        Self::new(0, prefix)
    }

    /// Returns true if this is in the default VRF.
    pub fn is_default_vrf(&self) -> bool {
        self.vrf_id == 0
    }

    /// Returns true if this is a default route (0.0.0.0/0 or ::/0).
    pub fn is_default_route(&self) -> bool {
        self.prefix.is_default()
    }
}

impl fmt::Display for RouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_default_vrf() {
            write!(f, "{}", self.prefix)
        } else {
            write!(f, "vrf:{:x}/{}", self.vrf_id, self.prefix)
        }
    }
}

/// Next-hop group reference for a route.
///
/// This is the Rust equivalent of C++ `RouteNhg`. It stores the relationship
/// between a route and its next-hop group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteNhg {
    /// The next-hop group key (the actual next-hops).
    pub nhg_key: NextHopGroupKey,
    /// If the NHG is owned by NhgOrch, this is its index.
    pub nhg_index: Option<String>,
    /// SRv6 context index (if applicable).
    pub context_index: Option<String>,
}

impl RouteNhg {
    /// Creates a new RouteNhg with just the key.
    pub fn new(nhg_key: NextHopGroupKey) -> Self {
        Self {
            nhg_key,
            nhg_index: None,
            context_index: None,
        }
    }

    /// Creates a RouteNhg with an NhgOrch index.
    pub fn with_nhg_index(mut self, index: impl Into<String>) -> Self {
        self.nhg_index = Some(index.into());
        self
    }

    /// Creates a RouteNhg with an SRv6 context index.
    pub fn with_context_index(mut self, index: impl Into<String>) -> Self {
        self.context_index = Some(index.into());
        self
    }

    /// Returns true if this NHG is owned by NhgOrch.
    pub fn is_nhg_orch_owned(&self) -> bool {
        self.nhg_index.is_some()
    }

    /// Returns true if this is an SRv6 route.
    pub fn is_srv6(&self) -> bool {
        self.context_index.is_some()
    }

    /// Returns true if this is a blackhole/dropped route.
    pub fn is_blackhole(&self) -> bool {
        self.nhg_key.is_empty() && self.nhg_index.is_none()
    }
}

impl Default for RouteNhg {
    fn default() -> Self {
        Self::new(NextHopGroupKey::new())
    }
}

/// A route entry in the synced routes table.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    /// The route's next-hop group reference.
    pub nhg: RouteNhg,
    /// SAI route entry OID (if created).
    pub sai_route_id: Option<RawSaiObjectId>,
    /// Whether this route is pending removal.
    pub pending_removal: bool,
    /// Whether this route was marked dirty during resync.
    pub dirty: bool,
}

impl RouteEntry {
    /// Creates a new route entry.
    pub fn new(nhg: RouteNhg) -> Self {
        Self {
            nhg,
            sai_route_id: None,
            pending_removal: false,
            dirty: false,
        }
    }

    /// Creates a blackhole route entry.
    pub fn blackhole() -> Self {
        Self::new(RouteNhg::default())
    }

    /// Sets the SAI route ID.
    pub fn with_sai_id(mut self, id: RawSaiObjectId) -> Self {
        self.sai_route_id = Some(id);
        self
    }

    /// Returns true if this is a blackhole route.
    pub fn is_blackhole(&self) -> bool {
        self.nhg.is_blackhole()
    }

    /// Marks this route as dirty for resync.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clears the dirty flag.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

/// Table of routes indexed by IP prefix.
pub type RouteTable = HashMap<IpPrefix, RouteEntry>;

/// Table of routes indexed by VRF ID.
///
/// Structure: VRF ID → (IP Prefix → RouteEntry)
pub type RouteTables = HashMap<RawSaiObjectId, RouteTable>;

/// Label route entry for MPLS.
#[derive(Debug, Clone)]
pub struct LabelRouteEntry {
    /// The route's next-hop group reference.
    pub nhg: RouteNhg,
    /// SAI MPLS route entry OID (if created).
    pub sai_route_id: Option<RawSaiObjectId>,
}

impl LabelRouteEntry {
    /// Creates a new label route entry.
    pub fn new(nhg: RouteNhg) -> Self {
        Self {
            nhg,
            sai_route_id: None,
        }
    }
}

/// MPLS label.
pub type Label = u32;

/// Table of label routes indexed by label.
pub type LabelRouteTable = HashMap<Label, LabelRouteEntry>;

/// Table of label routes indexed by VRF ID.
pub type LabelRouteTables = HashMap<RawSaiObjectId, LabelRouteTable>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_route_key_default_vrf() {
        let prefix = IpPrefix::new(
            sonic_types::IpAddress::V4(Ipv4Addr::new(10, 0, 0, 0).into()),
            24,
        )
        .unwrap();
        let key = RouteKey::default_vrf(prefix.clone());

        assert!(key.is_default_vrf());
        assert_eq!(key.prefix, prefix);
        assert_eq!(key.to_string(), "10.0.0.0/24");
    }

    #[test]
    fn test_route_key_with_vrf() {
        let prefix = IpPrefix::new(
            sonic_types::IpAddress::V4(Ipv4Addr::new(10, 0, 0, 0).into()),
            24,
        )
        .unwrap();
        let key = RouteKey::new(0x1234, prefix);

        assert!(!key.is_default_vrf());
        assert!(key.to_string().contains("vrf:1234"));
    }

    #[test]
    fn test_route_key_default_route() {
        // Default route is 0.0.0.0/0
        let prefix = IpPrefix::new(
            sonic_types::IpAddress::V4(Ipv4Addr::new(0, 0, 0, 0).into()),
            0,
        )
        .unwrap();
        let key = RouteKey::default_vrf(prefix);

        assert!(key.is_default_route());
    }

    #[test]
    fn test_route_nhg_basic() {
        let nhg = RouteNhg::new(NextHopGroupKey::new());
        assert!(nhg.is_blackhole());
        assert!(!nhg.is_nhg_orch_owned());
        assert!(!nhg.is_srv6());
    }

    #[test]
    fn test_route_nhg_with_index() {
        let nhg = RouteNhg::new(NextHopGroupKey::new()).with_nhg_index("nhg_1");
        assert!(nhg.is_nhg_orch_owned());
        assert!(!nhg.is_blackhole()); // Has nhg_index, so not blackhole
    }

    #[test]
    fn test_route_entry() {
        let nhg = RouteNhg::default();
        let mut entry = RouteEntry::new(nhg);

        assert!(entry.is_blackhole());
        assert!(!entry.dirty);

        entry.mark_dirty();
        assert!(entry.dirty);

        entry.clear_dirty();
        assert!(!entry.dirty);
    }

    #[test]
    fn test_route_tables() {
        let mut tables: RouteTables = HashMap::new();

        let prefix = IpPrefix::new(
            sonic_types::IpAddress::V4(Ipv4Addr::new(10, 0, 0, 0).into()),
            24,
        )
        .unwrap();

        // Add to default VRF
        tables
            .entry(0)
            .or_default()
            .insert(prefix.clone(), RouteEntry::new(RouteNhg::default()));

        assert!(tables.contains_key(&0));
        assert!(tables.get(&0).unwrap().contains_key(&prefix));
    }
}
