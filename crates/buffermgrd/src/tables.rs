//! Table and field name constants for buffermgrd

// CONFIG_DB tables
pub const CFG_PORT_TABLE: &str = "PORT";
pub const CFG_PORT_CABLE_LEN_TABLE: &str = "CABLE_LENGTH";
pub const CFG_PORT_QOS_MAP_TABLE: &str = "PORT_QOS_MAP";
pub const CFG_BUFFER_PROFILE_TABLE: &str = "BUFFER_PROFILE";
pub const CFG_BUFFER_PG_TABLE: &str = "BUFFER_PG";
pub const CFG_BUFFER_POOL_TABLE: &str = "BUFFER_POOL";

// APPL_DB tables
pub const APP_BUFFER_PROFILE_TABLE: &str = "BUFFER_PROFILE_TABLE";
pub const APP_BUFFER_PG_TABLE: &str = "BUFFER_PG_TABLE";
pub const APP_BUFFER_POOL_TABLE: &str = "BUFFER_POOL_TABLE";
pub const APP_BUFFER_QUEUE_TABLE: &str = "BUFFER_QUEUE_TABLE";
pub const APP_BUFFER_PORT_INGRESS_PROFILE_LIST: &str = "BUFFER_PORT_INGRESS_PROFILE_LIST";
pub const APP_BUFFER_PORT_EGRESS_PROFILE_LIST: &str = "BUFFER_PORT_EGRESS_PROFILE_LIST";

/// PORT table fields
pub mod port_fields {
    pub const SPEED: &str = "speed";
    pub const ADMIN_STATUS: &str = "admin_status";
}

/// PORT_QOS_MAP table fields
pub mod qos_map_fields {
    pub const PFC_ENABLE: &str = "pfc_enable";
}

/// BUFFER_PROFILE table fields
pub mod buffer_profile_fields {
    pub const POOL: &str = "pool";
    pub const XON: &str = "xon";
    pub const XON_OFFSET: &str = "xon_offset";
    pub const XOFF: &str = "xoff";
    pub const SIZE: &str = "size";
    pub const DYNAMIC_TH: &str = "dynamic_th";
}

/// BUFFER_PG table fields
pub mod buffer_pg_fields {
    pub const PROFILE: &str = "profile";
}

/// BUFFER_POOL table fields
pub mod buffer_pool_fields {
    pub const MODE: &str = "mode";
}

/// Special keys
pub const PORT_NAME_GLOBAL: &str = "global";
