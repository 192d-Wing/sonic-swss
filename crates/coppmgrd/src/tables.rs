//! Table and field name constants

// CONFIG_DB tables
pub const CFG_COPP_TRAP_TABLE: &str = "COPP_TRAP";
pub const CFG_COPP_GROUP_TABLE: &str = "COPP_GROUP";
pub const CFG_FEATURE_TABLE: &str = "FEATURE";

// APPL_DB tables
pub const APP_COPP_TABLE: &str = "COPP_TABLE";

// STATE_DB tables
pub const STATE_COPP_TRAP_TABLE: &str = "COPP_TRAP_TABLE";
pub const STATE_COPP_GROUP_TABLE: &str = "COPP_GROUP_TABLE";

// COPP_TRAP field names
pub mod trap_fields {
    pub const TRAP_IDS: &str = "trap_ids";
    pub const TRAP_GROUP: &str = "trap_group";
    pub const ALWAYS_ENABLED: &str = "always_enabled";
}

// COPP_GROUP field names
pub mod group_fields {
    pub const TRAP_IDS: &str = "trap_ids"; // Aggregated list
    pub const QUEUE: &str = "queue";
    pub const TRAP_ACTION: &str = "trap_action";
    pub const TRAP_PRIORITY: &str = "trap_priority";
    pub const METER_TYPE: &str = "meter_type";
    pub const MODE: &str = "mode";
    pub const COLOR: &str = "color";
    pub const CBS: &str = "cbs";
    pub const CIR: &str = "cir";
    pub const PBS: &str = "pbs";
    pub const PIR: &str = "pir";
    pub const GREEN_ACTION: &str = "green_action";
    pub const RED_ACTION: &str = "red_action";
    pub const YELLOW_ACTION: &str = "yellow_action";
    pub const GENETLINK_NAME: &str = "genetlink_name";
    pub const GENETLINK_MCGRP_NAME: &str = "genetlink_mcgrp_name";
}

// FEATURE field names
pub mod feature_fields {
    pub const STATE: &str = "state";
}

// STATE field value
pub const STATE_OK: &str = "ok";

// Default CoPP init file path
pub const COPP_INIT_FILE: &str = "/etc/sonic/copp_cfg.json";
