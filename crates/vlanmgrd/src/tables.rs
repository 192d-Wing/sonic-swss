//! Table name constants for vlanmgrd

/// CONFIG_DB VLAN table name
pub const CFG_VLAN_TABLE_NAME: &str = "VLAN";

/// CONFIG_DB VLAN_MEMBER table name
pub const CFG_VLAN_MEMBER_TABLE_NAME: &str = "VLAN_MEMBER";

/// APPL_DB VLAN table name
pub const APP_VLAN_TABLE_NAME: &str = "VLAN_TABLE";

/// APPL_DB VLAN_MEMBER table name
pub const APP_VLAN_MEMBER_TABLE_NAME: &str = "VLAN_MEMBER_TABLE";

/// APPL_DB FDB table name
pub const APP_FDB_TABLE_NAME: &str = "FDB_TABLE";

/// APPL_DB PORT table name
pub const APP_PORT_TABLE_NAME: &str = "PORT_TABLE";

/// STATE_DB PORT table name
pub const STATE_PORT_TABLE_NAME: &str = "PORT_TABLE";

/// STATE_DB LAG table name
pub const STATE_LAG_TABLE_NAME: &str = "LAG_TABLE";

/// STATE_DB VLAN table name
pub const STATE_VLAN_TABLE_NAME: &str = "VLAN_TABLE";

/// STATE_DB VLAN_MEMBER table name
pub const STATE_VLAN_MEMBER_TABLE_NAME: &str = "VLAN_MEMBER_TABLE";

/// Field names
pub mod fields {
    /// VLAN ID field
    pub const VLAN_ID: &str = "vlanid";

    /// Admin status field
    pub const ADMIN_STATUS: &str = "admin_status";

    /// MTU field
    pub const MTU: &str = "mtu";

    /// MAC address field
    pub const MAC: &str = "mac";

    /// Tagging mode field
    pub const TAGGING_MODE: &str = "tagging_mode";

    /// Untagged members field
    pub const UNTAGGED_MEMBERS: &str = "untagged_members";
}
