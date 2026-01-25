//! STP types and structures.

use sonic_sai::types::RawSaiObjectId;
use std::collections::{HashMap, HashSet};

/// STP port state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StpState {
    Disabled = 0,
    Blocking = 1,
    Listening = 2,
    Learning = 3,
    Forwarding = 4,
}

impl StpState {
    /// Parses an STP state from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "0" | "disabled" => Some(Self::Disabled),
            "1" | "blocking" => Some(Self::Blocking),
            "2" | "listening" => Some(Self::Listening),
            "3" | "learning" => Some(Self::Learning),
            "4" | "forwarding" => Some(Self::Forwarding),
            _ => None,
        }
    }

    /// Converts to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Blocking => "blocking",
            Self::Listening => "listening",
            Self::Learning => "learning",
            Self::Forwarding => "forwarding",
        }
    }

    /// Converts to SAI STP port state.
    pub fn to_sai_state(self) -> SaiStpPortState {
        match self {
            Self::Disabled | Self::Blocking | Self::Listening => SaiStpPortState::Blocking,
            Self::Learning => SaiStpPortState::Learning,
            Self::Forwarding => SaiStpPortState::Forwarding,
        }
    }
}

/// SAI STP port states (subset used by SAI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaiStpPortState {
    Blocking,
    Learning,
    Forwarding,
}

/// STP instance entry tracking VLANs.
#[derive(Debug, Clone)]
pub struct StpInstanceEntry {
    /// SAI STP instance object ID.
    pub stp_inst_oid: RawSaiObjectId,
    /// Set of VLAN aliases associated with this instance.
    pub vlan_list: HashSet<String>,
}

impl StpInstanceEntry {
    /// Creates a new STP instance entry.
    pub fn new(stp_inst_oid: RawSaiObjectId) -> Self {
        Self {
            stp_inst_oid,
            vlan_list: HashSet::new(),
        }
    }

    /// Adds a VLAN to the instance.
    pub fn add_vlan(&mut self, vlan_alias: String) -> bool {
        self.vlan_list.insert(vlan_alias)
    }

    /// Removes a VLAN from the instance.
    pub fn remove_vlan(&mut self, vlan_alias: &str) -> bool {
        self.vlan_list.remove(vlan_alias)
    }

    /// Returns the number of VLANs in this instance.
    pub fn vlan_count(&self) -> usize {
        self.vlan_list.len()
    }
}

/// STP port identifiers map: instance ID → STP port OID.
pub type StpPortIds = HashMap<u16, RawSaiObjectId>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stp_state_parse() {
        assert_eq!(StpState::parse("0"), Some(StpState::Disabled));
        assert_eq!(StpState::parse("disabled"), Some(StpState::Disabled));
        assert_eq!(StpState::parse("1"), Some(StpState::Blocking));
        assert_eq!(StpState::parse("blocking"), Some(StpState::Blocking));
        assert_eq!(StpState::parse("4"), Some(StpState::Forwarding));
        assert_eq!(StpState::parse("forwarding"), Some(StpState::Forwarding));
        assert_eq!(StpState::parse("invalid"), None);
    }

    #[test]
    fn test_stp_state_to_sai() {
        assert_eq!(StpState::Disabled.to_sai_state(), SaiStpPortState::Blocking);
        assert_eq!(StpState::Blocking.to_sai_state(), SaiStpPortState::Blocking);
        assert_eq!(
            StpState::Listening.to_sai_state(),
            SaiStpPortState::Blocking
        );
        assert_eq!(StpState::Learning.to_sai_state(), SaiStpPortState::Learning);
        assert_eq!(
            StpState::Forwarding.to_sai_state(),
            SaiStpPortState::Forwarding
        );
    }

    #[test]
    fn test_stp_instance_entry() {
        let mut entry = StpInstanceEntry::new(0x1234);

        assert_eq!(entry.vlan_count(), 0);

        assert!(entry.add_vlan("Vlan100".to_string()));
        assert_eq!(entry.vlan_count(), 1);

        // Adding same VLAN returns false
        assert!(!entry.add_vlan("Vlan100".to_string()));
        assert_eq!(entry.vlan_count(), 1);

        assert!(entry.add_vlan("Vlan200".to_string()));
        assert_eq!(entry.vlan_count(), 2);

        assert!(entry.remove_vlan("Vlan100"));
        assert_eq!(entry.vlan_count(), 1);

        assert!(!entry.remove_vlan("Vlan100"));
        assert_eq!(entry.vlan_count(), 1);
    }
}
