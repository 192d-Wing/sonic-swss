//! Type definitions for vrfmgrd

use serde::{Deserialize, Serialize};

/// VRF routing table configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrfInfo {
    /// VRF name
    pub name: String,
    /// Routing table ID (1001-2000, or 5000 for mgmt)
    pub table_id: u32,
    /// VNI for EVPN (optional)
    pub vni: Option<u32>,
}

impl VrfInfo {
    /// Create a new VrfInfo
    pub fn new(name: impl Into<String>, table_id: u32) -> Self {
        Self {
            name: name.into(),
            table_id,
            vni: None,
        }
    }

    /// Create a new VrfInfo with VNI
    pub fn with_vni(name: impl Into<String>, table_id: u32, vni: u32) -> Self {
        Self {
            name: name.into(),
            table_id,
            vni: Some(vni),
        }
    }
}

/// VRF table ID pool constants
pub const VRF_TABLE_START: u32 = 1001;
pub const VRF_TABLE_END: u32 = 2000;

/// Local routing rule preference (after l3mdev-table)
pub const TABLE_LOCAL_PREF: u32 = 1001;

/// Management VRF table ID (reserved)
pub const MGMT_VRF_TABLE_ID: u32 = 5000;

/// Management VRF name
pub const MGMT_VRF_NAME: &str = "mgmt";

/// EVPN NVO configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvpnNvoConfig {
    /// NVO name
    pub nvo_name: String,
    /// Source VXLAN tunnel name
    pub source_vtep: String,
}

impl EvpnNvoConfig {
    /// Create a new EvpnNvoConfig
    pub fn new(nvo_name: impl Into<String>, source_vtep: impl Into<String>) -> Self {
        Self {
            nvo_name: nvo_name.into(),
            source_vtep: source_vtep.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vrf_info_new() {
        let info = VrfInfo::new("Vrf1", 1001);
        assert_eq!(info.name, "Vrf1");
        assert_eq!(info.table_id, 1001);
        assert_eq!(info.vni, None);
    }

    #[test]
    fn test_vrf_info_with_vni() {
        let info = VrfInfo::with_vni("Vrf1", 1001, 1000);
        assert_eq!(info.name, "Vrf1");
        assert_eq!(info.table_id, 1001);
        assert_eq!(info.vni, Some(1000));
    }

    #[test]
    fn test_constants() {
        assert_eq!(VRF_TABLE_START, 1001);
        assert_eq!(VRF_TABLE_END, 2000);
        assert_eq!(MGMT_VRF_TABLE_ID, 5000);
        assert_eq!(MGMT_VRF_NAME, "mgmt");
    }

    #[test]
    fn test_evpn_nvo_config_new() {
        let config = EvpnNvoConfig::new("nvo1", "vtep");
        assert_eq!(config.nvo_name, "nvo1");
        assert_eq!(config.source_vtep, "vtep");
    }
}
