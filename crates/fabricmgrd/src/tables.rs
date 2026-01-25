//! Table name constants for fabricmgrd

/// CONFIG_DB FABRIC_MONITOR table
pub const CFG_FABRIC_MONITOR_DATA_TABLE_NAME: &str = "FABRIC_MONITOR";

/// CONFIG_DB FABRIC_PORT table
pub const CFG_FABRIC_MONITOR_PORT_TABLE_NAME: &str = "FABRIC_PORT";

/// APPL_DB FABRIC_MONITOR_DATA table
pub const APP_FABRIC_MONITOR_DATA_TABLE_NAME: &str = "FABRIC_MONITOR_DATA";

/// APPL_DB FABRIC_PORT table
pub const APP_FABRIC_MONITOR_PORT_TABLE_NAME: &str = "FABRIC_PORT_TABLE";

/// Special key for fabric monitor data
pub const FABRIC_MONITOR_DATA_KEY: &str = "FABRIC_MONITOR_DATA";

/// Field names used in fabric tables
pub mod fields {
    // Fabric monitoring thresholds
    pub const MON_ERR_THRESH_CRC_CELLS: &str = "monErrThreshCrcCells";
    pub const MON_ERR_THRESH_RX_CELLS: &str = "monErrThreshRxCells";
    pub const MON_POLL_THRESH_RECOVERY: &str = "monPollThreshRecovery";
    pub const MON_POLL_THRESH_ISOLATION: &str = "monPollThreshIsolation";
    pub const MON_STATE: &str = "monState";

    // Fabric port fields
    pub const ALIAS: &str = "alias";
    pub const LANES: &str = "lanes";
    pub const ISOLATE_STATUS: &str = "isolateStatus";
}
