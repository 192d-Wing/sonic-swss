//! Type definitions for vlanmgrd

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// VLAN configuration information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VlanInfo {
    /// VLAN ID
    pub vlan_id: u16,
    /// Admin status ("up" or "down")
    pub admin_status: String,
    /// MTU
    pub mtu: u32,
    /// MAC address
    pub mac: String,
    /// VLAN members: port_alias -> tagging_mode
    pub members: HashMap<String, String>,
}

impl VlanInfo {
    /// Create a new VlanInfo with default values
    pub fn new(vlan_id: u16) -> Self {
        Self {
            vlan_id,
            admin_status: "up".to_string(),
            mtu: 9100,
            mac: String::new(),
            members: HashMap::new(),
        }
    }
}

/// VLAN member information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VlanMemberInfo {
    /// VLAN ID
    pub vlan_id: u16,
    /// Port alias
    pub port_alias: String,
    /// Tagging mode
    pub tagging_mode: TaggingMode,
}

impl VlanMemberInfo {
    /// Create a new VlanMemberInfo
    pub fn new(vlan_id: u16, port_alias: impl Into<String>, tagging_mode: TaggingMode) -> Self {
        Self {
            vlan_id,
            port_alias: port_alias.into(),
            tagging_mode,
        }
    }
}

/// VLAN tagging mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaggingMode {
    /// Tagged mode
    Tagged,
    /// Untagged mode
    Untagged,
    /// Priority tagged mode
    PriorityTagged,
}

impl FromStr for TaggingMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "tagged" => TaggingMode::Tagged,
            "untagged" => TaggingMode::Untagged,
            "priority_tagged" => TaggingMode::PriorityTagged,
            _ => TaggingMode::Tagged, // Default to tagged
        })
    }
}

impl TaggingMode {
    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            TaggingMode::Tagged => "tagged",
            TaggingMode::Untagged => "untagged",
            TaggingMode::PriorityTagged => "priority_tagged",
        }
    }

    /// Convert to bridge command argument
    pub fn to_bridge_cmd(&self) -> &str {
        match self {
            TaggingMode::Tagged => "",
            TaggingMode::Untagged | TaggingMode::PriorityTagged => "pvid untagged",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vlan_info_new() {
        let info = VlanInfo::new(100);
        assert_eq!(info.vlan_id, 100);
        assert_eq!(info.admin_status, "up");
        assert_eq!(info.mtu, 9100);
        assert!(info.members.is_empty());
    }

    #[test]
    fn test_tagging_mode_from_str() {
        assert_eq!(
            "tagged".parse::<TaggingMode>().unwrap(),
            TaggingMode::Tagged
        );
        assert_eq!(
            "untagged".parse::<TaggingMode>().unwrap(),
            TaggingMode::Untagged
        );
        assert_eq!(
            "priority_tagged".parse::<TaggingMode>().unwrap(),
            TaggingMode::PriorityTagged
        );
        assert_eq!(
            "invalid".parse::<TaggingMode>().unwrap(),
            TaggingMode::Tagged
        );
    }

    #[test]
    fn test_tagging_mode_to_bridge_cmd() {
        assert_eq!(TaggingMode::Tagged.to_bridge_cmd(), "");
        assert_eq!(TaggingMode::Untagged.to_bridge_cmd(), "pvid untagged");
        assert_eq!(TaggingMode::PriorityTagged.to_bridge_cmd(), "pvid untagged");
    }

    #[test]
    fn test_vlan_member_info_new() {
        let member = VlanMemberInfo::new(100, "Ethernet0", TaggingMode::Untagged);
        assert_eq!(member.vlan_id, 100);
        assert_eq!(member.port_alias, "Ethernet0");
        assert_eq!(member.tagging_mode, TaggingMode::Untagged);
    }
}
