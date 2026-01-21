//! AclOrch - Main ACL orchestrator.
//!
//! The AclOrch manages all ACL tables and rules in the switch. It handles:
//! - Table creation and deletion
//! - Rule creation, update, and deletion
//! - Port binding and unbinding
//! - Integration with dependent orchs (mirror, neighbor, route)

use std::collections::HashMap;
use std::sync::Arc;

use sonic_orch_common::SyncMap;
use sonic_sai::types::RawSaiObjectId;

use super::range::AclRangeCache;
use super::rule::AclRule;
use super::table::{AclTable, AclTableConfig};
use super::table_type::{
    create_ctrlplane_table_type, create_drop_table_type, create_l3_table_type,
    create_l3v6_table_type, create_mirror_table_type, create_pfcwd_table_type, AclTableType,
};
use super::types::{AclPriority, AclStage, AclTableId, MetaDataValue};

/// Error type for AclOrch operations.
#[derive(Debug, Clone)]
pub enum AclOrchError {
    /// Table not found.
    TableNotFound(String),
    /// Rule not found.
    RuleNotFound(String, String),
    /// Table already exists.
    TableAlreadyExists(String),
    /// Rule already exists.
    RuleAlreadyExists(String, String),
    /// Table type not found.
    TableTypeNotFound(String),
    /// Invalid configuration.
    InvalidConfig(String),
    /// SAI error.
    SaiError(String),
    /// Resource exhausted.
    ResourceExhausted(String),
    /// Validation error.
    ValidationError(String),
    /// Dependency error (e.g., mirror session not found).
    DependencyError(String),
}

impl std::fmt::Display for AclOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TableNotFound(id) => write!(f, "ACL table not found: {}", id),
            Self::RuleNotFound(table, rule) => {
                write!(f, "ACL rule {} not found in table {}", rule, table)
            }
            Self::TableAlreadyExists(id) => write!(f, "ACL table already exists: {}", id),
            Self::RuleAlreadyExists(table, rule) => {
                write!(f, "ACL rule {} already exists in table {}", rule, table)
            }
            Self::TableTypeNotFound(name) => write!(f, "ACL table type not found: {}", name),
            Self::InvalidConfig(msg) => write!(f, "Invalid ACL config: {}", msg),
            Self::SaiError(msg) => write!(f, "SAI error: {}", msg),
            Self::ResourceExhausted(msg) => write!(f, "Resource exhausted: {}", msg),
            Self::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            Self::DependencyError(msg) => write!(f, "Dependency error: {}", msg),
        }
    }
}

impl std::error::Error for AclOrchError {}

/// Result type alias for AclOrch operations.
pub type Result<T> = std::result::Result<T, AclOrchError>;

/// Callbacks for AclOrch to interact with other orchs.
#[derive(Clone, Default)]
pub struct AclOrchCallbacks {
    /// Get port OID by alias.
    pub get_port_oid: Option<Arc<dyn Fn(&str) -> Option<RawSaiObjectId> + Send + Sync>>,
    /// Get mirror session OID by name.
    pub get_mirror_session_oid:
        Option<Arc<dyn Fn(&str) -> Option<RawSaiObjectId> + Send + Sync>>,
    /// Increment mirror session reference.
    pub incr_mirror_ref: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Decrement mirror session reference.
    pub decr_mirror_ref: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Get next-hop OID by key.
    pub get_nexthop_oid: Option<Arc<dyn Fn(&str) -> Option<RawSaiObjectId> + Send + Sync>>,
    /// Increment next-hop reference.
    pub incr_nexthop_ref: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Decrement next-hop reference.
    pub decr_nexthop_ref: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

impl std::fmt::Debug for AclOrchCallbacks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AclOrchCallbacks")
            .field("get_port_oid", &self.get_port_oid.is_some())
            .field("get_mirror_session_oid", &self.get_mirror_session_oid.is_some())
            .finish()
    }
}

/// Configuration for AclOrch.
#[derive(Debug, Clone)]
pub struct AclOrchConfig {
    /// Minimum ACL priority.
    pub min_priority: AclPriority,
    /// Maximum ACL priority.
    pub max_priority: AclPriority,
    /// Maximum number of tables.
    pub max_tables: usize,
    /// Whether combined mirror V6 tables are supported.
    pub combined_mirror_v6: bool,
    /// Whether L3V4V6 tables are supported (per stage).
    pub l3v4v6_supported: HashMap<AclStage, bool>,
    /// Whether ACL metadata is supported.
    pub metadata_supported: bool,
    /// Minimum metadata value.
    pub metadata_min: u16,
    /// Maximum metadata value.
    pub metadata_max: u16,
}

impl Default for AclOrchConfig {
    fn default() -> Self {
        Self {
            min_priority: 0,
            max_priority: 999999,
            max_tables: 1024,
            combined_mirror_v6: false,
            l3v4v6_supported: HashMap::new(),
            metadata_supported: true,
            metadata_min: MetaDataValue::MIN,
            metadata_max: MetaDataValue::MAX,
        }
    }
}

/// ACL action capabilities for a stage.
#[derive(Debug, Clone, Default)]
pub struct AclActionCapabilities {
    /// Supported action types.
    pub supported_actions: Vec<super::types::AclActionType>,
    /// Whether action list is mandatory on table creation.
    pub action_list_mandatory: bool,
}

/// Statistics for AclOrch operations.
#[derive(Debug, Clone, Default)]
pub struct AclOrchStats {
    /// Number of tables created.
    pub tables_created: u64,
    /// Number of tables deleted.
    pub tables_deleted: u64,
    /// Number of rules created.
    pub rules_created: u64,
    /// Number of rules deleted.
    pub rules_deleted: u64,
    /// Number of rules updated.
    pub rules_updated: u64,
    /// Number of SAI errors.
    pub sai_errors: u64,
}

/// AclOrch - Main ACL orchestration structure.
///
/// This manages all ACL tables and rules in the switch.
#[derive(Debug)]
pub struct AclOrch {
    /// Configuration.
    config: AclOrchConfig,

    /// Callbacks for interacting with other orchs.
    callbacks: Option<Arc<AclOrchCallbacks>>,

    // ============ Table Type Registry ============
    /// Registered table types: type name → type definition.
    table_types: HashMap<String, Arc<AclTableType>>,

    // ============ Tables ============
    /// ACL tables indexed by table ID.
    tables: SyncMap<AclTableId, AclTable>,

    /// ACL tables indexed by SAI OID (for reverse lookup).
    table_oid_to_id: HashMap<RawSaiObjectId, AclTableId>,

    // ============ Capabilities ============
    /// Action capabilities per stage.
    action_capabilities: HashMap<AclStage, AclActionCapabilities>,

    // ============ Metadata Management ============
    /// Allocated metadata values: value → reference count.
    metadata_refs: HashMap<u16, u32>,

    // ============ Range Cache ============
    /// Shared ACL range cache.
    range_cache: Arc<AclRangeCache>,

    // ============ State ============
    /// Whether the orch is initialized.
    initialized: bool,

    /// Statistics.
    stats: AclOrchStats,
}

impl AclOrch {
    /// Creates a new AclOrch with the given configuration.
    pub fn new(config: AclOrchConfig) -> Self {
        let mut orch = Self {
            config,
            callbacks: None,
            table_types: HashMap::new(),
            tables: SyncMap::new(),
            table_oid_to_id: HashMap::new(),
            action_capabilities: HashMap::new(),
            metadata_refs: HashMap::new(),
            range_cache: Arc::new(AclRangeCache::new()),
            initialized: false,
            stats: AclOrchStats::default(),
        };

        // Register built-in table types
        orch.register_builtin_types();

        orch
    }

    /// Sets the callbacks.
    pub fn set_callbacks(&mut self, callbacks: AclOrchCallbacks) {
        self.callbacks = Some(Arc::new(callbacks));
    }

    /// Registers the built-in table types.
    fn register_builtin_types(&mut self) {
        let types = [
            create_l3_table_type(),
            create_l3v6_table_type(),
            create_mirror_table_type(),
            create_pfcwd_table_type(),
            create_drop_table_type(),
            create_ctrlplane_table_type(),
        ];

        for tt in types {
            self.table_types.insert(tt.name.clone(), Arc::new(tt));
        }
    }

    // ============ Table Type Operations ============

    /// Gets a table type by name.
    pub fn get_table_type(&self, name: &str) -> Option<Arc<AclTableType>> {
        self.table_types.get(name).cloned()
    }

    /// Registers a custom table type.
    pub fn register_table_type(&mut self, table_type: AclTableType) -> Result<()> {
        if self.table_types.contains_key(&table_type.name) {
            return Err(AclOrchError::InvalidConfig(format!(
                "Table type {} already exists",
                table_type.name
            )));
        }
        self.table_types
            .insert(table_type.name.clone(), Arc::new(table_type));
        Ok(())
    }

    /// Unregisters a custom table type (built-in types cannot be removed).
    pub fn unregister_table_type(&mut self, name: &str) -> Result<()> {
        if let Some(tt) = self.table_types.get(name) {
            if tt.is_builtin {
                return Err(AclOrchError::InvalidConfig(
                    "Cannot remove built-in table type".to_string(),
                ));
            }
        }
        self.table_types.remove(name);
        Ok(())
    }

    /// Returns all registered table type names.
    pub fn table_type_names(&self) -> Vec<String> {
        self.table_types.keys().cloned().collect()
    }

    // ============ Table Operations ============

    /// Returns the number of tables.
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    /// Returns true if a table exists.
    pub fn has_table(&self, table_id: &str) -> bool {
        self.tables.contains_key(&table_id.to_string())
    }

    /// Gets a table by ID.
    pub fn get_table(&self, table_id: &str) -> Option<AclTable> {
        self.tables.get(&table_id.to_string()).map(|t| t.clone())
    }

    /// Gets a table by SAI OID.
    pub fn get_table_by_oid(&self, oid: RawSaiObjectId) -> Option<AclTable> {
        self.table_oid_to_id
            .get(&oid)
            .and_then(|id| self.tables.get(id))
            .map(|t| t.clone())
    }

    /// Creates a new ACL table from configuration.
    pub fn create_table(&mut self, config: &AclTableConfig) -> Result<()> {
        config.validate().map_err(AclOrchError::InvalidConfig)?;

        let table_id = config.id.clone().unwrap();
        let type_name = config.type_name.clone().unwrap();

        // Check if table already exists
        if self.tables.contains_key(&table_id) {
            return Err(AclOrchError::TableAlreadyExists(table_id));
        }

        // Check resource limits
        if self.tables.len() >= self.config.max_tables {
            return Err(AclOrchError::ResourceExhausted("Max tables reached".to_string()));
        }

        // Get table type
        let table_type = self
            .table_types
            .get(&type_name)
            .ok_or_else(|| AclOrchError::TableTypeNotFound(type_name.clone()))?
            .clone();

        // Create table
        let table = AclTable::from_config(config, table_type)
            .map_err(AclOrchError::ValidationError)?;

        // In a real implementation, we would call SAI here to create the table
        // For now, just store it
        self.tables.insert(table_id.clone(), table);
        self.stats.tables_created += 1;

        Ok(())
    }

    /// Removes an ACL table.
    pub fn remove_table(&mut self, table_id: &str) -> Result<()> {
        let table = self
            .tables
            .remove(&table_id.to_string())
            .ok_or_else(|| AclOrchError::TableNotFound(table_id.to_string()))?;

        // Clean up OID mapping
        if table.table_oid != 0 {
            self.table_oid_to_id.remove(&table.table_oid);
        }

        // In a real implementation, we would:
        // 1. Remove all rules
        // 2. Unbind all ports
        // 3. Remove the SAI table

        self.stats.tables_deleted += 1;

        Ok(())
    }

    /// Updates a table's configuration (e.g., ports).
    pub fn update_table_ports(
        &mut self,
        table_id: &str,
        new_ports: Vec<String>,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let table = self
            .tables
            .get_mut(&table_id.to_string())
            .ok_or_else(|| AclOrchError::TableNotFound(table_id.to_string()))?;

        let new_port_set: std::collections::HashSet<_> = new_ports.into_iter().collect();
        Ok(table.update_ports(&new_port_set))
    }

    // ============ Rule Operations ============

    /// Returns the total number of rules across all tables.
    pub fn total_rule_count(&self) -> usize {
        self.tables.values().map(|t| t.rule_count()).sum()
    }

    /// Gets a rule from a table.
    pub fn get_rule(&self, table_id: &str, rule_id: &str) -> Option<AclRule> {
        self.tables
            .get(&table_id.to_string())
            .and_then(|t| t.get_rule(rule_id).cloned())
    }

    /// Adds a rule to a table.
    pub fn add_rule(&mut self, table_id: &str, rule: AclRule) -> Result<()> {
        // Validate priority
        rule.validate(self.config.min_priority, self.config.max_priority)
            .map_err(AclOrchError::ValidationError)?;

        let table = self
            .tables
            .get_mut(&table_id.to_string())
            .ok_or_else(|| AclOrchError::TableNotFound(table_id.to_string()))?;

        table
            .add_rule(rule)
            .map_err(|e| AclOrchError::RuleAlreadyExists(table_id.to_string(), e))?;

        // In a real implementation, we would call SAI here to create the rule

        self.stats.rules_created += 1;

        Ok(())
    }

    /// Removes a rule from a table.
    pub fn remove_rule(&mut self, table_id: &str, rule_id: &str) -> Result<AclRule> {
        let table = self
            .tables
            .get_mut(&table_id.to_string())
            .ok_or_else(|| AclOrchError::TableNotFound(table_id.to_string()))?;

        let rule = table
            .remove_rule(rule_id)
            .ok_or_else(|| AclOrchError::RuleNotFound(table_id.to_string(), rule_id.to_string()))?;

        // In a real implementation, we would call SAI here to remove the rule

        self.stats.rules_deleted += 1;

        Ok(rule)
    }

    /// Updates a rule in a table.
    pub fn update_rule(&mut self, table_id: &str, rule: AclRule) -> Result<AclRule> {
        // Validate priority
        rule.validate(self.config.min_priority, self.config.max_priority)
            .map_err(AclOrchError::ValidationError)?;

        let table = self
            .tables
            .get_mut(&table_id.to_string())
            .ok_or_else(|| AclOrchError::TableNotFound(table_id.to_string()))?;

        let old_rule = table
            .update_rule(rule)
            .map_err(|e| AclOrchError::RuleNotFound(table_id.to_string(), e))?;

        // In a real implementation, we would call SAI here to update the rule

        self.stats.rules_updated += 1;

        Ok(old_rule)
    }

    // ============ Port Binding Operations ============

    /// Binds a port to a table.
    pub fn bind_port(
        &mut self,
        table_id: &str,
        port_alias: &str,
        port_oid: RawSaiObjectId,
    ) -> Result<()> {
        let table = self
            .tables
            .get_mut(&table_id.to_string())
            .ok_or_else(|| AclOrchError::TableNotFound(table_id.to_string()))?;

        // In a real implementation, we would:
        // 1. Create SAI ACL group member
        // 2. Store the binding

        // For now, just record with a placeholder group member OID
        table.bind_port(port_alias, port_oid, 0);

        Ok(())
    }

    /// Unbinds a port from a table.
    pub fn unbind_port(&mut self, table_id: &str, port_alias: &str) -> Result<()> {
        let table = self
            .tables
            .get_mut(&table_id.to_string())
            .ok_or_else(|| AclOrchError::TableNotFound(table_id.to_string()))?;

        // In a real implementation, we would remove the SAI group member

        table.unbind_port(port_alias);

        Ok(())
    }

    // ============ Metadata Operations ============

    /// Allocates a metadata value.
    pub fn allocate_metadata(&mut self) -> Result<MetaDataValue> {
        if !self.config.metadata_supported {
            return Err(AclOrchError::InvalidConfig(
                "Metadata not supported on this platform".to_string(),
            ));
        }

        // Find an unused value
        for value in self.config.metadata_min..=self.config.metadata_max {
            if !self.metadata_refs.contains_key(&value) {
                self.metadata_refs.insert(value, 1);
                return MetaDataValue::new(value)
                    .ok_or_else(|| AclOrchError::InvalidConfig("Invalid metadata value".to_string()));
            }
        }

        Err(AclOrchError::ResourceExhausted(
            "No free metadata values".to_string(),
        ))
    }

    /// Increments the reference count for a metadata value.
    pub fn incr_metadata_ref(&mut self, value: MetaDataValue) {
        *self.metadata_refs.entry(value.value()).or_insert(0) += 1;
    }

    /// Decrements the reference count for a metadata value.
    /// Returns true if the value is now free (ref count = 0).
    pub fn decr_metadata_ref(&mut self, value: MetaDataValue) -> bool {
        if let Some(count) = self.metadata_refs.get_mut(&value.value()) {
            if *count > 0 {
                *count -= 1;
            }
            if *count == 0 {
                self.metadata_refs.remove(&value.value());
                return true;
            }
        }
        false
    }

    /// Returns true if a metadata value is allocated.
    pub fn is_metadata_allocated(&self, value: MetaDataValue) -> bool {
        self.metadata_refs.contains_key(&value.value())
    }

    // ============ State and Statistics ============

    /// Returns true if the orch is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Sets the initialized flag.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.initialized = initialized;
    }

    /// Returns the configuration.
    pub fn config(&self) -> &AclOrchConfig {
        &self.config
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &AclOrchStats {
        &self.stats
    }

    /// Returns the range cache.
    pub fn range_cache(&self) -> &Arc<AclRangeCache> {
        &self.range_cache
    }

    /// Returns all table IDs.
    pub fn table_ids(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::rule::{AclRuleAction, AclRuleMatch};
    use super::*;

    #[test]
    fn test_acl_orch_new() {
        let orch = AclOrch::new(AclOrchConfig::default());

        assert!(!orch.is_initialized());
        assert_eq!(orch.table_count(), 0);

        // Built-in types should be registered
        assert!(orch.get_table_type("L3").is_some());
        assert!(orch.get_table_type("L3V6").is_some());
        assert!(orch.get_table_type("MIRROR").is_some());
    }

    #[test]
    fn test_create_table() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress);

        orch.create_table(&config).unwrap();

        assert!(orch.has_table("TestTable"));
        assert_eq!(orch.table_count(), 1);
    }

    #[test]
    fn test_create_duplicate_table() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress);

        orch.create_table(&config).unwrap();
        let result = orch.create_table(&config);

        assert!(matches!(result, Err(AclOrchError::TableAlreadyExists(_))));
    }

    #[test]
    fn test_remove_table() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress);

        orch.create_table(&config).unwrap();
        orch.remove_table("TestTable").unwrap();

        assert!(!orch.has_table("TestTable"));
        assert_eq!(orch.table_count(), 0);
    }

    #[test]
    fn test_add_rule() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress);

        orch.create_table(&config).unwrap();

        let rule = AclRule::packet("rule1")
            .with_priority(100)
            .with_match(AclRuleMatch::ip_protocol(6))
            .with_action(AclRuleAction::drop());

        orch.add_rule("TestTable", rule).unwrap();

        assert!(orch.get_rule("TestTable", "rule1").is_some());
        assert_eq!(orch.total_rule_count(), 1);
    }

    #[test]
    fn test_remove_rule() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress);

        orch.create_table(&config).unwrap();

        let rule = AclRule::packet("rule1")
            .with_priority(100)
            .with_action(AclRuleAction::drop());

        orch.add_rule("TestTable", rule).unwrap();
        let removed = orch.remove_rule("TestTable", "rule1").unwrap();

        assert_eq!(removed.id, "rule1");
        assert!(orch.get_rule("TestTable", "rule1").is_none());
    }

    #[test]
    fn test_invalid_table_type() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("NONEXISTENT")
            .with_stage(AclStage::Ingress);

        let result = orch.create_table(&config);
        assert!(matches!(result, Err(AclOrchError::TableTypeNotFound(_))));
    }

    #[test]
    fn test_metadata_allocation() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let meta1 = orch.allocate_metadata().unwrap();
        assert!(orch.is_metadata_allocated(meta1));

        let meta2 = orch.allocate_metadata().unwrap();
        assert_ne!(meta1.value(), meta2.value());

        // Release one
        assert!(orch.decr_metadata_ref(meta1));
        assert!(!orch.is_metadata_allocated(meta1));
    }

    #[test]
    fn test_statistics() {
        let mut orch = AclOrch::new(AclOrchConfig::default());

        let config = AclTableConfig::new()
            .with_id("TestTable")
            .with_type("L3")
            .with_stage(AclStage::Ingress);

        orch.create_table(&config).unwrap();
        assert_eq!(orch.stats().tables_created, 1);

        let rule = AclRule::packet("rule1")
            .with_priority(100)
            .with_action(AclRuleAction::drop());
        orch.add_rule("TestTable", rule).unwrap();
        assert_eq!(orch.stats().rules_created, 1);

        orch.remove_rule("TestTable", "rule1").unwrap();
        assert_eq!(orch.stats().rules_deleted, 1);

        orch.remove_table("TestTable").unwrap();
        assert_eq!(orch.stats().tables_deleted, 1);
    }

    #[test]
    fn test_custom_table_type() {
        use super::super::table_type::AclTableTypeBuilder;
        use super::super::types::{AclActionType, AclBindPointType, AclMatchField};

        let mut orch = AclOrch::new(AclOrchConfig::default());

        let custom_type = AclTableTypeBuilder::new()
            .with_name("CUSTOM")
            .with_bind_point(AclBindPointType::Port)
            .with_match(AclMatchField::SrcIp)
            .with_action(AclActionType::PacketAction)
            .build()
            .unwrap();

        orch.register_table_type(custom_type).unwrap();
        assert!(orch.get_table_type("CUSTOM").is_some());

        // Can use custom type
        let config = AclTableConfig::new()
            .with_id("CustomTable")
            .with_type("CUSTOM")
            .with_stage(AclStage::Ingress);

        orch.create_table(&config).unwrap();
        assert!(orch.has_table("CustomTable"));
    }
}
