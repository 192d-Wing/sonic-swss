//! Table name constants for portmgrd.
//!
//! These match the schema definitions in swss-common.

/// CONFIG_DB table for port configuration.
pub const CFG_PORT_TABLE_NAME: &str = "PORT";

/// CONFIG_DB table for SendToIngress port configuration.
pub const CFG_SEND_TO_INGRESS_PORT_TABLE_NAME: &str = "SEND_TO_INGRESS_PORT";

/// CONFIG_DB table for LAG member detection.
pub const CFG_LAG_MEMBER_TABLE_NAME: &str = "PORTCHANNEL_MEMBER";

/// STATE_DB table for port state.
pub const STATE_PORT_TABLE_NAME: &str = "PORT_TABLE";

/// APPL_DB table for port configuration (written by portmgrd).
pub const APP_PORT_TABLE_NAME: &str = "PORT_TABLE";

/// APPL_DB table for SendToIngress configuration.
pub const APP_SEND_TO_INGRESS_PORT_TABLE_NAME: &str = "SEND_TO_INGRESS_PORT_TABLE";

/// Field names used in port tables.
pub mod fields {
    /// Port MTU field.
    pub const MTU: &str = "mtu";

    /// Port admin status field (up/down).
    pub const ADMIN_STATUS: &str = "admin_status";

    /// Port state field in STATE_DB.
    pub const STATE: &str = "state";
}
