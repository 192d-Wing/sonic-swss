//! Interface Manager Type Definitions

use std::collections::{HashMap, HashSet};

/// Sub-interface information
#[derive(Debug, Clone, PartialEq)]
pub struct SubIntfInfo {
    /// VLAN ID for this sub-interface
    pub vlan_id: String,

    /// MTU setting
    pub mtu: String,

    /// Desired admin status
    pub admin_status: String,

    /// Current admin status (cached)
    pub curr_admin_status: String,
}

impl SubIntfInfo {
    pub fn new(vlan_id: String) -> Self {
        Self {
            vlan_id,
            mtu: String::new(),
            admin_status: String::new(),
            curr_admin_status: String::new(),
        }
    }
}

/// Sub-interface name → info mapping
pub type SubIntfMap = HashMap<String, SubIntfInfo>;

/// Interface type classification
#[derive(Debug, Clone, PartialEq)]
pub enum IntfType {
    /// Physical port (e.g., Ethernet0)
    Physical(String),

    /// VLAN interface (e.g., Vlan100)
    Vlan(String),

    /// LAG interface (e.g., PortChannel1)
    Lag(String),

    /// Loopback interface (e.g., Loopback0)
    Loopback(String),

    /// Sub-interface (e.g., Ethernet0.100)
    SubInterface { parent: String, vlan_id: String },
}

impl IntfType {
    /// Parse interface name and classify type
    pub fn from_name(name: &str) -> Option<Self> {
        if name.starts_with("Ethernet") {
            if name.contains('.') {
                // Sub-interface
                let (parent, vlan_id) = crate::subintf::parse_subintf_name(name)?;
                Some(IntfType::SubInterface { parent, vlan_id })
            } else {
                Some(IntfType::Physical(name.to_string()))
            }
        } else if name.starts_with("Vlan") {
            Some(IntfType::Vlan(name.to_string()))
        } else if name.starts_with("PortChannel") {
            if name.contains('.') {
                let (parent, vlan_id) = crate::subintf::parse_subintf_name(name)?;
                Some(IntfType::SubInterface { parent, vlan_id })
            } else {
                Some(IntfType::Lag(name.to_string()))
            }
        } else if name.starts_with("Po") && name.contains('.') {
            // Short LAG sub-interface (Po1.100)
            let (parent, vlan_id) = crate::subintf::parse_subintf_name(name)?;
            Some(IntfType::SubInterface { parent, vlan_id })
        } else if name.starts_with("Loopback") {
            Some(IntfType::Loopback(name.to_string()))
        } else {
            None
        }
    }

    /// Check if this is a sub-interface
    pub fn is_sub_interface(&self) -> bool {
        matches!(self, IntfType::SubInterface { .. })
    }

    /// Get the interface name
    pub fn name(&self) -> &str {
        match self {
            IntfType::Physical(n)
            | IntfType::Vlan(n)
            | IntfType::Lag(n)
            | IntfType::Loopback(n) => n,
            IntfType::SubInterface { parent, vlan_id: _ } => {
                // Note: This is a simplified version, caller should reconstruct full name
                parent
            }
        }
    }
}

/// Switch type
#[derive(Debug, Clone, PartialEq)]
pub enum SwitchType {
    /// Normal switch
    Normal,

    /// VOQ (Virtual Output Queue) switch
    Voq,
}

impl SwitchType {
    /// Parse switch type from string
    pub fn from_str(s: &str) -> Self {
        if s.eq_ignore_ascii_case("voq") {
            SwitchType::Voq
        } else {
            SwitchType::Normal
        }
    }

    /// Check if this is a VOQ switch
    pub fn is_voq(&self) -> bool {
        matches!(self, SwitchType::Voq)
    }
}

/// Interface state tracking
pub type IntfStateMap = HashMap<String, String>;

/// Loopback interface set
pub type LoopbackIntfSet = HashSet<String>;

/// Pending replay interface set (for warm restart)
pub type PendingReplayIntfSet = HashSet<String>;

/// IPv6 link-local mode interface set
pub type Ipv6LinkLocalModeSet = HashSet<String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subintf_info_new() {
        let info = SubIntfInfo::new("100".to_string());
        assert_eq!(info.vlan_id, "100");
        assert!(info.mtu.is_empty());
        assert!(info.admin_status.is_empty());
    }

    #[test]
    fn test_intf_type_from_name_physical() {
        let intf_type = IntfType::from_name("Ethernet0").unwrap();
        assert!(matches!(intf_type, IntfType::Physical(_)));
    }

    #[test]
    fn test_intf_type_from_name_vlan() {
        let intf_type = IntfType::from_name("Vlan100").unwrap();
        assert!(matches!(intf_type, IntfType::Vlan(_)));
    }

    #[test]
    fn test_intf_type_from_name_lag() {
        let intf_type = IntfType::from_name("PortChannel1").unwrap();
        assert!(matches!(intf_type, IntfType::Lag(_)));
    }

    #[test]
    fn test_intf_type_from_name_loopback() {
        let intf_type = IntfType::from_name("Loopback0").unwrap();
        assert!(matches!(intf_type, IntfType::Loopback(_)));
    }

    #[test]
    fn test_intf_type_from_name_subintf() {
        let intf_type = IntfType::from_name("Ethernet0.100").unwrap();
        match intf_type {
            IntfType::SubInterface { parent, vlan_id } => {
                assert_eq!(parent, "Ethernet0");
                assert_eq!(vlan_id, "100");
            }
            _ => panic!("Expected SubInterface"),
        }
    }

    #[test]
    fn test_intf_type_is_sub_interface() {
        let subintf = IntfType::SubInterface {
            parent: "Ethernet0".to_string(),
            vlan_id: "100".to_string(),
        };
        assert!(subintf.is_sub_interface());

        let physical = IntfType::Physical("Ethernet0".to_string());
        assert!(!physical.is_sub_interface());
    }

    #[test]
    fn test_switch_type_from_str() {
        assert_eq!(SwitchType::from_str("voq"), SwitchType::Voq);
        assert_eq!(SwitchType::from_str("VOQ"), SwitchType::Voq);
        assert_eq!(SwitchType::from_str("normal"), SwitchType::Normal);
        assert_eq!(SwitchType::from_str(""), SwitchType::Normal);
    }

    #[test]
    fn test_switch_type_is_voq() {
        assert!(SwitchType::Voq.is_voq());
        assert!(!SwitchType::Normal.is_voq());
    }
}
