//! Port configuration loading from CONFIG_DB
//!
//! Reads port configuration from SONiC CONFIG_DB and writes to APP_DB.
//! Handles port attributes like lanes, speed, MTU, and admin status.

use crate::error::{PortsyncError, Result};
use std::collections::HashMap;

/// Database connection abstraction (mock for testing)
/// In production, this will use sonic-redis connections
#[derive(Clone, Debug)]
pub struct DatabaseConnection {
    /// Database name (CONFIG_DB, APP_DB, STATE_DB)
    pub db_name: String,
    /// Stored key-value pairs (for testing)
    data: HashMap<String, HashMap<String, String>>,
}

impl DatabaseConnection {
    /// Create new database connection
    pub fn new(db_name: String) -> Self {
        Self {
            db_name,
            data: HashMap::new(),
        }
    }

    /// Get hash values from database
    pub async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>> {
        Ok(self.data.get(key).cloned().unwrap_or_default())
    }

    /// Set hash field values in database
    pub async fn hset(&mut self, key: &str, fields: &[(String, String)]) -> Result<()> {
        let entry = self.data.entry(key.to_string()).or_insert_with(HashMap::new);
        for (field, value) in fields {
            entry.insert(field.clone(), value.clone());
        }
        Ok(())
    }

    /// Delete key from database
    pub async fn delete(&mut self, key: &str) -> Result<()> {
        self.data.remove(key);
        Ok(())
    }

    /// Get all keys matching pattern
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let keys: Vec<_> = self
            .data
            .keys()
            .filter(|k| {
                if pattern == "*" {
                    true
                } else if pattern.ends_with('*') {
                    let prefix = &pattern[..pattern.len() - 1];
                    k.starts_with(prefix)
                } else {
                    k.as_str() == pattern
                }
            })
            .cloned()
            .collect();
        Ok(keys)
    }
}

/// Port configuration loaded from CONFIG_DB
#[derive(Clone, Debug, PartialEq)]
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

    /// Create from database fields
    pub fn from_fields(name: String, fields: &HashMap<String, String>) -> Self {
        let mut config = Self::new(name);
        config.lanes = fields.get("lanes").cloned();
        config.speed = fields.get("speed").cloned();
        config.alias = fields.get("alias").cloned();
        config.admin_status = fields.get("admin_status").cloned();
        config.mtu = fields.get("mtu").cloned();
        config.description = fields.get("description").cloned();
        config
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
pub async fn load_port_config(
    config_db: &DatabaseConnection,
    app_db: &mut DatabaseConnection,
    warm_restart: bool,
) -> Result<Vec<PortConfig>> {
    // Get all PORT table entries from CONFIG_DB
    let port_keys = config_db.keys("PORT|*").await?;

    let mut ports = Vec::new();

    for key in port_keys {
        // Parse port name from key (format: "PORT|Ethernet0")
        let port_name = if let Some(name) = key.strip_prefix("PORT|") {
            name.to_string()
        } else {
            continue;
        };

        // Get port fields from CONFIG_DB
        let fields = config_db.hgetall(&key).await?;
        let port_config = PortConfig::from_fields(port_name.clone(), &fields);

        // Validate port configuration
        port_config.validate()?;

        // Write to APP_DB (skip during warm restart)
        if !warm_restart {
            let app_key = format!("PORT_TABLE|{}", port_config.name);
            let field_values: Vec<(String, String)> = port_config
                .to_field_values()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            app_db.hset(&app_key, &field_values).await?;
        }

        ports.push(port_config);
    }

    Ok(ports)
}

/// Validate port configuration
pub fn validate_port_config(port: &PortConfig) -> Result<()> {
    port.validate()
}

/// Send PortConfigDone signal to APP_DB
pub async fn send_port_config_done(app_db: &mut DatabaseConnection) -> Result<()> {
    // Write PortConfigDone marker to APP_DB
    let fields = vec![("".to_string(), "".to_string())];
    app_db.hset("PortConfigDone", &fields).await?;
    Ok(())
}

/// Send PortInitDone signal to APP_DB
pub async fn send_port_init_done(app_db: &mut DatabaseConnection) -> Result<()> {
    // Write PortInitDone marker with lanes=0 to signal initialization complete
    let fields = vec![("lanes".to_string(), "0".to_string())];
    app_db.hset("PortInitDone", &fields).await?;
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

    #[test]
    fn test_port_config_from_fields() {
        let mut fields = HashMap::new();
        fields.insert("lanes".to_string(), "4".to_string());
        fields.insert("speed".to_string(), "100G".to_string());

        let cfg = PortConfig::from_fields("Ethernet0".to_string(), &fields);
        assert_eq!(cfg.name, "Ethernet0");
        assert_eq!(cfg.lanes, Some("4".to_string()));
        assert_eq!(cfg.speed, Some("100G".to_string()));
    }

    #[tokio::test]
    async fn test_database_connection_creation() {
        let db = DatabaseConnection::new("CONFIG_DB".to_string());
        assert_eq!(db.db_name, "CONFIG_DB");
    }

    #[tokio::test]
    async fn test_database_hset_and_hgetall() {
        let mut db = DatabaseConnection::new("APP_DB".to_string());
        let fields = vec![
            ("lanes".to_string(), "4".to_string()),
            ("speed".to_string(), "100G".to_string()),
        ];

        db.hset("PORT_TABLE|Ethernet0", &fields).await.unwrap();
        let result = db.hgetall("PORT_TABLE|Ethernet0").await.unwrap();

        assert_eq!(result.get("lanes"), Some(&"4".to_string()));
        assert_eq!(result.get("speed"), Some(&"100G".to_string()));
    }

    #[tokio::test]
    async fn test_load_port_config_empty() {
        let config_db = DatabaseConnection::new("CONFIG_DB".to_string());
        let mut app_db = DatabaseConnection::new("APP_DB".to_string());

        let ports = load_port_config(&config_db, &mut app_db, false).await.unwrap();
        assert!(ports.is_empty());
    }

    #[tokio::test]
    async fn test_send_port_config_done() {
        let mut app_db = DatabaseConnection::new("APP_DB".to_string());
        send_port_config_done(&mut app_db).await.unwrap();

        let result = app_db.hgetall("PortConfigDone").await.unwrap();
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_send_port_init_done() {
        let mut app_db = DatabaseConnection::new("APP_DB".to_string());
        send_port_init_done(&mut app_db).await.unwrap();

        let result = app_db.hgetall("PortInitDone").await.unwrap();
        assert_eq!(result.get("lanes"), Some(&"0".to_string()));
    }
}
