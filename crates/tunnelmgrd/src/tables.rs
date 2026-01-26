//! Table and field name constants for tunnelmgrd

// CONFIG_DB tables
pub const CFG_TUNNEL_TABLE: &str = "TUNNEL";
pub const CFG_LOOPBACK_INTERFACE_TABLE: &str = "LOOPBACK_INTERFACE";
pub const CFG_PEER_SWITCH_TABLE: &str = "PEER_SWITCH";

// APPL_DB tables (producer)
pub const APP_TUNNEL_DECAP_TABLE: &str = "TUNNEL_DECAP_TABLE";
pub const APP_TUNNEL_DECAP_TERM_TABLE: &str = "TUNNEL_DECAP_TERM_TABLE";

// APPL_DB tables (consumer)
pub const APP_TUNNEL_ROUTE_TABLE: &str = "APP_TUNNEL_ROUTE_TABLE";

/// PEER_SWITCH table fields
pub mod peer_fields {
    pub const ADDRESS_IPV4: &str = "address_ipv4";
}

/// TUNNEL table fields
pub mod tunnel_fields {
    pub const DST_IP: &str = "dst_ip";
    pub const SRC_IP: &str = "src_ip";
    pub const TUNNEL_TYPE: &str = "tunnel_type";
}

/// TUNNEL_DECAP_TERM table fields
pub mod decap_term_fields {
    pub const SRC_IP: &str = "src_ip";
    pub const TERM_TYPE: &str = "term_type";
    pub const TERM_TYPE_P2P: &str = "P2P";
    pub const TERM_TYPE_P2MP: &str = "P2MP";
}
