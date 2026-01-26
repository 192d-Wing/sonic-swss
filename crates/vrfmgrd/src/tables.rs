//! Database table name constants for vrfmgrd

// CONFIG_DB tables
/// VRF table in CONFIG_DB
pub const CFG_VRF_TABLE_NAME: &str = "VRF";

/// VXLAN tunnel table in CONFIG_DB (for VRF-VNI mapping)
pub const CFG_VXLAN_TUNNEL_TABLE_NAME: &str = "VXLAN_TUNNEL";

/// Management VRF configuration table in CONFIG_DB
pub const CFG_MGMT_VRF_CONFIG_TABLE_NAME: &str = "MGMT_VRF_CONFIG";

/// EVPN NVO table in CONFIG_DB
pub const CFG_EVPN_NVO_TABLE_NAME: &str = "EVPN_NVO";

// APPL_DB tables
/// VRF table in APPL_DB
pub const APP_VRF_TABLE_NAME: &str = "VRF_TABLE";

/// VNET table in APPL_DB
pub const APP_VNET_TABLE_NAME: &str = "VNET_TABLE";

/// VXLAN VRF table in APPL_DB
pub const APP_VXLAN_VRF_TABLE_NAME: &str = "VXLAN_VRF_TABLE";

// STATE_DB tables
/// VRF table in STATE_DB
pub const STATE_VRF_TABLE_NAME: &str = "VRF_TABLE";

/// VRF object table in STATE_DB
pub const STATE_VRF_OBJECT_TABLE_NAME: &str = "VRF_OBJECT_TABLE";

/// Field names used in CONFIG_DB and APPL_DB
pub mod fields {
    /// VNI (VXLAN Network Identifier) field
    pub const VNI: &str = "vni";

    /// Source VTEP (VXLAN Tunnel Endpoint) field
    pub const SOURCE_VTEP: &str = "source_vtep";

    /// VXLAN tunnel name field
    pub const VXLAN_TUNNEL: &str = "vxlan_tunnel";

    /// Management VRF enabled field
    pub const MGMT_VRF_ENABLED: &str = "mgmtVrfEnabled";

    /// In-band management enabled field
    pub const IN_BAND_MGMT_ENABLED: &str = "in_band_mgmt_enabled";
}
