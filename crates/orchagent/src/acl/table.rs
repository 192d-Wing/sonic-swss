//! ACL table management.
//!
//! An ACL table is a collection of ACL rules that share the same type
//! (match fields, actions, bind points).

use sonic_sai::types::RawSaiObjectId;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use super::rule::AclRule;
use super::table_type::AclTableType;
use super::types::{AclRuleId, AclStage, AclTableId};

/// ACL table configuration from CONFIG_DB.
#[derive(Debug, Clone, Default)]
pub struct AclTableConfig {
    /// Table ID (name).
    pub id: Option<String>,
    /// Table type name (e.g., "L3", "MIRROR").
    pub type_name: Option<String>,
    /// ACL stage.
    pub stage: Option<AclStage>,
    /// Ports to bind to.
    pub ports: Vec<String>,
    /// Description.
    pub description: Option<String>,
}

impl AclTableConfig {
    /// Creates a new empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the table ID.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the table type name.
    pub fn with_type(mut self, type_name: impl Into<String>) -> Self {
        self.type_name = Some(type_name.into());
        self
    }

    /// Sets the stage.
    pub fn with_stage(mut self, stage: AclStage) -> Self {
        self.stage = Some(stage);
        self
    }

    /// Sets the ports.
    pub fn with_ports(mut self, ports: Vec<String>) -> Self {
        self.ports = ports;
        self
    }

    /// Parses a field from CONFIG_DB.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        match field.to_uppercase().as_str() {
            "TYPE" => {
                self.type_name = Some(value.to_string());
            }
            "STAGE" => {
                self.stage = Some(value.parse()?);
            }
            "PORTS" | "PORTS@" => {
                self.ports = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "POLICY_DESC" | "DESCRIPTION" => {
                self.description = Some(value.to_string());
            }
            _ => {
                // Ignore unknown fields for forward compatibility
            }
        }
        Ok(())
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_none() {
            return Err("Table ID is required".to_string());
        }
        if self.type_name.is_none() {
            return Err("Table type is required".to_string());
        }
        if self.stage.is_none() {
            return Err("Table stage is required".to_string());
        }
        Ok(())
    }
}

/// Port binding state for an ACL table.
#[derive(Debug, Clone)]
pub struct PortBinding {
    /// Port OID.
    pub port_oid: RawSaiObjectId,
    /// ACL group member OID (for this port-table binding).
    pub group_member_oid: RawSaiObjectId,
}

/// ACL table structure.
///
/// Represents an ACL table in the switch. Tables contain rules that share
/// the same type (match fields, actions) and can be bound to ports.
#[derive(Debug, Clone)]
pub struct AclTable {
    /// Table ID (unique name).
    pub id: AclTableId,
    /// Table type (defines supported matches/actions).
    pub table_type: Arc<AclTableType>,
    /// ACL stage (ingress/egress).
    pub stage: AclStage,
    /// Description.
    pub description: String,
    /// SAI table object ID.
    pub table_oid: RawSaiObjectId,
    /// Port bindings: port alias → binding info.
    pub port_bindings: HashMap<String, PortBinding>,
    /// Ports in configuration (may not be bound yet).
    pub configured_ports: HashSet<String>,
    /// Pending ports (configured but port doesn't exist yet).
    pub pending_ports: HashSet<String>,
    /// Rules in this table: rule ID → rule.
    pub rules: HashMap<AclRuleId, AclRule>,
    /// Whether to bind to the switch (for PFCWD-style tables).
    pub bind_to_switch: bool,
}

impl AclTable {
    /// Creates a new ACL table.
    pub fn new(id: impl Into<String>, table_type: Arc<AclTableType>, stage: AclStage) -> Self {
        Self {
            id: id.into(),
            table_type,
            stage,
            description: String::new(),
            table_oid: 0,
            port_bindings: HashMap::new(),
            configured_ports: HashSet::new(),
            pending_ports: HashSet::new(),
            rules: HashMap::new(),
            bind_to_switch: false,
        }
    }

    /// Creates a table from a configuration.
    pub fn from_config(
        config: &AclTableConfig,
        table_type: Arc<AclTableType>,
    ) -> Result<Self, String> {
        config.validate()?;

        let id = config.id.clone().unwrap();
        let stage = config.stage.unwrap();

        // Validate stage is supported
        if !table_type.supports_stage(stage) {
            return Err(format!(
                "Table type {} does not support stage {}",
                table_type.name, stage
            ));
        }

        let mut table = Self::new(id, table_type, stage);

        if let Some(ref desc) = config.description {
            table.description = desc.clone();
        }

        // Add configured ports (will be resolved to OIDs later)
        for port in &config.ports {
            table.configured_ports.insert(port.clone());
            table.pending_ports.insert(port.clone());
        }

        Ok(table)
    }

    /// Returns the SAI table OID.
    pub fn sai_id(&self) -> RawSaiObjectId {
        self.table_oid
    }

    /// Returns true if the table is created in SAI.
    pub fn is_created(&self) -> bool {
        self.table_oid != 0
    }

    /// Returns the number of rules in the table.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Returns true if the table has no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Returns true if a port is bound to this table.
    pub fn is_port_bound(&self, port_alias: &str) -> bool {
        self.port_bindings.contains_key(port_alias)
    }

    /// Returns true if a port is configured (but may not be bound).
    pub fn is_port_configured(&self, port_alias: &str) -> bool {
        self.configured_ports.contains(port_alias)
    }

    /// Returns true if a port is pending (configured but not bound).
    pub fn is_port_pending(&self, port_alias: &str) -> bool {
        self.pending_ports.contains(port_alias)
    }

    /// Gets a rule by ID.
    pub fn get_rule(&self, rule_id: &str) -> Option<&AclRule> {
        self.rules.get(rule_id)
    }

    /// Gets a mutable rule by ID.
    pub fn get_rule_mut(&mut self, rule_id: &str) -> Option<&mut AclRule> {
        self.rules.get_mut(rule_id)
    }

    /// Adds a rule to the table.
    ///
    /// Returns an error if a rule with the same ID already exists.
    pub fn add_rule(&mut self, rule: AclRule) -> Result<(), String> {
        // Validate rule matches are supported by table type
        self.table_type.validate_matches(&rule.match_fields())?;
        self.table_type.validate_actions(&rule.action_types())?;

        if self.rules.contains_key(&rule.id) {
            return Err(format!(
                "Rule {} already exists in table {}",
                rule.id, self.id
            ));
        }

        self.rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    /// Removes a rule from the table.
    ///
    /// Returns the removed rule, or None if not found.
    pub fn remove_rule(&mut self, rule_id: &str) -> Option<AclRule> {
        self.rules.remove(rule_id)
    }

    /// Updates a rule in the table.
    ///
    /// Returns an error if the rule doesn't exist.
    pub fn update_rule(&mut self, rule: AclRule) -> Result<AclRule, String> {
        if !self.rules.contains_key(&rule.id) {
            return Err(format!("Rule {} not found in table {}", rule.id, self.id));
        }

        // Validate rule matches are supported by table type
        self.table_type.validate_matches(&rule.match_fields())?;
        self.table_type.validate_actions(&rule.action_types())?;

        let old_rule = self.rules.insert(rule.id.clone(), rule);
        Ok(old_rule.unwrap())
    }

    /// Clears all rules from the table.
    pub fn clear_rules(&mut self) {
        self.rules.clear();
    }

    /// Binds a port to this table.
    ///
    /// Records the binding info. The actual SAI binding should be done
    /// by the orchestrator.
    pub fn bind_port(
        &mut self,
        port_alias: &str,
        port_oid: RawSaiObjectId,
        group_member_oid: RawSaiObjectId,
    ) {
        self.port_bindings.insert(
            port_alias.to_string(),
            PortBinding {
                port_oid,
                group_member_oid,
            },
        );
        self.pending_ports.remove(port_alias);
    }

    /// Unbinds a port from this table.
    ///
    /// Returns the binding info, or None if not bound.
    pub fn unbind_port(&mut self, port_alias: &str) -> Option<PortBinding> {
        let binding = self.port_bindings.remove(port_alias);
        if binding.is_some() && self.configured_ports.contains(port_alias) {
            self.pending_ports.insert(port_alias.to_string());
        }
        binding
    }

    /// Adds a port to the configuration.
    pub fn add_configured_port(&mut self, port_alias: &str) {
        self.configured_ports.insert(port_alias.to_string());
        if !self.port_bindings.contains_key(port_alias) {
            self.pending_ports.insert(port_alias.to_string());
        }
    }

    /// Removes a port from the configuration.
    pub fn remove_configured_port(&mut self, port_alias: &str) {
        self.configured_ports.remove(port_alias);
        self.pending_ports.remove(port_alias);
    }

    /// Returns the binding info for a port.
    pub fn get_port_binding(&self, port_alias: &str) -> Option<&PortBinding> {
        self.port_bindings.get(port_alias)
    }

    /// Returns all bound port aliases.
    pub fn bound_ports(&self) -> Vec<String> {
        self.port_bindings.keys().cloned().collect()
    }

    /// Returns all pending port aliases.
    pub fn pending_ports_list(&self) -> Vec<String> {
        self.pending_ports.iter().cloned().collect()
    }

    /// Returns all rule IDs.
    pub fn rule_ids(&self) -> Vec<String> {
        self.rules.keys().cloned().collect()
    }

    /// Updates ports from a new configuration.
    ///
    /// Returns (ports_to_add, ports_to_remove).
    pub fn update_ports(&mut self, new_ports: &HashSet<String>) -> (Vec<String>, Vec<String>) {
        let add: Vec<String> = new_ports
            .difference(&self.configured_ports)
            .cloned()
            .collect();
        let remove: Vec<String> = self
            .configured_ports
            .difference(new_ports)
            .cloned()
            .collect();

        // Update configured and pending sets
        for port in &add {
            self.configured_ports.insert(port.clone());
            if !self.port_bindings.contains_key(port) {
                self.pending_ports.insert(port.clone());
            }
        }
        for port in &remove {
            self.configured_ports.remove(port);
            self.pending_ports.remove(port);
        }

        (add, remove)
    }
}

impl fmt::Display for AclTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AclTable({}, type={}, stage={}, rules={}, bound_ports={})",
            self.id,
            self.table_type.name,
            self.stage,
            self.rules.len(),
            self.port_bindings.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::rule::{AclRuleAction, AclRuleMatch};
    use super::super::table_type::create_l3_table_type;
    use super::*;

    fn test_table_type() -> Arc<AclTableType> {
        Arc::new(create_l3_table_type())
    }

    #[test]
    fn test_table_config() {
        let mut config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress);

        config.parse_field("PORTS", "Ethernet0,Ethernet4").unwrap();
        config
            .parse_field("POLICY_DESC", "Test description")
            .unwrap();

        assert_eq!(config.id, Some("TestTable".to_string()));
        assert_eq!(config.type_name, Some("L3".to_string()));
        assert_eq!(config.stage, Some(AclStage::Ingress));
        assert_eq!(config.ports, vec!["Ethernet0", "Ethernet4"]);
        assert_eq!(config.description, Some("Test description".to_string()));
    }

    #[test]
    fn test_table_config_validate() {
        let config = AclTableConfig::new();
        assert!(config.validate().is_err()); // Missing all required fields

        let config = AclTableConfig::new()
            .with_id("Test")
            .with_type("L3")
            .with_stage(AclStage::Ingress);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_table_new() {
        let table = AclTable::new("test_table", test_table_type(), AclStage::Ingress);

        assert_eq!(table.id, "test_table");
        assert_eq!(table.stage, AclStage::Ingress);
        assert!(!table.is_created());
        assert!(table.is_empty());
    }

    #[test]
    fn test_table_from_config() {
        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress)
            .with_ports(vec!["Ethernet0".to_string(), "Ethernet4".to_string()]);

        let table = AclTable::from_config(&config, test_table_type()).unwrap();

        assert_eq!(table.id, "TestTable");
        assert!(table.is_port_configured("Ethernet0"));
        assert!(table.is_port_pending("Ethernet0"));
    }

    #[test]
    fn test_table_add_rule() {
        let mut table = AclTable::new("test_table", test_table_type(), AclStage::Ingress);

        let rule = AclRule::packet("rule1")
            .with_priority(100)
            .with_match(AclRuleMatch::ip_protocol(6))
            .with_action(AclRuleAction::drop());

        table.add_rule(rule).unwrap();
        assert_eq!(table.rule_count(), 1);
        assert!(table.get_rule("rule1").is_some());
    }

    #[test]
    fn test_table_add_duplicate_rule() {
        let mut table = AclTable::new("test_table", test_table_type(), AclStage::Ingress);

        let rule1 = AclRule::packet("rule1").with_priority(100);
        let rule2 = AclRule::packet("rule1").with_priority(200);

        table.add_rule(rule1).unwrap();
        assert!(table.add_rule(rule2).is_err()); // Duplicate ID
    }

    #[test]
    fn test_table_remove_rule() {
        let mut table = AclTable::new("test_table", test_table_type(), AclStage::Ingress);

        let rule = AclRule::packet("rule1").with_priority(100);
        table.add_rule(rule).unwrap();

        let removed = table.remove_rule("rule1");
        assert!(removed.is_some());
        assert!(table.is_empty());

        let removed = table.remove_rule("nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_table_port_binding() {
        let mut table = AclTable::new("test_table", test_table_type(), AclStage::Ingress);
        table.add_configured_port("Ethernet0");

        assert!(table.is_port_configured("Ethernet0"));
        assert!(table.is_port_pending("Ethernet0"));
        assert!(!table.is_port_bound("Ethernet0"));

        // Bind the port
        table.bind_port("Ethernet0", 0x1234, 0x5678);

        assert!(table.is_port_bound("Ethernet0"));
        assert!(!table.is_port_pending("Ethernet0"));

        let binding = table.get_port_binding("Ethernet0").unwrap();
        assert_eq!(binding.port_oid, 0x1234);
        assert_eq!(binding.group_member_oid, 0x5678);

        // Unbind the port
        let binding = table.unbind_port("Ethernet0");
        assert!(binding.is_some());
        assert!(!table.is_port_bound("Ethernet0"));
        assert!(table.is_port_pending("Ethernet0")); // Back to pending
    }

    #[test]
    fn test_table_update_ports() {
        let mut table = AclTable::new("test_table", test_table_type(), AclStage::Ingress);
        table.add_configured_port("Ethernet0");
        table.add_configured_port("Ethernet4");

        let new_ports: HashSet<_> = ["Ethernet4", "Ethernet8"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let (add, remove) = table.update_ports(&new_ports);

        assert_eq!(add, vec!["Ethernet8"]);
        assert!(remove.contains(&"Ethernet0".to_string()));

        assert!(table.is_port_configured("Ethernet4"));
        assert!(table.is_port_configured("Ethernet8"));
        assert!(!table.is_port_configured("Ethernet0"));
    }

    #[test]
    fn test_table_display() {
        let table = AclTable::new("test_table", test_table_type(), AclStage::Ingress);
        let display = table.to_string();
        assert!(display.contains("test_table"));
        assert!(display.contains("L3"));
        assert!(display.contains("INGRESS"));
    }
}
