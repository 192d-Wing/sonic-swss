//! CrmOrch implementation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::types::{
    crm_acl_key, crm_acl_table_key, crm_dash_acl_group_key, crm_ext_table_key,
    AclBindPoint, AclStage, CrmResourceCounter, CrmResourceEntry, CrmResourceStatus,
    CrmResourceType, CrmThresholdField, CrmThresholdType, ThresholdCheck,
    CRM_COUNTERS_TABLE_KEY, DEFAULT_HIGH_THRESHOLD, DEFAULT_LOW_THRESHOLD,
    DEFAULT_POLLING_INTERVAL,
};

/// CRM orchestrator error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrmOrchError {
    /// Resource type not found.
    ResourceNotFound(CrmResourceType),
    /// Counter key not found.
    CounterNotFound(String),
    /// Invalid threshold value.
    InvalidThreshold(String),
    /// Resource not supported.
    NotSupported(CrmResourceType),
    /// Parse error.
    ParseError(String),
}

impl std::fmt::Display for CrmOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceNotFound(r) => write!(f, "Resource not found: {}", r),
            Self::CounterNotFound(k) => write!(f, "Counter not found: {}", k),
            Self::InvalidThreshold(msg) => write!(f, "Invalid threshold: {}", msg),
            Self::NotSupported(r) => write!(f, "Resource not supported: {}", r),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for CrmOrchError {}

// Audit logging macro for CRM operations
macro_rules! audit_log {
    (
        resource_id: $rid:expr,
        action: $act:expr,
        category: $cat:expr,
        outcome: $out:expr,
        details: $det:expr
    ) => {
        // This is a placeholder for actual audit logging
        // In production, this would log to syslog, audit trails, or security logging system
        eprintln!("[AUDIT] Resource: {}, Action: {}, Category: {}, Outcome: {}, Details: {}",
            $rid, $act, $cat, $out, $det);
    };
}

/// Callbacks for CrmOrch operations.
pub trait CrmOrchCallbacks: Send + Sync {
    /// Publishes a threshold event.
    fn publish_threshold_event(
        &self,
        resource: &str,
        counter_key: &str,
        used: u32,
        available: u32,
        threshold: u32,
        exceeded: bool,
    );

    /// Queries available resources from SAI.
    fn query_resource_availability(
        &self,
        resource_type: CrmResourceType,
    ) -> Option<(u32, u32)>; // (used, available)

    /// Queries ACL resource availability.
    fn query_acl_availability(
        &self,
        stage: AclStage,
        bind_point: AclBindPoint,
    ) -> Option<(u32, u32)>;

    /// Writes counters to COUNTERS_DB.
    fn write_counters(
        &self,
        resource: &str,
        key: &str,
        used: u32,
        available: u32,
    );

    /// Returns true if this is a DPU (DASH) switch.
    fn is_dpu(&self) -> bool;
}

/// CRM orchestrator configuration.
#[derive(Debug, Clone)]
pub struct CrmOrchConfig {
    /// Polling interval for resource monitoring.
    pub polling_interval: Duration,
}

impl Default for CrmOrchConfig {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_secs(DEFAULT_POLLING_INTERVAL),
        }
    }
}

impl CrmOrchConfig {
    /// Creates a new config with the given polling interval.
    pub fn with_polling_interval(interval: Duration) -> Self {
        Self {
            polling_interval: interval,
        }
    }
}

/// CRM orchestrator statistics.
#[derive(Debug, Clone, Default)]
pub struct CrmOrchStats {
    /// Number of timer expirations processed.
    pub timer_expirations: u64,
    /// Number of threshold events published.
    pub threshold_events: u64,
    /// Number of configuration updates processed.
    pub config_updates: u64,
    /// Number of resource increments.
    pub increments: u64,
    /// Number of resource decrements.
    pub decrements: u64,
}

/// CRM orchestrator for capacity resource management.
pub struct CrmOrch {
    /// Configuration.
    config: CrmOrchConfig,
    /// Resources map.
    resources: HashMap<CrmResourceType, CrmResourceEntry>,
    /// Callbacks for SAI and DB operations.
    callbacks: Option<Arc<dyn CrmOrchCallbacks>>,
    /// Whether the orch is initialized.
    initialized: bool,
    /// Statistics.
    stats: CrmOrchStats,
}

impl std::fmt::Debug for CrmOrch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrmOrch")
            .field("config", &self.config)
            .field("resources_count", &self.resources.len())
            .field("initialized", &self.initialized)
            .field("stats", &self.stats)
            .finish()
    }
}

impl CrmOrch {
    /// Creates a new CrmOrch with the given configuration.
    pub fn new(config: CrmOrchConfig) -> Self {
        let mut resources = HashMap::new();

        // Initialize all standard resource types
        for &res_type in CrmResourceType::standard_types() {
            resources.insert(res_type, CrmResourceEntry::new(res_type));
        }

        // Initialize DASH resource types
        for &res_type in CrmResourceType::dash_types() {
            resources.insert(res_type, CrmResourceEntry::new(res_type));
        }

        Self {
            config,
            resources,
            callbacks: None,
            initialized: false,
            stats: CrmOrchStats::default(),
        }
    }

    /// Sets the callbacks for this orch.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn CrmOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the configuration.
    pub fn config(&self) -> &CrmOrchConfig {
        &self.config
    }

    /// Returns the polling interval.
    pub fn polling_interval(&self) -> Duration {
        self.config.polling_interval
    }

    /// Sets the polling interval.
    pub fn set_polling_interval(&mut self, interval: Duration) {
        self.config.polling_interval = interval;
        self.stats.config_updates += 1;

        audit_log!(
            resource_id: "crm_config",
            action: "set_polling_interval",
            category: "ConfigurationChange",
            outcome: "SUCCESS",
            details: serde_json::json!({
                "polling_interval_seconds": interval.as_secs(),
            })
        );
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &CrmOrchStats {
        &self.stats
    }

    /// Returns true if the orch is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Marks the orch as initialized.
    pub fn set_initialized(&mut self) {
        self.initialized = true;
    }

    /// Gets a resource entry by type.
    pub fn get_resource(&self, resource_type: CrmResourceType) -> Option<&CrmResourceEntry> {
        self.resources.get(&resource_type)
    }

    /// Gets a mutable resource entry by type.
    pub fn get_resource_mut(
        &mut self,
        resource_type: CrmResourceType,
    ) -> Option<&mut CrmResourceEntry> {
        self.resources.get_mut(&resource_type)
    }

    // ========== Counter Increment/Decrement Operations ==========

    /// Increments the used counter for a global resource.
    pub fn increment_used(&mut self, resource_type: CrmResourceType) -> Result<u32, CrmOrchError> {
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or_else(|| {
                audit_log!(
                    resource_id: &resource_type.name().to_string(),
                    action: "increment_used",
                    category: "ResourceModify",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Resource not found",
                        "resource_type": resource_type.name(),
                    })
                );
                CrmOrchError::ResourceNotFound(resource_type)
            })?;

        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        let new_used = counter.increment_used();
        self.stats.increments += 1;

        audit_log!(
            resource_id: &resource_type.name().to_string(),
            action: "increment_used",
            category: "ResourceModify",
            outcome: "SUCCESS",
            details: serde_json::json!({
                "resource_type": resource_type.name(),
                "used": new_used,
                "available": counter.available,
            })
        );

        Ok(new_used)
    }

    /// Decrements the used counter for a global resource.
    pub fn decrement_used(&mut self, resource_type: CrmResourceType) -> Result<u32, CrmOrchError> {
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or_else(|| {
                audit_log!(
                    resource_id: &resource_type.name().to_string(),
                    action: "decrement_used",
                    category: "ResourceModify",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Resource not found",
                        "resource_type": resource_type.name(),
                    })
                );
                CrmOrchError::ResourceNotFound(resource_type)
            })?;

        let counter = entry
            .get_counter_mut(CRM_COUNTERS_TABLE_KEY)
            .ok_or_else(|| {
                audit_log!(
                    resource_id: &resource_type.name().to_string(),
                    action: "decrement_used",
                    category: "ResourceModify",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Counter not found",
                        "resource_type": resource_type.name(),
                    })
                );
                CrmOrchError::CounterNotFound(CRM_COUNTERS_TABLE_KEY.to_string())
            })?;

        self.stats.decrements += 1;
        counter
            .decrement_used()
            .ok_or_else(|| {
                audit_log!(
                    resource_id: &resource_type.name().to_string(),
                    action: "decrement_used",
                    category: "ResourceModify",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Counter underflow",
                        "resource_type": resource_type.name(),
                    })
                );
                CrmOrchError::InvalidThreshold("Counter underflow".to_string())
            })
            .map(|new_used| {
                audit_log!(
                    resource_id: &resource_type.name().to_string(),
                    action: "decrement_used",
                    category: "ResourceModify",
                    outcome: "SUCCESS",
                    details: serde_json::json!({
                        "resource_type": resource_type.name(),
                        "used": new_used,
                    })
                );
                new_used
            })
    }

    /// Increments the used counter for an ACL resource (table/group).
    pub fn increment_acl_used(
        &mut self,
        resource_type: CrmResourceType,
        stage: AclStage,
        bind_point: AclBindPoint,
    ) -> Result<u32, CrmOrchError> {
        if !resource_type.is_acl_resource() {
            return Err(CrmOrchError::InvalidThreshold(format!(
                "{} is not an ACL resource",
                resource_type
            )));
        }

        let key = crm_acl_key(stage, bind_point);
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        let counter = entry.get_or_create_counter(&key);
        self.stats.increments += 1;
        Ok(counter.increment_used())
    }

    /// Decrements the used counter for an ACL resource.
    /// Also removes per-table ACL entry/counter if table_id is provided.
    pub fn decrement_acl_used(
        &mut self,
        resource_type: CrmResourceType,
        stage: AclStage,
        bind_point: AclBindPoint,
        table_id: Option<u64>,
    ) -> Result<u32, CrmOrchError> {
        if !resource_type.is_acl_resource() {
            return Err(CrmOrchError::InvalidThreshold(format!(
                "{} is not an ACL resource",
                resource_type
            )));
        }

        let key = crm_acl_key(stage, bind_point);
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        let counter = entry
            .get_counter_mut(&key)
            .ok_or_else(|| CrmOrchError::CounterNotFound(key.clone()))?;

        self.stats.decrements += 1;
        let result = counter
            .decrement_used()
            .ok_or_else(|| CrmOrchError::InvalidThreshold("Counter underflow".to_string()))?;

        // If table_id is provided and this is AclTable, also clean up per-table entries
        if let Some(tid) = table_id {
            if resource_type == CrmResourceType::AclTable {
                let table_key = crm_acl_table_key(tid);
                // Remove entry and counter for this table from AclEntry and AclCounter
                if let Some(entry_res) = self.resources.get_mut(&CrmResourceType::AclEntry) {
                    entry_res.remove_counter(&table_key);
                }
                if let Some(counter_res) = self.resources.get_mut(&CrmResourceType::AclCounter) {
                    counter_res.remove_counter(&table_key);
                }
            }
        }

        Ok(result)
    }

    /// Increments the used counter for a per-table ACL resource.
    pub fn increment_acl_table_used(
        &mut self,
        resource_type: CrmResourceType,
        table_id: u64,
    ) -> Result<u32, CrmOrchError> {
        if !resource_type.is_per_table_resource() {
            return Err(CrmOrchError::InvalidThreshold(format!(
                "{} is not a per-table resource",
                resource_type
            )));
        }

        let key = crm_acl_table_key(table_id);
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        let counter = entry.get_or_create_counter(&key);
        counter.id = table_id;
        self.stats.increments += 1;
        Ok(counter.increment_used())
    }

    /// Decrements the used counter for a per-table ACL resource.
    pub fn decrement_acl_table_used(
        &mut self,
        resource_type: CrmResourceType,
        table_id: u64,
    ) -> Result<u32, CrmOrchError> {
        if !resource_type.is_per_table_resource() {
            return Err(CrmOrchError::InvalidThreshold(format!(
                "{} is not a per-table resource",
                resource_type
            )));
        }

        let key = crm_acl_table_key(table_id);
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        let counter = entry
            .get_counter_mut(&key)
            .ok_or_else(|| CrmOrchError::CounterNotFound(key))?;

        self.stats.decrements += 1;
        counter
            .decrement_used()
            .ok_or_else(|| CrmOrchError::InvalidThreshold("Counter underflow".to_string()))
    }

    /// Increments the used counter for an extension table.
    pub fn increment_ext_table_used(&mut self, table_name: &str) -> Result<u32, CrmOrchError> {
        let key = crm_ext_table_key(table_name);
        let entry = self
            .resources
            .get_mut(&CrmResourceType::ExtTable)
            .ok_or(CrmOrchError::ResourceNotFound(CrmResourceType::ExtTable))?;

        let counter = entry.get_or_create_counter(&key);
        self.stats.increments += 1;
        Ok(counter.increment_used())
    }

    /// Decrements the used counter for an extension table.
    pub fn decrement_ext_table_used(&mut self, table_name: &str) -> Result<u32, CrmOrchError> {
        let key = crm_ext_table_key(table_name);
        let entry = self
            .resources
            .get_mut(&CrmResourceType::ExtTable)
            .ok_or(CrmOrchError::ResourceNotFound(CrmResourceType::ExtTable))?;

        let counter = entry
            .get_counter_mut(&key)
            .ok_or_else(|| CrmOrchError::CounterNotFound(key))?;

        self.stats.decrements += 1;
        counter
            .decrement_used()
            .ok_or_else(|| CrmOrchError::InvalidThreshold("Counter underflow".to_string()))
    }

    /// Increments the used counter for a DASH ACL resource.
    /// For DashAclGroup, this also initializes the rule counter.
    pub fn increment_dash_acl_used(
        &mut self,
        resource_type: CrmResourceType,
        group_id: u64,
    ) -> Result<u32, CrmOrchError> {
        if resource_type != CrmResourceType::DashAclGroup
            && resource_type != CrmResourceType::DashAclRule
        {
            return Err(CrmOrchError::InvalidThreshold(format!(
                "{} is not a DASH ACL resource",
                resource_type
            )));
        }

        let key = crm_dash_acl_group_key(group_id);
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        let counter = entry.get_or_create_counter(&key);
        counter.id = group_id;
        self.stats.increments += 1;
        let result = counter.increment_used();

        // When adding a DASH ACL group, also initialize its rule counter
        if resource_type == CrmResourceType::DashAclGroup {
            if let Some(rule_entry) = self.resources.get_mut(&CrmResourceType::DashAclRule) {
                let rule_counter = rule_entry.get_or_create_counter(&key);
                rule_counter.id = group_id;
            }
        }

        Ok(result)
    }

    /// Decrements the used counter for a DASH ACL resource.
    /// For DashAclGroup, this also removes the rule counter.
    pub fn decrement_dash_acl_used(
        &mut self,
        resource_type: CrmResourceType,
        group_id: u64,
    ) -> Result<u32, CrmOrchError> {
        if resource_type != CrmResourceType::DashAclGroup
            && resource_type != CrmResourceType::DashAclRule
        {
            return Err(CrmOrchError::InvalidThreshold(format!(
                "{} is not a DASH ACL resource",
                resource_type
            )));
        }

        let key = crm_dash_acl_group_key(group_id);
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        let counter = entry
            .get_counter_mut(&key)
            .ok_or_else(|| CrmOrchError::CounterNotFound(key.clone()))?;

        self.stats.decrements += 1;
        let result = counter
            .decrement_used()
            .ok_or_else(|| CrmOrchError::InvalidThreshold("Counter underflow".to_string()))?;

        // When removing a DASH ACL group, also remove its rule counter
        if resource_type == CrmResourceType::DashAclGroup && result == 0 {
            entry.remove_counter(&key);
            if let Some(rule_entry) = self.resources.get_mut(&CrmResourceType::DashAclRule) {
                rule_entry.remove_counter(&key);
            }
        }

        Ok(result)
    }

    // ========== Configuration Operations ==========

    /// Sets the threshold type for a resource.
    pub fn set_threshold_type(
        &mut self,
        resource_type: CrmResourceType,
        threshold_type: CrmThresholdType,
    ) -> Result<(), CrmOrchError> {
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        entry.threshold_type = threshold_type;
        self.stats.config_updates += 1;
        Ok(())
    }

    /// Sets the low threshold for a resource.
    pub fn set_low_threshold(
        &mut self,
        resource_type: CrmResourceType,
        value: u32,
    ) -> Result<(), CrmOrchError> {
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        entry.low_threshold = value;
        self.stats.config_updates += 1;
        Ok(())
    }

    /// Sets the high threshold for a resource.
    pub fn set_high_threshold(
        &mut self,
        resource_type: CrmResourceType,
        value: u32,
    ) -> Result<(), CrmOrchError> {
        let entry = self
            .resources
            .get_mut(&resource_type)
            .ok_or(CrmOrchError::ResourceNotFound(resource_type))?;

        entry.high_threshold = value;
        self.stats.config_updates += 1;
        Ok(())
    }

    /// Handles a configuration field update.
    pub fn handle_config_field(&mut self, field: &str, value: &str) -> Result<(), CrmOrchError> {
        // Handle polling interval separately
        if field == "polling_interval" {
            let interval: u64 = value
                .parse()
                .map_err(|_| CrmOrchError::ParseError(format!("Invalid interval: {}", value)))?;
            self.set_polling_interval(Duration::from_secs(interval));
            return Ok(());
        }

        // Parse threshold field
        let (resource_name, field_type) = CrmThresholdField::parse_field(field)
            .ok_or_else(|| CrmOrchError::ParseError(format!("Unknown field: {}", field)))?;

        let resource_type: CrmResourceType = resource_name
            .parse()
            .map_err(|e| CrmOrchError::ParseError(e))?;

        match field_type {
            CrmThresholdField::Type => {
                let threshold_type: CrmThresholdType = value
                    .parse()
                    .map_err(|e| CrmOrchError::ParseError(e))?;
                self.set_threshold_type(resource_type, threshold_type)?;
            }
            CrmThresholdField::Low => {
                let threshold: u32 = value
                    .parse()
                    .map_err(|_| CrmOrchError::ParseError(format!("Invalid value: {}", value)))?;
                self.set_low_threshold(resource_type, threshold)?;
            }
            CrmThresholdField::High => {
                let threshold: u32 = value
                    .parse()
                    .map_err(|_| CrmOrchError::ParseError(format!("Invalid value: {}", value)))?;
                self.set_high_threshold(resource_type, threshold)?;
            }
        }

        Ok(())
    }

    // ========== Timer/Polling Operations ==========

    /// Handles timer expiration - queries SAI, updates counters, checks thresholds.
    pub fn handle_timer_expiration(&mut self) {
        self.stats.timer_expirations += 1;

        // Query SAI for available counters
        self.get_resource_available_counters();

        // Update COUNTERS_DB
        self.update_counters_table();

        // Check thresholds and publish events
        self.check_thresholds();
    }

    /// Queries SAI for resource availability and updates counters.
    fn get_resource_available_counters(&mut self) {
        let callbacks = match &self.callbacks {
            Some(cb) => Arc::clone(cb),
            None => return,
        };

        let is_dpu = callbacks.is_dpu();

        // Query standard resources
        for &res_type in CrmResourceType::standard_types() {
            if res_type.is_acl_resource() {
                // ACL resources are queried per stage/bind-point
                continue;
            }

            if let Some((used, available)) = callbacks.query_resource_availability(res_type) {
                if let Some(entry) = self.resources.get_mut(&res_type) {
                    let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
                    counter.available = available;
                    // Don't overwrite used - it's tracked by increment/decrement

                    // Log successful query for critical resources
                    audit_log!(
                        resource_id: &res_type.name().to_string(),
                        action: "query_resource_availability",
                        category: "Read",
                        outcome: "SUCCESS",
                        details: serde_json::json!({
                            "resource_type": res_type.name(),
                            "used": used,
                            "available": available,
                            "utilization_percent": counter.utilization_percent(),
                        })
                    );
                }
            }
        }

        // Query ACL resources per stage/bind-point combination
        let stages = [AclStage::Ingress, AclStage::Egress];
        let bind_points = [
            AclBindPoint::Port,
            AclBindPoint::Lag,
            AclBindPoint::Vlan,
            AclBindPoint::Rif,
            AclBindPoint::Switch,
        ];

        for &stage in &stages {
            for &bind_point in &bind_points {
                if let Some((used, available)) = callbacks.query_acl_availability(stage, bind_point)
                {
                    let key = crm_acl_key(stage, bind_point);

                    // Update AclTable and AclGroup
                    for &res_type in &[CrmResourceType::AclTable, CrmResourceType::AclGroup] {
                        if let Some(entry) = self.resources.get_mut(&res_type) {
                            let counter = entry.get_or_create_counter(&key);
                            counter.available = available;
                        }
                    }
                }
            }
        }

        // Query DASH resources if this is a DPU
        if is_dpu {
            for &res_type in CrmResourceType::dash_types() {
                if let Some((used, available)) = callbacks.query_resource_availability(res_type) {
                    if let Some(entry) = self.resources.get_mut(&res_type) {
                        // DASH resources may have per-group counters
                        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
                        counter.available = available;
                    }
                }
            }
        }
    }

    /// Updates COUNTERS_DB with current counter values.
    fn update_counters_table(&self) {
        let callbacks = match &self.callbacks {
            Some(cb) => cb,
            None => return,
        };

        for (res_type, entry) in &self.resources {
            for (key, counter) in &entry.counters {
                callbacks.write_counters(res_type.name(), key, counter.used, counter.available);
            }
        }
    }

    /// Checks thresholds and publishes events for any violations.
    fn check_thresholds(&mut self) {
        let callbacks = match &self.callbacks {
            Some(cb) => Arc::clone(cb),
            None => return,
        };

        for (res_type, entry) in &mut self.resources {
            let threshold_type = entry.threshold_type;
            let high = entry.high_threshold;
            let low = entry.low_threshold;

            for (key, counter) in &mut entry.counters {
                if counter.used == 0 && counter.available == 0 {
                    continue;
                }

                match counter.check_threshold(threshold_type, high, low) {
                    ThresholdCheck::Exceeded { utilization, threshold } => {
                        self.stats.threshold_events += 1;
                        callbacks.publish_threshold_event(
                            res_type.name(),
                            key,
                            counter.used,
                            counter.available,
                            threshold,
                            true,
                        );
                    }
                    ThresholdCheck::Recovered { utilization, threshold } => {
                        callbacks.publish_threshold_event(
                            res_type.name(),
                            key,
                            counter.used,
                            counter.available,
                            threshold,
                            false,
                        );
                    }
                    ThresholdCheck::Normal => {}
                }
            }
        }
    }

    /// Returns the used counter value for a resource.
    pub fn get_used(&self, resource_type: CrmResourceType) -> Option<u32> {
        self.resources
            .get(&resource_type)
            .and_then(|entry| entry.get_counter(CRM_COUNTERS_TABLE_KEY))
            .map(|counter| counter.used)
    }

    /// Returns the available counter value for a resource.
    pub fn get_available(&self, resource_type: CrmResourceType) -> Option<u32> {
        self.resources
            .get(&resource_type)
            .and_then(|entry| entry.get_counter(CRM_COUNTERS_TABLE_KEY))
            .map(|counter| counter.available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crm::CRM_EXCEEDED_MSG_MAX;

    #[test]
    fn test_crm_orch_new() {
        let orch = CrmOrch::new(CrmOrchConfig::default());
        assert!(!orch.is_initialized());
        assert_eq!(orch.polling_interval(), Duration::from_secs(DEFAULT_POLLING_INTERVAL));

        // Check that all standard resources are initialized
        for &res_type in CrmResourceType::standard_types() {
            assert!(orch.get_resource(res_type).is_some());
        }
    }

    #[test]
    fn test_increment_decrement_global() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Increment
        let used = orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(used, 1);

        let used = orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(used, 2);

        // Decrement
        let used = orch.decrement_used(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(used, 1);

        // Verify statistics
        assert_eq!(orch.stats().increments, 2);
        assert_eq!(orch.stats().decrements, 1);
    }

    #[test]
    fn test_increment_decrement_underflow() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Decrement without increment should fail
        let result = orch.decrement_used(CrmResourceType::Ipv4Route);
        assert!(result.is_err());
    }

    #[test]
    fn test_acl_increment_decrement() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Increment ACL table
        let used = orch
            .increment_acl_used(CrmResourceType::AclTable, AclStage::Ingress, AclBindPoint::Port)
            .unwrap();
        assert_eq!(used, 1);

        // Decrement
        let used = orch
            .decrement_acl_used(
                CrmResourceType::AclTable,
                AclStage::Ingress,
                AclBindPoint::Port,
                None,
            )
            .unwrap();
        assert_eq!(used, 0);
    }

    #[test]
    fn test_per_table_acl() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Increment per-table ACL entry
        let table_id = 0x1234;
        let used = orch
            .increment_acl_table_used(CrmResourceType::AclEntry, table_id)
            .unwrap();
        assert_eq!(used, 1);

        // Verify counter was created
        let entry = orch.get_resource(CrmResourceType::AclEntry).unwrap();
        let key = crm_acl_table_key(table_id);
        assert!(entry.get_counter(&key).is_some());

        // Decrement
        let used = orch
            .decrement_acl_table_used(CrmResourceType::AclEntry, table_id)
            .unwrap();
        assert_eq!(used, 0);
    }

    #[test]
    fn test_ext_table() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let used = orch.increment_ext_table_used("my_p4_table").unwrap();
        assert_eq!(used, 1);

        let used = orch.decrement_ext_table_used("my_p4_table").unwrap();
        assert_eq!(used, 0);
    }

    #[test]
    fn test_dash_acl() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());
        let group_id = 0xabcd;

        // Increment DASH ACL group
        let used = orch
            .increment_dash_acl_used(CrmResourceType::DashAclGroup, group_id)
            .unwrap();
        assert_eq!(used, 1);

        // Verify rule counter was also created
        let rule_entry = orch.get_resource(CrmResourceType::DashAclRule).unwrap();
        let key = crm_dash_acl_group_key(group_id);
        assert!(rule_entry.get_counter(&key).is_some());

        // Increment rule
        let used = orch
            .increment_dash_acl_used(CrmResourceType::DashAclRule, group_id)
            .unwrap();
        assert_eq!(used, 1);
    }

    #[test]
    fn test_threshold_config() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.set_threshold_type(CrmResourceType::Ipv4Route, CrmThresholdType::Used)
            .unwrap();
        orch.set_low_threshold(CrmResourceType::Ipv4Route, 50)
            .unwrap();
        orch.set_high_threshold(CrmResourceType::Ipv4Route, 90)
            .unwrap();

        let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(entry.threshold_type, CrmThresholdType::Used);
        assert_eq!(entry.low_threshold, 50);
        assert_eq!(entry.high_threshold, 90);
    }

    #[test]
    fn test_handle_config_field() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Polling interval
        orch.handle_config_field("polling_interval", "60").unwrap();
        assert_eq!(orch.polling_interval(), Duration::from_secs(60));

        // Threshold type
        orch.handle_config_field("ipv4_route_threshold_type", "used")
            .unwrap();
        let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(entry.threshold_type, CrmThresholdType::Used);

        // Low threshold
        orch.handle_config_field("ipv4_route_low_threshold", "50")
            .unwrap();
        let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(entry.low_threshold, 50);

        // High threshold
        orch.handle_config_field("ipv4_route_high_threshold", "95")
            .unwrap();
        let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(entry.high_threshold, 95);
    }

    #[test]
    fn test_invalid_resource() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Non-ACL resource for ACL operation
        let result =
            orch.increment_acl_used(CrmResourceType::Ipv4Route, AclStage::Ingress, AclBindPoint::Port);
        assert!(result.is_err());

        // Non-per-table resource for per-table operation
        let result = orch.increment_acl_table_used(CrmResourceType::AclTable, 0x1234);
        assert!(result.is_err());
    }

    #[test]
    fn test_statistics() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        orch.decrement_used(CrmResourceType::Ipv4Route).unwrap();
        orch.set_polling_interval(Duration::from_secs(60));
        orch.handle_timer_expiration();

        let stats = orch.stats();
        assert_eq!(stats.increments, 2);
        assert_eq!(stats.decrements, 1);
        assert_eq!(stats.config_updates, 1);
        assert_eq!(stats.timer_expirations, 1);
    }

    #[test]
    fn test_get_used_available() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Initial state
        assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(0));

        // After increment
        orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(1));

        // Available is set by SAI queries
        assert_eq!(orch.get_available(CrmResourceType::Ipv4Route), Some(0));
    }

    // ========== Resource Tracking Tests ==========

    #[test]
    fn test_ipv4_route_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Increment IPv4 routes
        assert_eq!(orch.increment_used(CrmResourceType::Ipv4Route).unwrap(), 1);
        assert_eq!(orch.increment_used(CrmResourceType::Ipv4Route).unwrap(), 2);
        assert_eq!(orch.increment_used(CrmResourceType::Ipv4Route).unwrap(), 3);

        assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(3));

        // Decrement
        assert_eq!(orch.decrement_used(CrmResourceType::Ipv4Route).unwrap(), 2);
        assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(2));
    }

    #[test]
    fn test_ipv6_route_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        assert_eq!(orch.increment_used(CrmResourceType::Ipv6Route).unwrap(), 1);
        assert_eq!(orch.increment_used(CrmResourceType::Ipv6Route).unwrap(), 2);
        assert_eq!(orch.get_used(CrmResourceType::Ipv6Route), Some(2));

        assert_eq!(orch.decrement_used(CrmResourceType::Ipv6Route).unwrap(), 1);
        assert_eq!(orch.decrement_used(CrmResourceType::Ipv6Route).unwrap(), 0);
    }

    #[test]
    fn test_ipv4_neighbor_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        for i in 1..=10 {
            assert_eq!(orch.increment_used(CrmResourceType::Ipv4Neighbor).unwrap(), i);
        }
        assert_eq!(orch.get_used(CrmResourceType::Ipv4Neighbor), Some(10));
    }

    #[test]
    fn test_ipv6_neighbor_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        for i in 1..=5 {
            assert_eq!(orch.increment_used(CrmResourceType::Ipv6Neighbor).unwrap(), i);
        }
        assert_eq!(orch.get_used(CrmResourceType::Ipv6Neighbor), Some(5));

        for i in (0..5).rev() {
            assert_eq!(orch.decrement_used(CrmResourceType::Ipv6Neighbor).unwrap(), i);
        }
    }

    #[test]
    fn test_nexthop_group_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        assert_eq!(orch.increment_used(CrmResourceType::NexthopGroup).unwrap(), 1);
        assert_eq!(orch.increment_used(CrmResourceType::NexthopGroup).unwrap(), 2);
        assert_eq!(orch.get_used(CrmResourceType::NexthopGroup), Some(2));
    }

    #[test]
    fn test_nexthop_group_member_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Add multiple members
        for i in 1..=20 {
            assert_eq!(orch.increment_used(CrmResourceType::NexthopGroupMember).unwrap(), i);
        }
        assert_eq!(orch.get_used(CrmResourceType::NexthopGroupMember), Some(20));

        // Remove some members
        assert_eq!(orch.decrement_used(CrmResourceType::NexthopGroupMember).unwrap(), 19);
        assert_eq!(orch.decrement_used(CrmResourceType::NexthopGroupMember).unwrap(), 18);
    }

    #[test]
    fn test_acl_table_tracking_multiple_stages() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Ingress + Port
        let used = orch.increment_acl_used(
            CrmResourceType::AclTable,
            AclStage::Ingress,
            AclBindPoint::Port
        ).unwrap();
        assert_eq!(used, 1);

        // Egress + Port (different counter)
        let used = orch.increment_acl_used(
            CrmResourceType::AclTable,
            AclStage::Egress,
            AclBindPoint::Port
        ).unwrap();
        assert_eq!(used, 1);

        // Ingress + Vlan (different counter)
        let used = orch.increment_acl_used(
            CrmResourceType::AclTable,
            AclStage::Ingress,
            AclBindPoint::Vlan
        ).unwrap();
        assert_eq!(used, 1);

        // Increment same counter again
        let used = orch.increment_acl_used(
            CrmResourceType::AclTable,
            AclStage::Ingress,
            AclBindPoint::Port
        ).unwrap();
        assert_eq!(used, 2);
    }

    #[test]
    fn test_acl_entry_per_table() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let table_id_1 = 0x1000;
        let table_id_2 = 0x2000;

        // Add entries to table 1
        assert_eq!(orch.increment_acl_table_used(CrmResourceType::AclEntry, table_id_1).unwrap(), 1);
        assert_eq!(orch.increment_acl_table_used(CrmResourceType::AclEntry, table_id_1).unwrap(), 2);
        assert_eq!(orch.increment_acl_table_used(CrmResourceType::AclEntry, table_id_1).unwrap(), 3);

        // Add entries to table 2
        assert_eq!(orch.increment_acl_table_used(CrmResourceType::AclEntry, table_id_2).unwrap(), 1);
        assert_eq!(orch.increment_acl_table_used(CrmResourceType::AclEntry, table_id_2).unwrap(), 2);

        // Verify separate tracking
        let entry = orch.get_resource(CrmResourceType::AclEntry).unwrap();
        let key1 = crm_acl_table_key(table_id_1);
        let key2 = crm_acl_table_key(table_id_2);
        assert_eq!(entry.get_counter(&key1).unwrap().used, 3);
        assert_eq!(entry.get_counter(&key2).unwrap().used, 2);
    }

    #[test]
    fn test_fdb_entry_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        assert_eq!(orch.increment_used(CrmResourceType::FdbEntry).unwrap(), 1);
        assert_eq!(orch.increment_used(CrmResourceType::FdbEntry).unwrap(), 2);
        assert_eq!(orch.get_used(CrmResourceType::FdbEntry), Some(2));

        assert_eq!(orch.decrement_used(CrmResourceType::FdbEntry).unwrap(), 1);
    }

    #[test]
    fn test_ipmc_entry_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        for i in 1..=100 {
            assert_eq!(orch.increment_used(CrmResourceType::IpmcEntry).unwrap(), i);
        }
        assert_eq!(orch.get_used(CrmResourceType::IpmcEntry), Some(100));
    }

    #[test]
    fn test_dnat_snat_entry_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // DNAT entries
        assert_eq!(orch.increment_used(CrmResourceType::DnatEntry).unwrap(), 1);
        assert_eq!(orch.increment_used(CrmResourceType::DnatEntry).unwrap(), 2);
        assert_eq!(orch.get_used(CrmResourceType::DnatEntry), Some(2));

        // SNAT entries
        assert_eq!(orch.increment_used(CrmResourceType::SnatEntry).unwrap(), 1);
        assert_eq!(orch.increment_used(CrmResourceType::SnatEntry).unwrap(), 2);
        assert_eq!(orch.increment_used(CrmResourceType::SnatEntry).unwrap(), 3);
        assert_eq!(orch.get_used(CrmResourceType::SnatEntry), Some(3));
    }

    // ========== Threshold Configuration Tests ==========

    #[test]
    fn test_set_threshold_type_percentage() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.set_threshold_type(CrmResourceType::Ipv4Route, CrmThresholdType::Percentage).unwrap();
        let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(entry.threshold_type, CrmThresholdType::Percentage);
    }

    #[test]
    fn test_set_threshold_type_used() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.set_threshold_type(CrmResourceType::Ipv6Neighbor, CrmThresholdType::Used).unwrap();
        let entry = orch.get_resource(CrmResourceType::Ipv6Neighbor).unwrap();
        assert_eq!(entry.threshold_type, CrmThresholdType::Used);
    }

    #[test]
    fn test_set_threshold_type_free() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.set_threshold_type(CrmResourceType::FdbEntry, CrmThresholdType::Free).unwrap();
        let entry = orch.get_resource(CrmResourceType::FdbEntry).unwrap();
        assert_eq!(entry.threshold_type, CrmThresholdType::Free);
    }

    #[test]
    fn test_set_high_low_thresholds() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.set_high_threshold(CrmResourceType::Ipv4Route, 95).unwrap();
        orch.set_low_threshold(CrmResourceType::Ipv4Route, 60).unwrap();

        let entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(entry.high_threshold, 95);
        assert_eq!(entry.low_threshold, 60);
    }

    #[test]
    fn test_threshold_config_multiple_resources() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Configure IPv4 routes
        orch.set_threshold_type(CrmResourceType::Ipv4Route, CrmThresholdType::Percentage).unwrap();
        orch.set_high_threshold(CrmResourceType::Ipv4Route, 90).unwrap();
        orch.set_low_threshold(CrmResourceType::Ipv4Route, 70).unwrap();

        // Configure IPv6 neighbors
        orch.set_threshold_type(CrmResourceType::Ipv6Neighbor, CrmThresholdType::Used).unwrap();
        orch.set_high_threshold(CrmResourceType::Ipv6Neighbor, 1000).unwrap();
        orch.set_low_threshold(CrmResourceType::Ipv6Neighbor, 500).unwrap();

        // Verify independent configuration
        let ipv4_entry = orch.get_resource(CrmResourceType::Ipv4Route).unwrap();
        assert_eq!(ipv4_entry.threshold_type, CrmThresholdType::Percentage);
        assert_eq!(ipv4_entry.high_threshold, 90);

        let ipv6_entry = orch.get_resource(CrmResourceType::Ipv6Neighbor).unwrap();
        assert_eq!(ipv6_entry.threshold_type, CrmThresholdType::Used);
        assert_eq!(ipv6_entry.high_threshold, 1000);
    }

    #[test]
    fn test_threshold_config_updates_stats() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());
        let initial_updates = orch.stats().config_updates;

        orch.set_threshold_type(CrmResourceType::Ipv4Route, CrmThresholdType::Used).unwrap();
        orch.set_high_threshold(CrmResourceType::Ipv4Route, 100).unwrap();
        orch.set_low_threshold(CrmResourceType::Ipv4Route, 50).unwrap();

        assert_eq!(orch.stats().config_updates, initial_updates + 3);
    }

    // ========== Polling Configuration Tests ==========

    #[test]
    fn test_set_polling_interval() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.set_polling_interval(Duration::from_secs(60));
        assert_eq!(orch.polling_interval(), Duration::from_secs(60));

        orch.set_polling_interval(Duration::from_secs(120));
        assert_eq!(orch.polling_interval(), Duration::from_secs(120));
    }

    #[test]
    fn test_polling_interval_validation() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Very short interval
        orch.set_polling_interval(Duration::from_secs(1));
        assert_eq!(orch.polling_interval(), Duration::from_secs(1));

        // Very long interval
        orch.set_polling_interval(Duration::from_secs(3600));
        assert_eq!(orch.polling_interval(), Duration::from_secs(3600));
    }

    #[test]
    fn test_default_polling_interval() {
        let orch = CrmOrch::new(CrmOrchConfig::default());
        assert_eq!(orch.polling_interval(), Duration::from_secs(DEFAULT_POLLING_INTERVAL));
        assert_eq!(orch.polling_interval(), Duration::from_secs(300)); // 5 minutes
    }

    #[test]
    fn test_custom_polling_config() {
        let config = CrmOrchConfig::with_polling_interval(Duration::from_secs(180));
        let orch = CrmOrch::new(config);
        assert_eq!(orch.polling_interval(), Duration::from_secs(180));
    }

    #[test]
    fn test_handle_config_field_polling_interval() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.handle_config_field("polling_interval", "90").unwrap();
        assert_eq!(orch.polling_interval(), Duration::from_secs(90));
    }

    #[test]
    fn test_handle_config_field_invalid_interval() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let result = orch.handle_config_field("polling_interval", "not_a_number");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::ParseError(_)));
    }

    // ========== Resource Usage Queries Tests ==========

    #[test]
    fn test_get_available_resources() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Set available through resource entry
        let entry = orch.get_resource_mut(CrmResourceType::Ipv4Route).unwrap();
        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.available = 1000;

        assert_eq!(orch.get_available(CrmResourceType::Ipv4Route), Some(1000));
    }

    #[test]
    fn test_get_used_resources() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        orch.increment_used(CrmResourceType::Ipv6Route).unwrap();
        orch.increment_used(CrmResourceType::Ipv6Route).unwrap();
        orch.increment_used(CrmResourceType::Ipv6Route).unwrap();

        assert_eq!(orch.get_used(CrmResourceType::Ipv6Route), Some(3));
    }

    #[test]
    fn test_usage_percentage_calculation() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Set up a resource with known usage
        let entry = orch.get_resource_mut(CrmResourceType::FdbEntry).unwrap();
        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.used = 75;
        counter.available = 25;

        // Verify percentage calculation: 75 / (75 + 25) * 100 = 75%
        assert_eq!(counter.utilization_percent(), 75);
    }

    #[test]
    fn test_usage_percentage_at_limits() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let entry = orch.get_resource_mut(CrmResourceType::Ipv4Neighbor).unwrap();
        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);

        // 0% usage
        counter.used = 0;
        counter.available = 100;
        assert_eq!(counter.utilization_percent(), 0);

        // 100% usage
        counter.used = 100;
        counter.available = 0;
        assert_eq!(counter.utilization_percent(), 100);

        // 50% usage
        counter.used = 50;
        counter.available = 50;
        assert_eq!(counter.utilization_percent(), 50);
    }

    #[test]
    fn test_per_resource_type_queries() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Set different values for different resources
        orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        orch.increment_used(CrmResourceType::Ipv6Route).unwrap();
        orch.increment_used(CrmResourceType::Ipv6Route).unwrap();
        orch.increment_used(CrmResourceType::FdbEntry).unwrap();
        orch.increment_used(CrmResourceType::FdbEntry).unwrap();
        orch.increment_used(CrmResourceType::FdbEntry).unwrap();

        assert_eq!(orch.get_used(CrmResourceType::Ipv4Route), Some(1));
        assert_eq!(orch.get_used(CrmResourceType::Ipv6Route), Some(2));
        assert_eq!(orch.get_used(CrmResourceType::FdbEntry), Some(3));
    }

    // ========== Threshold Violations Tests ==========

    #[test]
    fn test_threshold_exceeded_high() {
        let mut counter = CrmResourceCounter::new();
        counter.used = 90;
        counter.available = 10;

        // Check with percentage threshold
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { utilization: 90, threshold: 85 }));
        assert_eq!(counter.exceeded_log_count, 1);
    }

    #[test]
    fn test_threshold_recovery_low() {
        let mut counter = CrmResourceCounter::new();
        counter.used = 90;
        counter.available = 10;

        // First exceed
        counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert_eq!(counter.exceeded_log_count, 1);

        // Then recover
        counter.used = 60;
        counter.available = 40;
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Recovered { utilization: 60, threshold: 70 }));
        assert_eq!(counter.exceeded_log_count, 0);
    }

    #[test]
    fn test_multiple_simultaneous_violations() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Set up multiple resources with violations
        let ipv4_entry = orch.get_resource_mut(CrmResourceType::Ipv4Route).unwrap();
        let counter = ipv4_entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.used = 90;
        counter.available = 10;

        let ipv6_entry = orch.get_resource_mut(CrmResourceType::Ipv6Route).unwrap();
        let counter = ipv6_entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.used = 95;
        counter.available = 5;

        let fdb_entry = orch.get_resource_mut(CrmResourceType::FdbEntry).unwrap();
        let counter = fdb_entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.used = 88;
        counter.available = 12;

        // All should report exceeded
        let ipv4_counter = orch.get_resource(CrmResourceType::Ipv4Route).unwrap()
            .get_counter(CRM_COUNTERS_TABLE_KEY).unwrap();
        let result = ipv4_counter.clone().check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { .. }));

        let ipv6_counter = orch.get_resource(CrmResourceType::Ipv6Route).unwrap()
            .get_counter(CRM_COUNTERS_TABLE_KEY).unwrap();
        let result = ipv6_counter.clone().check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { .. }));
    }

    #[test]
    fn test_threshold_state_tracking() {
        let mut counter = CrmResourceCounter::new();
        counter.used = 50;
        counter.available = 50;

        // Normal state
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Normal));
        assert_eq!(counter.exceeded_log_count, 0);

        // Exceed threshold
        counter.used = 90;
        counter.available = 10;
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { .. }));
        assert_eq!(counter.exceeded_log_count, 1);

        // Still exceeded (rate limited after reaching)
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { .. }));
        assert_eq!(counter.exceeded_log_count, 2);
    }

    #[test]
    fn test_threshold_with_used_type() {
        let mut counter = CrmResourceCounter::new();
        counter.used = 1000;
        counter.available = 500;

        // Threshold based on used count
        let result = counter.check_threshold(CrmThresholdType::Used, 900, 700);
        assert!(matches!(result, ThresholdCheck::Exceeded { utilization: 1000, threshold: 900 }));

        // Recovery
        counter.used = 650;
        let result = counter.check_threshold(CrmThresholdType::Used, 900, 700);
        assert!(matches!(result, ThresholdCheck::Recovered { utilization: 650, threshold: 700 }));
    }

    #[test]
    fn test_threshold_with_free_type() {
        let mut counter = CrmResourceCounter::new();
        counter.used = 50;
        counter.available = 150;

        // Threshold based on free count (high free triggers exceeded)
        // When available >= high (150 >= 100), it's "exceeded"
        let result = counter.check_threshold(CrmThresholdType::Free, 100, 50);
        assert!(matches!(result, ThresholdCheck::Exceeded { utilization: 150, threshold: 100 }));

        // Recovery (less free)
        counter.available = 40;
        let result = counter.check_threshold(CrmThresholdType::Free, 100, 50);
        assert!(matches!(result, ThresholdCheck::Recovered { utilization: 40, threshold: 50 }));
    }

    // ========== Statistics Tests ==========

    #[test]
    fn test_increment_decrement_stats() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        assert_eq!(orch.stats().increments, 0);
        assert_eq!(orch.stats().decrements, 0);

        orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        orch.increment_used(CrmResourceType::Ipv4Route).unwrap();
        orch.increment_used(CrmResourceType::Ipv6Route).unwrap();

        assert_eq!(orch.stats().increments, 3);

        orch.decrement_used(CrmResourceType::Ipv4Route).unwrap();

        assert_eq!(orch.stats().decrements, 1);
    }

    #[test]
    fn test_config_update_stats() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let initial = orch.stats().config_updates;

        orch.set_polling_interval(Duration::from_secs(60));
        orch.set_threshold_type(CrmResourceType::Ipv4Route, CrmThresholdType::Used).unwrap();
        orch.set_high_threshold(CrmResourceType::Ipv4Route, 100).unwrap();
        orch.set_low_threshold(CrmResourceType::Ipv4Route, 50).unwrap();

        assert_eq!(orch.stats().config_updates, initial + 4);
    }

    #[test]
    fn test_timer_expiration_stats() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        assert_eq!(orch.stats().timer_expirations, 0);

        orch.handle_timer_expiration();
        assert_eq!(orch.stats().timer_expirations, 1);

        orch.handle_timer_expiration();
        orch.handle_timer_expiration();
        assert_eq!(orch.stats().timer_expirations, 3);
    }

    #[test]
    fn test_acl_stats_tracking() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let initial_inc = orch.stats().increments;
        let initial_dec = orch.stats().decrements;

        orch.increment_acl_used(CrmResourceType::AclTable, AclStage::Ingress, AclBindPoint::Port).unwrap();
        orch.increment_acl_used(CrmResourceType::AclTable, AclStage::Egress, AclBindPoint::Vlan).unwrap();

        assert_eq!(orch.stats().increments, initial_inc + 2);

        orch.decrement_acl_used(CrmResourceType::AclTable, AclStage::Ingress, AclBindPoint::Port, None).unwrap();

        assert_eq!(orch.stats().decrements, initial_dec + 1);
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_invalid_resource_type_for_acl_operation() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Try to use non-ACL resource for ACL operation
        let result = orch.increment_acl_used(
            CrmResourceType::Ipv4Route,
            AclStage::Ingress,
            AclBindPoint::Port
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::InvalidThreshold(_)));
    }

    #[test]
    fn test_invalid_resource_type_for_per_table_operation() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Try to use non-per-table resource for per-table operation
        let result = orch.increment_acl_table_used(CrmResourceType::AclTable, 0x1234);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::InvalidThreshold(_)));
    }

    #[test]
    fn test_decrement_without_counter() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Decrement ACL without creating counter first
        let result = orch.decrement_acl_used(
            CrmResourceType::AclTable,
            AclStage::Ingress,
            AclBindPoint::Port,
            None
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::CounterNotFound(_)));
    }

    #[test]
    fn test_counter_underflow_protection() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        // Try to decrement below zero - should get InvalidThreshold error
        let result = orch.decrement_used(CrmResourceType::Ipv4Route);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::InvalidThreshold(_)));
    }

    #[test]
    fn test_invalid_config_field() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let result = orch.handle_config_field("invalid_field_name", "100");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::ParseError(_)));
    }

    #[test]
    fn test_invalid_threshold_value() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let result = orch.handle_config_field("ipv4_route_high_threshold", "not_a_number");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::ParseError(_)));
    }

    #[test]
    fn test_invalid_threshold_type_value() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let result = orch.handle_config_field("ipv4_route_threshold_type", "invalid_type");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CrmOrchError::ParseError(_)));
    }

    // ========== Edge Cases Tests ==========

    #[test]
    fn test_resource_at_100_percent_usage() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let entry = orch.get_resource_mut(CrmResourceType::Ipv4Route).unwrap();
        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.used = 1000;
        counter.available = 0;

        assert_eq!(counter.utilization_percent(), 100);

        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { utilization: 100, .. }));
    }

    #[test]
    fn test_resource_at_0_percent_usage() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let entry = orch.get_resource_mut(CrmResourceType::Ipv6Route).unwrap();
        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.used = 0;
        counter.available = 1000;

        assert_eq!(counter.utilization_percent(), 0);

        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Normal));
    }

    #[test]
    fn test_rapid_threshold_crossing() {
        let mut counter = CrmResourceCounter::new();

        // Start with 10% usage
        counter.used = 10;
        counter.available = 90;

        // Cross high threshold (90% usage)
        counter.used = 90;
        counter.available = 10;
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { .. }));
        assert_eq!(counter.exceeded_log_count, 1);

        // Drop below low threshold (60% usage)
        counter.used = 60;
        counter.available = 40;
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Recovered { .. }));
        assert_eq!(counter.exceeded_log_count, 0);

        // Cross high again (95% usage - should report exceeded since counter was reset)
        counter.used = 95;
        counter.available = 5;
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Exceeded { .. }));
        assert_eq!(counter.exceeded_log_count, 1);

        // Drop below low again (65% usage)
        counter.used = 65;
        counter.available = 35;
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Recovered { .. }));
        assert_eq!(counter.exceeded_log_count, 0);
    }

    #[test]
    fn test_zero_total_resources() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());

        let entry = orch.get_resource_mut(CrmResourceType::FdbEntry).unwrap();
        let counter = entry.get_or_create_counter(CRM_COUNTERS_TABLE_KEY);
        counter.used = 0;
        counter.available = 0;

        // Should handle division by zero gracefully
        assert_eq!(counter.utilization_percent(), 0);
    }

    #[test]
    fn test_acl_table_cleanup_on_delete() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());
        let table_id = 0x5678;

        // Add ACL table
        orch.increment_acl_used(CrmResourceType::AclTable, AclStage::Ingress, AclBindPoint::Port).unwrap();

        // Add entries for this table
        orch.increment_acl_table_used(CrmResourceType::AclEntry, table_id).unwrap();
        orch.increment_acl_table_used(CrmResourceType::AclCounter, table_id).unwrap();

        // Verify counters exist
        let table_key = crm_acl_table_key(table_id);
        assert!(orch.get_resource(CrmResourceType::AclEntry).unwrap().get_counter(&table_key).is_some());
        assert!(orch.get_resource(CrmResourceType::AclCounter).unwrap().get_counter(&table_key).is_some());

        // Delete the table (should clean up entry and counter)
        orch.decrement_acl_used(
            CrmResourceType::AclTable,
            AclStage::Ingress,
            AclBindPoint::Port,
            Some(table_id)
        ).unwrap();

        // Verify cleanup
        assert!(orch.get_resource(CrmResourceType::AclEntry).unwrap().get_counter(&table_key).is_none());
        assert!(orch.get_resource(CrmResourceType::AclCounter).unwrap().get_counter(&table_key).is_none());
    }

    #[test]
    fn test_dash_acl_group_cleanup() {
        let mut orch = CrmOrch::new(CrmOrchConfig::default());
        let group_id = 0xabcd;

        // Add DASH ACL group
        orch.increment_dash_acl_used(CrmResourceType::DashAclGroup, group_id).unwrap();

        // Verify rule counter was created
        let key = crm_dash_acl_group_key(group_id);
        assert!(orch.get_resource(CrmResourceType::DashAclRule).unwrap().get_counter(&key).is_some());

        // Delete the group
        orch.decrement_dash_acl_used(CrmResourceType::DashAclGroup, group_id).unwrap();

        // Verify cleanup
        assert!(orch.get_resource(CrmResourceType::DashAclGroup).unwrap().get_counter(&key).is_none());
        assert!(orch.get_resource(CrmResourceType::DashAclRule).unwrap().get_counter(&key).is_none());
    }

    #[test]
    fn test_saturating_counter_increment() {
        let mut counter = CrmResourceCounter::new();
        counter.used = u32::MAX - 1;

        // Should saturate at u32::MAX
        counter.increment_used();
        assert_eq!(counter.used, u32::MAX);

        counter.increment_used();
        assert_eq!(counter.used, u32::MAX);
    }

    #[test]
    fn test_threshold_rate_limiting_max_messages() {
        let mut counter = CrmResourceCounter::new();
        counter.used = 100;
        counter.available = 0;

        // Should report exceeded for first CRM_EXCEEDED_MSG_MAX times
        for i in 0..CRM_EXCEEDED_MSG_MAX {
            let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
            assert!(matches!(result, ThresholdCheck::Exceeded { .. }));
            assert_eq!(counter.exceeded_log_count, i + 1);
        }

        // After max, should report Normal (rate limited)
        let result = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(result, ThresholdCheck::Normal));
        assert_eq!(counter.exceeded_log_count, CRM_EXCEEDED_MSG_MAX);
    }

    #[test]
    fn test_all_standard_resources_initialized() {
        let orch = CrmOrch::new(CrmOrchConfig::default());

        // Verify all standard resource types are present
        for &res_type in CrmResourceType::standard_types() {
            assert!(orch.get_resource(res_type).is_some(),
                "Resource {} should be initialized", res_type);
        }
    }

    #[test]
    fn test_all_dash_resources_initialized() {
        let orch = CrmOrch::new(CrmOrchConfig::default());

        // Verify all DASH resource types are present
        for &res_type in CrmResourceType::dash_types() {
            assert!(orch.get_resource(res_type).is_some(),
                "DASH resource {} should be initialized", res_type);
        }
    }
}
