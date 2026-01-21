//! Port-related types and table definitions.
//!
//! This module defines the core types used by PortsOrch for managing
//! ports, LAGs, VLANs, and related state.

use sonic_sai::types::RawSaiObjectId;
use std::collections::{HashMap, HashSet};

use super::port::Port;

/// Port initialization state machine states.
///
/// Ports go through this state machine during initialization:
/// - `ConfigMissing`: Port exists in hardware but no config received
/// - `ConfigReceived`: Config received from CONFIG_DB, pending application
/// - `ConfigDone`: Port fully configured and operational
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortInitState {
    /// Port config has not been received from CONFIG_DB.
    #[default]
    ConfigMissing,
    /// Port config received but not yet applied to SAI.
    ConfigReceived,
    /// Port is fully configured and ready.
    ConfigDone,
}

impl std::fmt::Display for PortInitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigMissing => write!(f, "CONFIG_MISSING"),
            Self::ConfigReceived => write!(f, "CONFIG_RECEIVED"),
            Self::ConfigDone => write!(f, "CONFIG_DONE"),
        }
    }
}

/// Table of ports indexed by port alias (e.g., "Ethernet0").
///
/// Uses `sonic_orch_common::SyncMap` to prevent auto-vivification bugs.
pub type PortTable = sonic_orch_common::SyncMap<String, Port>;

/// LAG (Link Aggregation Group) information.
#[derive(Debug, Clone)]
pub struct LagInfo {
    /// SAI object ID for the LAG.
    pub lag_id: RawSaiObjectId,
    /// Alias (e.g., "PortChannel0001").
    pub alias: String,
    /// Member ports (port aliases).
    pub members: HashSet<String>,
    /// LAG operational state.
    pub oper_status: bool,
    /// MTU configured on LAG.
    pub mtu: u32,
    /// Admin status (up/down).
    pub admin_status: bool,
}

impl LagInfo {
    /// Creates a new LAG info entry.
    pub fn new(lag_id: RawSaiObjectId, alias: impl Into<String>) -> Self {
        Self {
            lag_id,
            alias: alias.into(),
            members: HashSet::new(),
            oper_status: false,
            mtu: 9100,
            admin_status: true,
        }
    }

    /// Returns the LAG's SAI object ID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.lag_id
    }

    /// Adds a member port to the LAG.
    pub fn add_member(&mut self, port_alias: impl Into<String>) {
        self.members.insert(port_alias.into());
    }

    /// Removes a member port from the LAG.
    pub fn remove_member(&mut self, port_alias: &str) -> bool {
        self.members.remove(port_alias)
    }

    /// Returns true if the port is a member of this LAG.
    pub fn has_member(&self, port_alias: &str) -> bool {
        self.members.contains(port_alias)
    }

    /// Returns the number of member ports.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

/// Table of LAGs indexed by LAG alias.
pub type LagTable = sonic_orch_common::SyncMap<String, LagInfo>;

/// VLAN member information.
#[derive(Debug, Clone)]
pub struct VlanMemberInfo {
    /// SAI object ID for the VLAN member.
    pub vlan_member_id: RawSaiObjectId,
    /// Bridge port ID (for the port in this VLAN).
    pub bridge_port_id: RawSaiObjectId,
    /// Tagging mode (tagged/untagged).
    pub tagging_mode: VlanTaggingMode,
}

/// VLAN tagging mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VlanTaggingMode {
    /// Packets are tagged with VLAN ID.
    #[default]
    Tagged,
    /// Packets are untagged (native VLAN).
    Untagged,
    /// Priority tagged (VLAN ID 0).
    PriorityTagged,
}

impl std::fmt::Display for VlanTaggingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tagged => write!(f, "tagged"),
            Self::Untagged => write!(f, "untagged"),
            Self::PriorityTagged => write!(f, "priority_tagged"),
        }
    }
}

impl std::str::FromStr for VlanTaggingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tagged" => Ok(Self::Tagged),
            "untagged" => Ok(Self::Untagged),
            "priority_tagged" | "priority-tagged" => Ok(Self::PriorityTagged),
            _ => Err(format!("Unknown tagging mode: {}", s)),
        }
    }
}

/// VLAN information.
#[derive(Debug, Clone)]
pub struct VlanInfo {
    /// SAI object ID for the VLAN.
    pub vlan_id: RawSaiObjectId,
    /// VLAN ID (1-4094).
    pub vlan_number: u16,
    /// Alias (e.g., "Vlan100").
    pub alias: String,
    /// Members: port alias → member info.
    pub members: HashMap<String, VlanMemberInfo>,
    /// Host interface ID (for CPU port).
    pub host_if_id: Option<RawSaiObjectId>,
    /// MAC learning enabled.
    pub mac_learning: bool,
}

impl VlanInfo {
    /// Creates a new VLAN info entry.
    pub fn new(vlan_id: RawSaiObjectId, vlan_number: u16, alias: impl Into<String>) -> Self {
        Self {
            vlan_id,
            vlan_number,
            alias: alias.into(),
            members: HashMap::new(),
            host_if_id: None,
            mac_learning: true,
        }
    }

    /// Returns the VLAN's SAI object ID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.vlan_id
    }

    /// Adds a member port to the VLAN.
    pub fn add_member(&mut self, port_alias: impl Into<String>, member_info: VlanMemberInfo) {
        self.members.insert(port_alias.into(), member_info);
    }

    /// Removes a member port from the VLAN.
    pub fn remove_member(&mut self, port_alias: &str) -> Option<VlanMemberInfo> {
        self.members.remove(port_alias)
    }

    /// Returns true if the port is a member of this VLAN.
    pub fn has_member(&self, port_alias: &str) -> bool {
        self.members.contains_key(port_alias)
    }

    /// Returns the number of member ports.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

/// Table of VLANs indexed by VLAN alias.
pub type VlanTable = sonic_orch_common::SyncMap<String, VlanInfo>;

/// Gearbox port information (for external PHY/gearbox chips).
#[derive(Debug, Clone)]
pub struct GearboxPortInfo {
    /// SAI object ID for the gearbox port.
    pub port_id: RawSaiObjectId,
    /// Physical port index in gearbox.
    pub phy_index: u32,
    /// Line side lanes.
    pub line_lanes: Vec<u32>,
    /// System side lanes.
    pub system_lanes: Vec<u32>,
    /// Gearbox ID (which gearbox chip).
    pub gearbox_id: u32,
}

/// Table of gearbox ports indexed by port alias.
pub type GearboxPortTable = sonic_orch_common::SyncMap<String, GearboxPortInfo>;

/// System port information (for distributed systems/VOQ).
#[derive(Debug, Clone)]
pub struct SystemPortInfo {
    /// SAI object ID for the system port.
    pub system_port_id: RawSaiObjectId,
    /// System port number (global identifier).
    pub system_port_number: u32,
    /// Core index.
    pub core_index: u32,
    /// Core port index within the core.
    pub core_port_index: u32,
    /// Speed in Mbps.
    pub speed: u32,
    /// Number of VOQs.
    pub num_voq: u32,
}

/// Table of system ports indexed by system port number.
pub type SystemPortTable = sonic_orch_common::SyncMap<u32, SystemPortInfo>;

/// Port supported speeds information.
#[derive(Debug, Clone, Default)]
pub struct PortSupportedSpeeds {
    /// List of supported speeds in Mbps.
    pub speeds: Vec<u32>,
}

impl PortSupportedSpeeds {
    /// Creates a new supported speeds entry.
    pub fn new(speeds: Vec<u32>) -> Self {
        Self { speeds }
    }

    /// Returns true if the speed is supported.
    pub fn supports(&self, speed: u32) -> bool {
        self.speeds.contains(&speed)
    }

    /// Returns the maximum supported speed.
    pub fn max_speed(&self) -> Option<u32> {
        self.speeds.iter().max().copied()
    }

    /// Returns the minimum supported speed.
    pub fn min_speed(&self) -> Option<u32> {
        self.speeds.iter().min().copied()
    }
}

/// Port lane mapping information.
#[derive(Debug, Clone)]
pub struct PortLaneMapping {
    /// Physical lanes used by the port.
    pub lanes: Vec<u32>,
    /// Lane speed (per lane) in Mbps.
    pub lane_speed: u32,
}

impl PortLaneMapping {
    /// Creates a new lane mapping.
    pub fn new(lanes: Vec<u32>, lane_speed: u32) -> Self {
        Self { lanes, lane_speed }
    }

    /// Returns the number of lanes.
    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    /// Returns the total port speed (lanes × lane_speed).
    pub fn total_speed(&self) -> u32 {
        (self.lanes.len() as u32) * self.lane_speed
    }
}

/// Statistics for tracking port operations.
#[derive(Debug, Clone, Default)]
pub struct PortsOrchStats {
    /// Number of ports created.
    pub ports_created: u64,
    /// Number of ports deleted.
    pub ports_deleted: u64,
    /// Number of port config changes.
    pub port_config_changes: u64,
    /// Number of LAGs created.
    pub lags_created: u64,
    /// Number of LAGs deleted.
    pub lags_deleted: u64,
    /// Number of VLANs created.
    pub vlans_created: u64,
    /// Number of VLANs deleted.
    pub vlans_deleted: u64,
    /// Number of SAI errors encountered.
    pub sai_errors: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_init_state_display() {
        assert_eq!(PortInitState::ConfigMissing.to_string(), "CONFIG_MISSING");
        assert_eq!(PortInitState::ConfigReceived.to_string(), "CONFIG_RECEIVED");
        assert_eq!(PortInitState::ConfigDone.to_string(), "CONFIG_DONE");
    }

    #[test]
    fn test_vlan_tagging_mode_parse() {
        assert_eq!("tagged".parse::<VlanTaggingMode>().unwrap(), VlanTaggingMode::Tagged);
        assert_eq!("untagged".parse::<VlanTaggingMode>().unwrap(), VlanTaggingMode::Untagged);
        assert_eq!("priority_tagged".parse::<VlanTaggingMode>().unwrap(), VlanTaggingMode::PriorityTagged);
        assert!("invalid".parse::<VlanTaggingMode>().is_err());
    }

    #[test]
    fn test_lag_info() {
        let mut lag = LagInfo::new(0x1234, "PortChannel0001");
        assert_eq!(lag.alias, "PortChannel0001");
        assert_eq!(lag.member_count(), 0);

        lag.add_member("Ethernet0");
        lag.add_member("Ethernet4");
        assert_eq!(lag.member_count(), 2);
        assert!(lag.has_member("Ethernet0"));

        lag.remove_member("Ethernet0");
        assert_eq!(lag.member_count(), 1);
        assert!(!lag.has_member("Ethernet0"));
    }

    #[test]
    fn test_vlan_info() {
        let mut vlan = VlanInfo::new(0x5678, 100, "Vlan100");
        assert_eq!(vlan.vlan_number, 100);
        assert_eq!(vlan.member_count(), 0);

        let member = VlanMemberInfo {
            vlan_member_id: 0x1111,
            bridge_port_id: 0x2222,
            tagging_mode: VlanTaggingMode::Tagged,
        };
        vlan.add_member("Ethernet0", member);
        assert!(vlan.has_member("Ethernet0"));
        assert_eq!(vlan.member_count(), 1);

        let removed = vlan.remove_member("Ethernet0");
        assert!(removed.is_some());
        assert!(!vlan.has_member("Ethernet0"));
    }

    #[test]
    fn test_port_supported_speeds() {
        let speeds = PortSupportedSpeeds::new(vec![10000, 25000, 40000, 100000]);
        assert!(speeds.supports(25000));
        assert!(!speeds.supports(50000));
        assert_eq!(speeds.max_speed(), Some(100000));
        assert_eq!(speeds.min_speed(), Some(10000));
    }

    #[test]
    fn test_port_lane_mapping() {
        let mapping = PortLaneMapping::new(vec![0, 1, 2, 3], 25000);
        assert_eq!(mapping.lane_count(), 4);
        assert_eq!(mapping.total_speed(), 100000);
    }
}
