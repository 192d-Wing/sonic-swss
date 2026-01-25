//! Safe wrapper for SAI route API.
//!
//! This module provides type-safe access to SAI route configuration
//! and next-hop group management.

use crate::error::{SaiError, SaiResult};
use crate::types::{
    NextHopGroupMemberOid, NextHopGroupOid, NextHopOid, RouteEntryOid, VirtualRouterOid,
};
use sonic_types::IpPrefix;
use std::collections::HashSet;

/// Next-hop group type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NextHopGroupType {
    /// ECMP (Equal Cost Multi-Path) group
    #[default]
    Ecmp,
    /// WCMP (Weighted Cost Multi-Path) group
    Wcmp,
    /// Fine-grained ECMP group
    FineGrainEcmp,
    /// Class-based group
    ClassBased,
}

/// Route entry representing a destination prefix and VRF.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteEntry {
    /// Virtual router (VRF) this route belongs to
    pub vrf_id: VirtualRouterOid,
    /// Destination IP prefix
    pub destination: IpPrefix,
}

impl RouteEntry {
    /// Creates a new route entry.
    pub fn new(vrf_id: VirtualRouterOid, destination: IpPrefix) -> Self {
        Self {
            vrf_id,
            destination,
        }
    }
}

/// Next-hop group entry with reference counting.
///
/// This struct safely manages reference counts for shared next-hop groups,
/// preventing the auto-vivification bug present in the C++ implementation.
#[derive(Debug, Clone)]
pub struct NextHopGroupEntry {
    /// SAI object ID of this group
    pub id: NextHopGroupOid,
    /// Group type
    pub group_type: NextHopGroupType,
    /// Member next-hops and their weights
    pub members: HashSet<NextHopOid>,
    /// Reference count (routes using this group)
    ref_count: u32,
}

impl NextHopGroupEntry {
    /// Creates a new next-hop group entry.
    pub fn new(id: NextHopGroupOid, group_type: NextHopGroupType) -> Self {
        Self {
            id,
            group_type,
            members: HashSet::new(),
            ref_count: 0,
        }
    }

    /// Returns the current reference count.
    pub fn ref_count(&self) -> u32 {
        self.ref_count
    }

    /// Increments the reference count and returns the new value.
    pub fn increment_ref(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_add(1);
        self.ref_count
    }

    /// Decrements the reference count and returns the new value.
    ///
    /// # Errors
    ///
    /// Returns an error if the reference count would underflow.
    pub fn decrement_ref(&mut self) -> Result<u32, SaiError> {
        if self.ref_count == 0 {
            return Err(SaiError::internal("NextHopGroup reference count underflow"));
        }
        self.ref_count -= 1;
        Ok(self.ref_count)
    }

    /// Returns true if no routes are using this group.
    pub fn is_unreferenced(&self) -> bool {
        self.ref_count == 0
    }
}

/// Route packet action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RouteAction {
    /// Forward the packet
    #[default]
    Forward,
    /// Drop the packet
    Drop,
    /// Send to CPU
    Trap,
    /// Forward and copy to CPU
    Log,
    /// Deny (blackhole)
    Deny,
}

/// Configuration for creating a route.
#[derive(Debug, Clone)]
pub struct RouteConfig {
    /// Route entry (VRF + prefix)
    pub entry: RouteEntry,
    /// Packet action
    pub action: RouteAction,
    /// Next-hop for single NH routes
    pub next_hop: Option<NextHopOid>,
    /// Next-hop group for ECMP routes
    pub next_hop_group: Option<NextHopGroupOid>,
}

/// Safe wrapper for SAI route API.
pub struct RouteApi {
    vrf_id: VirtualRouterOid,
    // When FFI is enabled:
    // route_api: *const sai_route_api_t,
    // nhg_api: *const sai_next_hop_group_api_t,
}

impl RouteApi {
    /// Creates a new RouteApi instance.
    pub fn new(vrf_id: VirtualRouterOid) -> Self {
        Self { vrf_id }
    }

    /// Returns the default VRF ID.
    pub fn vrf_id(&self) -> VirtualRouterOid {
        self.vrf_id
    }

    /// Creates a new route entry.
    ///
    /// # Arguments
    ///
    /// * `config` - Route configuration
    ///
    /// # Errors
    ///
    /// Returns an error if route creation fails.
    pub fn create_route(&self, config: &RouteConfig) -> SaiResult<RouteEntryOid> {
        // Validate configuration
        if config.action == RouteAction::Forward {
            if config.next_hop.is_none() && config.next_hop_group.is_none() {
                return Err(SaiError::invalid_parameter(
                    "forward action requires next_hop or next_hop_group",
                ));
            }
        }

        // TODO: When FFI is enabled, call sai_route_api->create_route_entry()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Removes a route entry.
    pub fn remove_route(&self, entry: &RouteEntry) -> SaiResult<()> {
        if entry.vrf_id.is_null() {
            return Err(SaiError::invalid_parameter("VRF ID is null"));
        }

        // TODO: When FFI is enabled, call sai_route_api->remove_route_entry()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Creates a new next-hop group.
    ///
    /// # Arguments
    ///
    /// * `group_type` - Type of the group (ECMP, WCMP, etc.)
    ///
    /// # Returns
    ///
    /// The OID of the newly created next-hop group.
    pub fn create_next_hop_group(
        &self,
        group_type: NextHopGroupType,
    ) -> SaiResult<NextHopGroupOid> {
        // TODO: When FFI is enabled, call sai_next_hop_group_api->create_next_hop_group()
        let _ = group_type;
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Removes a next-hop group.
    ///
    /// # Errors
    ///
    /// Returns an error if the group is still referenced by routes.
    pub fn remove_next_hop_group(&self, group: NextHopGroupOid) -> SaiResult<()> {
        if group.is_null() {
            return Err(SaiError::invalid_parameter("group OID is null"));
        }

        // TODO: When FFI is enabled, call sai_next_hop_group_api->remove_next_hop_group()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Adds a member to a next-hop group.
    ///
    /// # Arguments
    ///
    /// * `group` - The next-hop group to add to
    /// * `next_hop` - The next-hop to add
    /// * `weight` - Weight for WCMP groups (ignored for ECMP)
    ///
    /// # Returns
    ///
    /// The OID of the newly created group member.
    pub fn add_next_hop_group_member(
        &self,
        group: NextHopGroupOid,
        next_hop: NextHopOid,
        weight: Option<u32>,
    ) -> SaiResult<NextHopGroupMemberOid> {
        if group.is_null() {
            return Err(SaiError::invalid_parameter("group OID is null"));
        }
        if next_hop.is_null() {
            return Err(SaiError::invalid_parameter("next_hop OID is null"));
        }

        // TODO: When FFI is enabled, call sai_next_hop_group_api->create_next_hop_group_member()
        let _ = weight;
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Removes a member from a next-hop group.
    pub fn remove_next_hop_group_member(&self, member: NextHopGroupMemberOid) -> SaiResult<()> {
        if member.is_null() {
            return Err(SaiError::invalid_parameter("member OID is null"));
        }

        // TODO: When FFI is enabled, call sai_next_hop_group_api->remove_next_hop_group_member()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Sets the route action.
    pub fn set_route_action(&self, entry: &RouteEntry, action: RouteAction) -> SaiResult<()> {
        if entry.vrf_id.is_null() {
            return Err(SaiError::invalid_parameter("VRF ID is null"));
        }

        // TODO: When FFI is enabled, call sai_route_api->set_route_entry_attribute()
        let _ = action;
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Sets the next-hop for a route.
    pub fn set_route_next_hop(&self, entry: &RouteEntry, next_hop: NextHopOid) -> SaiResult<()> {
        if entry.vrf_id.is_null() {
            return Err(SaiError::invalid_parameter("VRF ID is null"));
        }
        if next_hop.is_null() {
            return Err(SaiError::invalid_parameter("next_hop OID is null"));
        }

        // TODO: When FFI is enabled, call sai_route_api->set_route_entry_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Sets the next-hop group for a route.
    pub fn set_route_next_hop_group(
        &self,
        entry: &RouteEntry,
        group: NextHopGroupOid,
    ) -> SaiResult<()> {
        if entry.vrf_id.is_null() {
            return Err(SaiError::invalid_parameter("VRF ID is null"));
        }
        if group.is_null() {
            return Err(SaiError::invalid_parameter("group OID is null"));
        }

        // TODO: When FFI is enabled, call sai_route_api->set_route_entry_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nhg_entry_ref_count() {
        let mut entry = NextHopGroupEntry::new(NextHopGroupOid::NULL, NextHopGroupType::Ecmp);

        assert_eq!(entry.ref_count(), 0);
        assert!(entry.is_unreferenced());

        // Increment
        assert_eq!(entry.increment_ref(), 1);
        assert!(!entry.is_unreferenced());

        // Decrement
        assert_eq!(entry.decrement_ref().unwrap(), 0);
        assert!(entry.is_unreferenced());
    }

    #[test]
    fn test_nhg_entry_underflow_protection() {
        let mut entry = NextHopGroupEntry::new(NextHopGroupOid::NULL, NextHopGroupType::Ecmp);

        // Should error on underflow
        assert!(entry.decrement_ref().is_err());
    }

    #[test]
    fn test_route_entry() {
        let vrf = VirtualRouterOid::from_raw(1).unwrap();
        let prefix: IpPrefix = "10.0.0.0/24".parse().unwrap();
        let entry = RouteEntry::new(vrf, prefix);

        assert_eq!(entry.vrf_id, vrf);
        assert_eq!(entry.destination.prefix_len(), 24);
    }

    #[test]
    fn test_route_api_validation() {
        let api = RouteApi::new(VirtualRouterOid::NULL);

        // Forward without nexthop should fail
        let config = RouteConfig {
            entry: RouteEntry::new(VirtualRouterOid::NULL, "10.0.0.0/24".parse().unwrap()),
            action: RouteAction::Forward,
            next_hop: None,
            next_hop_group: None,
        };

        assert!(api.create_route(&config).is_err());
    }
}
