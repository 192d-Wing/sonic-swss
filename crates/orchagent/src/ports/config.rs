//! Port configuration parsing from Redis/CONFIG_DB.
//!
//! This module handles parsing port configuration from CONFIG_DB field-value pairs
//! into strongly-typed Rust structures.

use std::fmt;
use std::str::FromStr;

use super::port::{
    Port, PortAdminState, PortAutoNegMode, PortFecMode, PortInterfaceType, PortLinkTrainingMode,
    PortRole,
};

/// Error type for port configuration parsing.
#[derive(Debug, Clone)]
pub struct PortConfigError {
    pub field: String,
    pub message: String,
}

impl PortConfigError {
    /// Creates a new configuration error.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Creates an error for an invalid value.
    pub fn invalid_value(field: impl Into<String>, value: impl fmt::Display) -> Self {
        let field = field.into();
        Self::new(&field, format!("Invalid value: {}", value))
    }

    /// Creates an error for a missing required field.
    pub fn missing_field(field: impl Into<String>) -> Self {
        let field = field.into();
        Self::new(&field, "Required field is missing")
    }
}

impl fmt::Display for PortConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Port config error [{}]: {}", self.field, self.message)
    }
}

impl std::error::Error for PortConfigError {}

/// Port configuration parsed from CONFIG_DB.
///
/// This struct holds the configuration values before they are applied to a Port.
/// It supports partial configuration - only fields present in CONFIG_DB will be set.
#[derive(Debug, Clone, Default)]
pub struct PortConfig {
    /// Port alias (required).
    pub alias: Option<String>,
    /// Port description.
    pub description: Option<String>,
    /// Port index.
    pub index: Option<u32>,
    /// Physical lanes.
    pub lanes: Option<Vec<u32>>,
    /// Port speed in Mbps.
    pub speed: Option<u32>,
    /// Auto-negotiation mode.
    pub autoneg: Option<PortAutoNegMode>,
    /// Advertised speeds for auto-negotiation.
    pub adv_speeds: Option<Vec<u32>>,
    /// Interface type.
    pub interface_type: Option<PortInterfaceType>,
    /// Advertised interface types.
    pub adv_interface_types: Option<Vec<PortInterfaceType>>,
    /// FEC mode.
    pub fec: Option<PortFecMode>,
    /// MTU.
    pub mtu: Option<u32>,
    /// TPID.
    pub tpid: Option<u16>,
    /// Admin state.
    pub admin_status: Option<PortAdminState>,
    /// Port role.
    pub role: Option<PortRole>,
    /// Link training mode.
    pub link_training: Option<PortLinkTrainingMode>,
    /// PFC asymmetric mode.
    pub pfc_asym: Option<bool>,
    /// Preemphasis values.
    pub preemphasis: Option<Vec<i32>>,
    /// Override unreliable link state.
    pub override_unreliable_los: Option<bool>,
    /// Subport index (for breakout ports).
    pub subport: Option<u32>,
    /// Parent port (for breakout ports).
    pub parent_port: Option<String>,
    /// PT interface ID (for path tracing).
    pub pt_intf_id: Option<u32>,
    /// PT timestamp template.
    pub pt_timestamp_template: Option<String>,
}

impl PortConfig {
    /// Creates a new empty port configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a configuration with required alias.
    pub fn with_alias(alias: impl Into<String>) -> Self {
        Self {
            alias: Some(alias.into()),
            ..Default::default()
        }
    }

    /// Parses a configuration value from a field-value pair.
    ///
    /// This method handles individual field-value pairs from CONFIG_DB.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), PortConfigError> {
        match field.to_lowercase().as_str() {
            "alias" => {
                self.alias = Some(value.to_string());
            }
            "description" => {
                self.description = Some(value.to_string());
            }
            "index" => {
                self.index = Some(parse_u32(field, value)?);
            }
            "lanes" => {
                self.lanes = Some(parse_lanes(value)?);
            }
            "speed" => {
                self.speed = Some(parse_u32(field, value)?);
            }
            "autoneg" => {
                self.autoneg = Some(parse_enum(field, value)?);
            }
            "adv_speeds" => {
                self.adv_speeds = Some(parse_speeds(value)?);
            }
            "interface_type" => {
                self.interface_type = Some(parse_enum(field, value)?);
            }
            "adv_interface_types" => {
                self.adv_interface_types = Some(parse_interface_types(value)?);
            }
            "fec" => {
                self.fec = Some(parse_enum(field, value)?);
            }
            "mtu" => {
                self.mtu = Some(parse_u32(field, value)?);
            }
            "tpid" => {
                self.tpid = Some(parse_tpid(value)?);
            }
            "admin_status" => {
                self.admin_status = Some(parse_enum(field, value)?);
            }
            "role" => {
                self.role = Some(parse_enum(field, value)?);
            }
            "link_training" => {
                self.link_training = Some(parse_enum(field, value)?);
            }
            "pfc_asym" => {
                self.pfc_asym = Some(parse_bool(field, value)?);
            }
            "preemphasis" => {
                self.preemphasis = Some(parse_preemphasis(value)?);
            }
            "override_unreliable_los" => {
                self.override_unreliable_los = Some(parse_bool(field, value)?);
            }
            "subport" => {
                self.subport = Some(parse_u32(field, value)?);
            }
            "parent_port" => {
                self.parent_port = Some(value.to_string());
            }
            "pt_intf_id" => {
                self.pt_intf_id = Some(parse_u32(field, value)?);
            }
            "pt_timestamp_template" => {
                self.pt_timestamp_template = Some(value.to_string());
            }
            // Ignore unknown fields (forward compatibility)
            _ => {}
        }
        Ok(())
    }

    /// Parses multiple field-value pairs.
    pub fn parse_fields<'a>(
        &mut self,
        fields: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<(), PortConfigError> {
        for (field, value) in fields {
            self.parse_field(field, value)?;
        }
        Ok(())
    }

    /// Applies this configuration to a Port.
    ///
    /// Only fields that are Some will be applied.
    pub fn apply_to(&self, port: &mut Port) {
        if let Some(ref alias) = self.alias {
            port.alias = alias.clone();
        }
        if let Some(ref description) = self.description {
            port.description = description.clone();
        }
        if let Some(index) = self.index {
            port.index = index;
        }
        if let Some(ref lanes) = self.lanes {
            port.lanes = lanes.clone();
        }
        if let Some(speed) = self.speed {
            port.speed = speed;
        }
        if let Some(autoneg) = self.autoneg {
            port.autoneg = autoneg;
        }
        if let Some(ref adv_speeds) = self.adv_speeds {
            port.adv_speeds = adv_speeds.clone();
        }
        if let Some(interface_type) = self.interface_type {
            port.interface_type = interface_type;
        }
        if let Some(ref adv_interface_types) = self.adv_interface_types {
            port.adv_interface_types = adv_interface_types.clone();
        }
        if let Some(fec) = self.fec {
            port.fec_mode = fec;
        }
        if let Some(mtu) = self.mtu {
            port.mtu = mtu;
        }
        if let Some(tpid) = self.tpid {
            port.tpid = tpid;
        }
        if let Some(admin_status) = self.admin_status {
            port.admin_state = admin_status;
        }
        if let Some(role) = self.role {
            port.role = role;
        }
        if let Some(link_training) = self.link_training {
            port.link_training = link_training;
        }
        if let Some(pfc_asym) = self.pfc_asym {
            port.pfc_asym = pfc_asym;
        }
    }

    /// Creates a new Port from this configuration.
    ///
    /// Returns an error if required fields (alias, lanes) are missing.
    pub fn into_port(self) -> Result<Port, PortConfigError> {
        let alias = self
            .alias
            .ok_or_else(|| PortConfigError::missing_field("alias"))?;
        let lanes = self
            .lanes
            .ok_or_else(|| PortConfigError::missing_field("lanes"))?;

        let mut port = Port::physical(alias, lanes);

        // Apply optional fields
        if let Some(description) = self.description {
            port.description = description;
        }
        if let Some(index) = self.index {
            port.index = index;
        }
        if let Some(speed) = self.speed {
            port.speed = speed;
        }
        if let Some(autoneg) = self.autoneg {
            port.autoneg = autoneg;
        }
        if let Some(adv_speeds) = self.adv_speeds {
            port.adv_speeds = adv_speeds;
        }
        if let Some(interface_type) = self.interface_type {
            port.interface_type = interface_type;
        }
        if let Some(adv_interface_types) = self.adv_interface_types {
            port.adv_interface_types = adv_interface_types;
        }
        if let Some(fec) = self.fec {
            port.fec_mode = fec;
        }
        if let Some(mtu) = self.mtu {
            port.mtu = mtu;
        }
        if let Some(tpid) = self.tpid {
            port.tpid = tpid;
        }
        if let Some(admin_status) = self.admin_status {
            port.admin_state = admin_status;
        }
        if let Some(role) = self.role {
            port.role = role;
        }
        if let Some(link_training) = self.link_training {
            port.link_training = link_training;
        }
        if let Some(pfc_asym) = self.pfc_asym {
            port.pfc_asym = pfc_asym;
        }

        Ok(port)
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), PortConfigError> {
        // Validate speed if both speed and lanes are present
        if let (Some(speed), Some(ref lanes)) = (self.speed, &self.lanes) {
            let lane_count = lanes.len() as u32;
            if lane_count > 0 {
                let per_lane_speed = speed / lane_count;
                // Valid per-lane speeds: 10G, 25G, 50G, 100G, 200G
                if ![10000, 25000, 50000, 100000, 200000].contains(&per_lane_speed) {
                    // Just a warning, not an error (vendors may support other speeds)
                }
            }
        }

        // Validate MTU range
        if let Some(mtu) = self.mtu {
            if mtu < 68 || mtu > 9216 {
                return Err(PortConfigError::new(
                    "mtu",
                    format!("MTU {} out of range (68-9216)", mtu),
                ));
            }
        }

        Ok(())
    }
}

/// Parses a u32 value.
fn parse_u32(field: &str, value: &str) -> Result<u32, PortConfigError> {
    value
        .parse()
        .map_err(|_| PortConfigError::invalid_value(field, value))
}

/// Parses a boolean value.
fn parse_bool(field: &str, value: &str) -> Result<bool, PortConfigError> {
    match value.to_lowercase().as_str() {
        "on" | "true" | "1" | "yes" | "enabled" => Ok(true),
        "off" | "false" | "0" | "no" | "disabled" => Ok(false),
        _ => Err(PortConfigError::invalid_value(field, value)),
    }
}

/// Parses an enum value.
fn parse_enum<T: FromStr>(field: &str, value: &str) -> Result<T, PortConfigError>
where
    T::Err: fmt::Display,
{
    value
        .parse()
        .map_err(|e| PortConfigError::new(field, format!("{}", e)))
}

/// Parses lane values (comma-separated).
fn parse_lanes(value: &str) -> Result<Vec<u32>, PortConfigError> {
    value
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|_| PortConfigError::invalid_value("lanes", s))
        })
        .collect()
}

/// Parses speed values (comma-separated).
fn parse_speeds(value: &str) -> Result<Vec<u32>, PortConfigError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|_| PortConfigError::invalid_value("adv_speeds", s))
        })
        .collect()
}

/// Parses interface types (comma-separated).
fn parse_interface_types(value: &str) -> Result<Vec<PortInterfaceType>, PortConfigError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|s| {
            s.trim()
                .parse()
                .map_err(|e| PortConfigError::new("adv_interface_types", e))
        })
        .collect()
}

/// Parses TPID value (hex or decimal).
fn parse_tpid(value: &str) -> Result<u16, PortConfigError> {
    let value = value.trim();
    if value.starts_with("0x") || value.starts_with("0X") {
        u16::from_str_radix(&value[2..], 16)
            .map_err(|_| PortConfigError::invalid_value("tpid", value))
    } else {
        value
            .parse()
            .map_err(|_| PortConfigError::invalid_value("tpid", value))
    }
}

/// Parses preemphasis values (comma-separated).
fn parse_preemphasis(value: &str) -> Result<Vec<i32>, PortConfigError> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|s| {
            let s = s.trim();
            if s.starts_with("0x") || s.starts_with("0X") {
                i32::from_str_radix(&s[2..], 16)
                    .map_err(|_| PortConfigError::invalid_value("preemphasis", s))
            } else {
                s.parse()
                    .map_err(|_| PortConfigError::invalid_value("preemphasis", s))
            }
        })
        .collect()
}

/// LAG configuration parsed from CONFIG_DB.
#[derive(Debug, Clone, Default)]
pub struct LagConfig {
    /// LAG alias (e.g., "PortChannel0001").
    pub alias: Option<String>,
    /// MTU.
    pub mtu: Option<u32>,
    /// Admin status.
    pub admin_status: Option<PortAdminState>,
    /// Minimum links required for LAG to be up.
    pub min_links: Option<u32>,
    /// Fallback mode.
    pub fallback: Option<bool>,
    /// LACP fast rate.
    pub fast_rate: Option<bool>,
    /// TPID.
    pub tpid: Option<u16>,
}

impl LagConfig {
    /// Creates a new empty LAG configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a configuration field.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), PortConfigError> {
        match field.to_lowercase().as_str() {
            "alias" => {
                self.alias = Some(value.to_string());
            }
            "mtu" => {
                self.mtu = Some(parse_u32(field, value)?);
            }
            "admin_status" => {
                self.admin_status = Some(parse_enum(field, value)?);
            }
            "min_links" => {
                self.min_links = Some(parse_u32(field, value)?);
            }
            "fallback" => {
                self.fallback = Some(parse_bool(field, value)?);
            }
            "fast_rate" => {
                self.fast_rate = Some(parse_bool(field, value)?);
            }
            "tpid" => {
                self.tpid = Some(parse_tpid(value)?);
            }
            _ => {}
        }
        Ok(())
    }
}

/// VLAN configuration parsed from CONFIG_DB.
#[derive(Debug, Clone, Default)]
pub struct VlanConfig {
    /// VLAN alias (e.g., "Vlan100").
    pub alias: Option<String>,
    /// VLAN ID (1-4094).
    pub vlan_id: Option<u16>,
    /// MTU.
    pub mtu: Option<u32>,
    /// Admin status.
    pub admin_status: Option<PortAdminState>,
    /// MAC address (for SVI).
    pub mac_address: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// DHCP servers.
    pub dhcp_servers: Option<Vec<String>>,
}

impl VlanConfig {
    /// Creates a new empty VLAN configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a configuration field.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), PortConfigError> {
        match field.to_lowercase().as_str() {
            "alias" => {
                self.alias = Some(value.to_string());
            }
            "vlanid" | "vlan_id" => {
                let vlan_id = parse_u32(field, value)?;
                if vlan_id > 4094 {
                    return Err(PortConfigError::new(field, "VLAN ID must be 1-4094"));
                }
                self.vlan_id = Some(vlan_id as u16);
            }
            "mtu" => {
                self.mtu = Some(parse_u32(field, value)?);
            }
            "admin_status" => {
                self.admin_status = Some(parse_enum(field, value)?);
            }
            "mac" | "mac_address" => {
                self.mac_address = Some(value.to_string());
            }
            "description" => {
                self.description = Some(value.to_string());
            }
            "dhcp_servers" => {
                self.dhcp_servers = Some(value.split(',').map(|s| s.trim().to_string()).collect());
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lanes() {
        assert_eq!(parse_lanes("0,1,2,3").unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(parse_lanes("0").unwrap(), vec![0]);
        assert!(parse_lanes("invalid").is_err());
    }

    #[test]
    fn test_parse_tpid() {
        assert_eq!(parse_tpid("0x8100").unwrap(), 0x8100);
        assert_eq!(parse_tpid("0x88a8").unwrap(), 0x88a8);
        assert_eq!(parse_tpid("33024").unwrap(), 0x8100);
    }

    #[test]
    fn test_port_config_parse_field() {
        let mut config = PortConfig::new();

        config.parse_field("alias", "Ethernet0").unwrap();
        config.parse_field("lanes", "0,1,2,3").unwrap();
        config.parse_field("speed", "100000").unwrap();
        config.parse_field("mtu", "9100").unwrap();
        config.parse_field("admin_status", "up").unwrap();
        config.parse_field("fec", "rs").unwrap();

        assert_eq!(config.alias, Some("Ethernet0".to_string()));
        assert_eq!(config.lanes, Some(vec![0, 1, 2, 3]));
        assert_eq!(config.speed, Some(100000));
        assert_eq!(config.mtu, Some(9100));
        assert_eq!(config.admin_status, Some(PortAdminState::Up));
        assert_eq!(config.fec, Some(PortFecMode::Rs));
    }

    #[test]
    fn test_port_config_into_port() {
        let mut config = PortConfig::new();
        config.alias = Some("Ethernet0".to_string());
        config.lanes = Some(vec![0, 1, 2, 3]);
        config.speed = Some(100000);
        config.mtu = Some(9100);

        let port = config.into_port().unwrap();
        assert_eq!(port.alias, "Ethernet0");
        assert_eq!(port.lanes, vec![0, 1, 2, 3]);
        assert_eq!(port.speed, 100000);
        assert_eq!(port.mtu, 9100);
    }

    #[test]
    fn test_port_config_missing_alias() {
        let mut config = PortConfig::new();
        config.lanes = Some(vec![0]);

        let result = config.into_port();
        assert!(result.is_err());
        assert!(result.unwrap_err().field.contains("alias"));
    }

    #[test]
    fn test_port_config_missing_lanes() {
        let mut config = PortConfig::new();
        config.alias = Some("Ethernet0".to_string());

        let result = config.into_port();
        assert!(result.is_err());
        assert!(result.unwrap_err().field.contains("lanes"));
    }

    #[test]
    fn test_port_config_validate_mtu() {
        let mut config = PortConfig::new();
        config.mtu = Some(50); // Too small

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_lag_config() {
        let mut config = LagConfig::new();
        config.parse_field("alias", "PortChannel0001").unwrap();
        config.parse_field("mtu", "9100").unwrap();
        config.parse_field("min_links", "2").unwrap();
        config.parse_field("fallback", "true").unwrap();

        assert_eq!(config.alias, Some("PortChannel0001".to_string()));
        assert_eq!(config.mtu, Some(9100));
        assert_eq!(config.min_links, Some(2));
        assert_eq!(config.fallback, Some(true));
    }

    #[test]
    fn test_vlan_config() {
        let mut config = VlanConfig::new();
        config.parse_field("alias", "Vlan100").unwrap();
        config.parse_field("vlanid", "100").unwrap();
        config.parse_field("mtu", "9100").unwrap();

        assert_eq!(config.alias, Some("Vlan100".to_string()));
        assert_eq!(config.vlan_id, Some(100));
        assert_eq!(config.mtu, Some(9100));
    }

    #[test]
    fn test_vlan_config_invalid_vlan_id() {
        let mut config = VlanConfig::new();
        let result = config.parse_field("vlanid", "5000");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool("test", "on").unwrap());
        assert!(parse_bool("test", "true").unwrap());
        assert!(parse_bool("test", "1").unwrap());
        assert!(!parse_bool("test", "off").unwrap());
        assert!(!parse_bool("test", "false").unwrap());
        assert!(parse_bool("test", "invalid").is_err());
    }
}
