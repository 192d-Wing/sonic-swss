//! Table name constants for sflowmgrd

/// CONFIG_DB SFLOW table
pub const CFG_SFLOW_TABLE_NAME: &str = "SFLOW";

/// CONFIG_DB SFLOW_SESSION table
pub const CFG_SFLOW_SESSION_TABLE_NAME: &str = "SFLOW_SESSION";

/// CONFIG_DB PORT table (for port speed)
pub const CFG_PORT_TABLE_NAME: &str = "PORT";

/// STATE_DB PORT_TABLE (for operational speed)
pub const STATE_PORT_TABLE_NAME: &str = "PORT_TABLE";

/// APPL_DB SFLOW_TABLE
pub const APP_SFLOW_TABLE_NAME: &str = "SFLOW_TABLE";

/// APPL_DB SFLOW_SESSION_TABLE
pub const APP_SFLOW_SESSION_TABLE_NAME: &str = "SFLOW_SESSION_TABLE";

/// Field names used in sFlow tables
pub mod fields {
    pub const ADMIN_STATE: &str = "admin_state";
    pub const SAMPLE_RATE: &str = "sample_rate";
    pub const SAMPLE_DIRECTION: &str = "sample_direction";
    pub const SPEED: &str = "speed";
}

/// Special constants
pub mod constants {
    /// Error speed indicator
    pub const ERROR_SPEED: &str = "error";

    /// Speed not available indicator
    pub const NA_SPEED: &str = "N/A";

    /// Default sampling direction
    pub const DEFAULT_DIRECTION: &str = "rx";

    /// Default admin state
    pub const DEFAULT_ADMIN_STATE: &str = "up";

    /// Special key for "all interfaces" configuration
    pub const ALL_INTERFACES: &str = "all";
}
