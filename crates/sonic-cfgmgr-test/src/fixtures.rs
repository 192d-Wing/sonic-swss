//! Test fixtures for common cfgmgr patterns
//!
//! Provides reusable test scenarios for configuration manager testing

use std::collections::HashMap;

/// Configuration change operation type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigOp {
    /// SET operation (add or update)
    Set,
    /// DEL operation (delete)
    Del,
}

/// Represents a CONFIG_DB change event
#[derive(Debug, Clone)]
pub struct ConfigChange {
    /// Table name (e.g., "PORT", "SFLOW", "VLAN")
    pub table: String,
    /// Key within the table
    pub key: String,
    /// Operation type
    pub op: ConfigOp,
    /// Field-value pairs (for SET operations)
    pub fields: HashMap<String, String>,
}

impl ConfigChange {
    /// Create a SET operation
    pub fn set(table: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            key: key.into(),
            op: ConfigOp::Set,
            fields: HashMap::new(),
        }
    }

    /// Create a DEL operation
    pub fn del(table: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            key: key.into(),
            op: ConfigOp::Del,
            fields: HashMap::new(),
        }
    }

    /// Add a field to a SET operation
    pub fn with_field(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(field.into(), value.into());
        self
    }

    /// Add multiple fields to a SET operation
    pub fn with_fields<I, K, V>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (k, v) in fields {
            self.fields.insert(k.into(), v.into());
        }
        self
    }

    /// Get the Redis key for CONFIG_DB
    pub fn config_db_key(&self) -> String {
        format!("{}:{}", self.table, self.key)
    }
}

/// Common port configuration fixtures
pub mod port_fixtures {
    use super::*;

    /// Standard Ethernet port with default configuration
    pub fn ethernet_port_default(port_name: &str) -> ConfigChange {
        ConfigChange::set("PORT", port_name)
            .with_field("mtu", "9100")
            .with_field("admin_status", "up")
            .with_field("speed", "100000")
    }

    /// Port with custom MTU
    pub fn ethernet_port_custom_mtu(port_name: &str, mtu: &str) -> ConfigChange {
        ConfigChange::set("PORT", port_name)
            .with_field("mtu", mtu)
            .with_field("admin_status", "up")
    }

    /// Port admin down
    pub fn ethernet_port_admin_down(port_name: &str) -> ConfigChange {
        ConfigChange::set("PORT", port_name).with_field("admin_status", "down")
    }

    /// Delete port
    pub fn delete_port(port_name: &str) -> ConfigChange {
        ConfigChange::del("PORT", port_name)
    }
}

/// Common sFlow configuration fixtures
pub mod sflow_fixtures {
    use super::*;

    /// Global sFlow configuration
    pub fn sflow_global() -> ConfigChange {
        ConfigChange::set("SFLOW", "global")
            .with_field("admin_state", "up")
            .with_field("polling_interval", "20")
    }

    /// Global sFlow disabled
    pub fn sflow_global_disabled() -> ConfigChange {
        ConfigChange::set("SFLOW", "global").with_field("admin_state", "down")
    }

    /// Per-interface sFlow configuration
    pub fn sflow_interface(interface: &str, sample_rate: &str) -> ConfigChange {
        ConfigChange::set("SFLOW_SESSION", interface)
            .with_field("admin_state", "up")
            .with_field("sample_rate", sample_rate)
    }

    /// sFlow on all interfaces
    pub fn sflow_all_interfaces(sample_rate: &str) -> ConfigChange {
        ConfigChange::set("SFLOW_SESSION", "all")
            .with_field("admin_state", "up")
            .with_field("sample_rate", sample_rate)
    }
}

/// Common fabric configuration fixtures
pub mod fabric_fixtures {
    use super::*;

    /// Fabric monitoring configuration
    pub fn fabric_monitor_data() -> ConfigChange {
        ConfigChange::set("FABRIC_MONITOR", "FABRIC_MONITOR_DATA")
            .with_field("monState", "enable")
            .with_field("monErrThreshCrcCells", "1000")
            .with_field("monErrThreshRxCells", "2000")
            .with_field("monPollThreshRecovery", "8")
            .with_field("monPollThreshIsolation", "1")
    }

    /// Fabric port configuration
    pub fn fabric_port(port_name: &str) -> ConfigChange {
        ConfigChange::set("FABRIC_PORT", port_name)
            .with_field("alias", port_name)
            .with_field("lanes", "0,1,2,3")
            .with_field("isolateStatus", "False")
    }
}

/// Common VLAN configuration fixtures (for future vlanmgrd testing)
pub mod vlan_fixtures {
    use super::*;

    /// Create VLAN
    pub fn vlan(vlan_id: u16) -> ConfigChange {
        ConfigChange::set("VLAN", format!("Vlan{}", vlan_id))
            .with_field("vlanid", vlan_id.to_string())
    }

    /// Add member to VLAN
    pub fn vlan_member(vlan_id: u16, port: &str, tagging_mode: &str) -> ConfigChange {
        ConfigChange::set("VLAN_MEMBER", format!("Vlan{}|{}", vlan_id, port))
            .with_field("tagging_mode", tagging_mode)
    }

    /// Delete VLAN
    pub fn delete_vlan(vlan_id: u16) -> ConfigChange {
        ConfigChange::del("VLAN", format!("Vlan{}", vlan_id))
    }
}

/// Test scenario builder for complex multi-step tests
#[derive(Debug)]
pub struct TestScenario {
    /// Scenario name
    pub name: String,
    /// Sequence of configuration changes
    pub changes: Vec<ConfigChange>,
    /// Expected APPL_DB state after changes
    pub expected_app_db: HashMap<String, HashMap<String, String>>,
}

impl TestScenario {
    /// Create a new test scenario
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            changes: Vec::new(),
            expected_app_db: HashMap::new(),
        }
    }

    /// Add a configuration change to the scenario
    pub fn add_change(mut self, change: ConfigChange) -> Self {
        self.changes.push(change);
        self
    }

    /// Add an expected APPL_DB entry
    pub fn expect_app_db_entry(
        mut self,
        table_key: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let table_key = table_key.into();
        let field = field.into();
        let value = value.into();

        self.expected_app_db
            .entry(table_key)
            .or_default()
            .insert(field, value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_change_set() {
        let change = ConfigChange::set("PORT", "Ethernet0")
            .with_field("mtu", "9100")
            .with_field("admin_status", "up");

        assert_eq!(change.op, ConfigOp::Set);
        assert_eq!(change.table, "PORT");
        assert_eq!(change.key, "Ethernet0");
        assert_eq!(change.fields.len(), 2);
        assert_eq!(change.fields.get("mtu"), Some(&"9100".to_string()));
    }

    #[test]
    fn test_config_change_del() {
        let change = ConfigChange::del("PORT", "Ethernet0");

        assert_eq!(change.op, ConfigOp::Del);
        assert_eq!(change.table, "PORT");
        assert_eq!(change.key, "Ethernet0");
        assert!(change.fields.is_empty());
    }

    #[test]
    fn test_config_db_key() {
        let change = ConfigChange::set("PORT", "Ethernet0");
        assert_eq!(change.config_db_key(), "PORT:Ethernet0");
    }

    #[test]
    fn test_port_fixtures() {
        let port = port_fixtures::ethernet_port_default("Ethernet0");
        assert_eq!(port.fields.get("mtu"), Some(&"9100".to_string()));

        let custom_mtu = port_fixtures::ethernet_port_custom_mtu("Ethernet4", "1500");
        assert_eq!(custom_mtu.fields.get("mtu"), Some(&"1500".to_string()));

        let admin_down = port_fixtures::ethernet_port_admin_down("Ethernet8");
        assert_eq!(
            admin_down.fields.get("admin_status"),
            Some(&"down".to_string())
        );
    }

    #[test]
    fn test_sflow_fixtures() {
        let global = sflow_fixtures::sflow_global();
        assert_eq!(global.fields.get("admin_state"), Some(&"up".to_string()));

        let intf = sflow_fixtures::sflow_interface("Ethernet0", "4000");
        assert_eq!(intf.fields.get("sample_rate"), Some(&"4000".to_string()));
    }

    #[test]
    fn test_test_scenario() {
        let scenario = TestScenario::new("Port MTU change")
            .add_change(port_fixtures::ethernet_port_custom_mtu("Ethernet0", "1500"))
            .expect_app_db_entry("PORT_TABLE:Ethernet0", "mtu", "1500");

        assert_eq!(scenario.name, "Port MTU change");
        assert_eq!(scenario.changes.len(), 1);
        assert_eq!(scenario.expected_app_db.len(), 1);
    }
}
