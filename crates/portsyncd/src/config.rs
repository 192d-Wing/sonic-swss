//! Port configuration loading from CONFIG_DB
//!
//! Reads port configuration from SONiC CONFIG_DB and writes to APP_DB.
//! Handles port attributes like lanes, speed, MTU, and admin status.

use crate::error::{PortsyncError, Result};

/// Port configuration loaded from CONFIG_DB
#[derive(Clone, Debug)]
pub struct PortConfig {
    /// Port name (e.g., "Ethernet0")
    pub name: String,
    /// Lanes assigned to port
    pub lanes: Option<String>,
    /// Port speed
    pub speed: Option<String>,
    /// Port alias
    pub alias: Option<String>,
    /// Administrative status
    pub admin_status: Option<String>,
    /// Maximum transmission unit
    pub mtu: Option<String>,
    /// Port description
    pub description: Option<String>,
}

impl PortConfig {
    /// Create a new port configuration
    pub fn new(name: String) -> Self {
        Self {
            name,
            lanes: None,
            speed: None,
            alias: None,
            admin_status: None,
            mtu: None,
            description: None,
        }
    }

    /// Validate port configuration
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(PortsyncError::PortValidation(
                "Port name cannot be empty".to_string(),
            ));
        }

        if let Some(lanes) = &self.lanes {
            if lanes.is_empty() {
                return Err(PortsyncError::PortValidation(
                    "Lanes field cannot be empty".to_string(),
                ));
            }
        }

        if let Some(mtu) = &self.mtu {
            if mtu.parse::<u32>().is_err() {
                return Err(PortsyncError::PortValidation(
                    format!("Invalid MTU value: {}", mtu),
                ));
            }
        }

        Ok(())
    }

    /// Convert to field-value tuples for database storage
    pub fn to_field_values(&self) -> Vec<(String, String)> {
        let mut fields = Vec::new();

        if let Some(lanes) = &self.lanes {
            fields.push(("lanes".to_string(), lanes.clone()));
        }
        if let Some(speed) = &self.speed {
            fields.push(("speed".to_string(), speed.clone()));
        }
        if let Some(alias) = &self.alias {
            fields.push(("alias".to_string(), alias.clone()));
        }
        if let Some(admin_status) = &self.admin_status {
            fields.push(("admin_status".to_string(), admin_status.clone()));
        }
        if let Some(mtu) = &self.mtu {
            fields.push(("mtu".to_string(), mtu.clone()));
        }
        if let Some(description) = &self.description {
            fields.push(("description".to_string(), description.clone()));
        }

        fields
    }
}

/// Load port configuration from CONFIG_DB
/// (Stub - will be implemented in Day 2)
pub async fn load_port_config(
    _warm_restart: bool,
) -> Result<Vec<PortConfig>> {
    // TODO: Implement CONFIG_DB reading
    Ok(Vec::new())
}

/// Validate port configuration
pub fn validate_port_config(port: &PortConfig) -> Result<()> {
    port.validate()
}

/// Send PortConfigDone signal to APP_DB
/// (Stub - will be implemented in Day 2)
pub async fn send_port_config_done() -> Result<()> {
    // TODO: Implement APP_DB signal
    Ok(())
}

/// Send PortInitDone signal to APP_DB
/// (Stub - will be implemented in Day 2)
pub async fn send_port_init_done() -> Result<()> {
    // TODO: Implement APP_DB signal
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_config_creation() {
        let cfg = PortConfig::new("Ethernet0".to_string());
        assert_eq!(cfg.name, "Ethernet0");
        assert!(cfg.lanes.is_none());
    }

    #[test]
    fn test_port_config_validation_valid() {
        let mut cfg = PortConfig::new("Ethernet0".to_string());
        cfg.lanes = Some("4".to_string());
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_port_config_validation_empty_name() {
        let cfg = PortConfig::new("".to_string());
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_port_config_validation_invalid_mtu() {
        let mut cfg = PortConfig::new("Ethernet0".to_string());
        cfg.mtu = Some("invalid".to_string());
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_port_config_to_field_values() {
        let mut cfg = PortConfig::new("Ethernet0".to_string());
        cfg.lanes = Some("4".to_string());
        cfg.mtu = Some("9100".to_string());

        let fields = cfg.to_field_values();
        assert!(!fields.is_empty());
        assert!(fields.iter().any(|(k, _)| k == "lanes"));
        assert!(fields.iter().any(|(k, _)| k == "mtu"));
    }

    #[test]
    fn test_port_config_valid_mtu() {
        let mut cfg = PortConfig::new("Ethernet0".to_string());
        cfg.mtu = Some("9100".to_string());
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_port_config_multiple_attributes() {
        let mut cfg = PortConfig::new("Ethernet0".to_string());
        cfg.lanes = Some("4".to_string());
        cfg.speed = Some("100G".to_string());
        cfg.mtu = Some("9100".to_string());
        cfg.admin_status = Some("up".to_string());

        assert!(cfg.validate().is_ok());
        let fields = cfg.to_field_values();
        assert_eq!(fields.len(), 4);
    }
}
