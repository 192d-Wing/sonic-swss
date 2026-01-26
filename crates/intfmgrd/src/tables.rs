//! Table and field name constants

// CONFIG_DB tables
pub const CFG_INTF_TABLE: &str = "INTERFACE";
pub const CFG_VLAN_INTF_TABLE: &str = "VLAN_INTERFACE";
pub const CFG_LAG_INTF_TABLE: &str = "LAG_INTERFACE";
pub const CFG_LOOPBACK_INTF_TABLE: &str = "LOOPBACK_INTERFACE";
pub const CFG_PORTCHANNEL_INTF_TABLE: &str = "PORTCHANNEL_INTERFACE";
pub const CFG_DEVICE_METADATA_TABLE: &str = "DEVICE_METADATA";
pub const CFG_PORT_TABLE: &str = "PORT";

// APPL_DB tables
pub const APP_INTF_TABLE: &str = "INTF_TABLE";
pub const APP_PORT_TABLE: &str = "PORT_TABLE";
pub const APP_NEIGH_TABLE: &str = "NEIGH_TABLE";

// STATE_DB tables
pub const STATE_PORT_TABLE: &str = "PORT_TABLE";
pub const STATE_LAG_TABLE: &str = "LAG_TABLE";
pub const STATE_VLAN_TABLE: &str = "VLAN_TABLE";
pub const STATE_VRF_TABLE: &str = "VRF_TABLE";
pub const STATE_INTF_TABLE: &str = "INTERFACE_TABLE";
pub const STATE_MACSEC_INGRESS_SA_TABLE: &str = "MACSEC_INGRESS_SA_TABLE";

// INTERFACE field names
pub mod intf_fields {
    pub const VRF_NAME: &str = "vrf_name";
    pub const MPLS: &str = "mpls";
    pub const PROXY_ARP: &str = "proxy_arp";
    pub const GRAT_ARP: &str = "grat_arp";
    pub const IPV6_USE_LINK_LOCAL_ONLY: &str = "ipv6_use_link_local_only";
    pub const NAT_ZONE: &str = "nat_zone";
    pub const MAC_ADDR: &str = "mac_addr";
}

// Sub-interface field names
pub mod subintf_fields {
    pub const VLAN: &str = "vlan";
    pub const ADMIN_STATUS: &str = "admin_status";
    pub const MTU: &str = "mtu";
}

// PORT field names
pub mod port_fields {
    pub const ADMIN_STATUS: &str = "admin_status";
    pub const MTU: &str = "mtu";
}

// INTF_TABLE (APPL_DB) field names
pub mod app_intf_fields {
    pub const SCOPE: &str = "scope";
    pub const FAMILY: &str = "family";
}

// STATE field name
pub const STATE_FIELD: &str = "state";
pub const STATE_OK: &str = "ok";

// Shell commands
pub const IP_CMD: &str = "/sbin/ip";
pub const SYSCTL_CMD: &str = "sysctl";

// Interface prefixes
pub const VLAN_PREFIX: &str = "Vlan";
pub const LAG_PREFIX: &str = "PortChannel";
pub const SUBINTF_LAG_PREFIX: &str = "Po";
pub const LOOPBACK_PREFIX: &str = "Loopback";
pub const VNET_PREFIX: &str = "Vnet";
pub const VRF_PREFIX: &str = "Vrf";

// Special values
pub const VRF_MGMT: &str = "mgmt";
pub const MTU_INHERITANCE: &str = "0";
pub const LOOPBACK_DEFAULT_MTU: u32 = 65536;
pub const DEFAULT_MTU: u32 = 9100;
