//! Debug counter orchestration logic.

use super::types::{DebugCounterConfig, DebugCounterEntry, DebugCounterType, DropReason, FreeCounter};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};
use crate::audit_log;

#[derive(Debug, Clone)]
pub enum DebugCounterOrchError {
    CounterNotFound(String),
    CounterExists(String),
    DropReasonNotFound(String),
    InvalidType(String),
    FlexCounterError(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct DebugCounterOrchConfig {
    pub enable_flex_counter: bool,
    pub polling_interval_ms: u32,
}

#[derive(Debug, Clone, Default)]
pub struct DebugCounterOrchStats {
    pub counters_created: u64,
    pub counters_removed: u64,
    pub drop_reasons_added: u64,
    pub drop_reasons_removed: u64,
    pub flex_counter_registrations: u64,
}

pub trait DebugCounterOrchCallbacks: Send + Sync {
    fn create_debug_counter(&self, counter_type: DebugCounterType) -> Result<RawSaiObjectId, String>;
    fn remove_debug_counter(&self, oid: RawSaiObjectId) -> Result<(), String>;
    fn add_drop_reason_to_counter(&self, counter_id: RawSaiObjectId, drop_reason: &str) -> Result<(), String>;
    fn remove_drop_reason_from_counter(&self, counter_id: RawSaiObjectId, drop_reason: &str) -> Result<(), String>;
    fn register_flex_counter(&self, counter_id: RawSaiObjectId, counter_name: &str) -> Result<(), String>;
    fn unregister_flex_counter(&self, counter_name: &str) -> Result<(), String>;
    fn get_available_drop_reasons(&self, is_ingress: bool) -> Vec<String>;
}

pub struct DebugCounterOrch {
    config: DebugCounterOrchConfig,
    stats: DebugCounterOrchStats,
    callbacks: Option<Arc<dyn DebugCounterOrchCallbacks>>,
    debug_counters: HashMap<String, DebugCounterEntry>,
    free_counters: Vec<FreeCounter>,
}

impl DebugCounterOrch {
    pub fn new(config: DebugCounterOrchConfig) -> Self {
        Self {
            config,
            stats: DebugCounterOrchStats::default(),
            callbacks: None,
            debug_counters: HashMap::new(),
            free_counters: Vec::new(),
        }
    }

    pub fn set_callbacks(&mut self, callbacks: Arc<dyn DebugCounterOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    pub fn counter_exists(&self, name: &str) -> bool {
        self.debug_counters.contains_key(name)
    }

    pub fn get_counter(&self, name: &str) -> Option<&DebugCounterEntry> {
        self.debug_counters.get(name)
    }

    pub fn get_counter_mut(&mut self, name: &str) -> Option<&mut DebugCounterEntry> {
        self.debug_counters.get_mut(name)
    }

    pub fn create_debug_counter(&mut self, config: DebugCounterConfig) -> Result<(), DebugCounterOrchError> {
        if self.debug_counters.contains_key(&config.name) {
            let record = AuditRecord::new(
                AuditCategory::ErrorCondition,
                "DebugCounterOrch",
                format!("create_counter_failed: {}", config.name),
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&config.name)
            .with_object_type("debug_counter")
            .with_error("Counter already exists");
            audit_log!(record);

            return Err(DebugCounterOrchError::CounterExists(config.name.clone()));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| DebugCounterOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let counter_id = callbacks.create_debug_counter(config.counter_type)
            .map_err(DebugCounterOrchError::SaiError)?;

        let mut entry = DebugCounterEntry::new(config.name.clone(), config.counter_type, counter_id);
        entry.description = config.description.clone();

        // Add drop reasons
        for reason in &config.drop_reasons {
            callbacks.add_drop_reason_to_counter(counter_id, reason)
                .map_err(DebugCounterOrchError::SaiError)?;
            entry.add_drop_reason(reason.clone());
            self.stats.drop_reasons_added += 1;
        }

        // Register with flex counter if enabled
        if self.config.enable_flex_counter {
            callbacks.register_flex_counter(counter_id, &config.name)
                .map_err(DebugCounterOrchError::FlexCounterError)?;
            self.stats.flex_counter_registrations += 1;
        }

        let record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "DebugCounterOrch",
            format!("create_counter: {}", config.name),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&config.name)
        .with_object_type("debug_counter")
        .with_details(serde_json::json!({
            "counter_type": config.counter_type.as_str(),
            "counter_id": format!("{:#x}", counter_id),
            "drop_reasons_count": config.drop_reasons.len(),
            "flex_counter_enabled": self.config.enable_flex_counter,
        }));
        audit_log!(record);

        self.debug_counters.insert(config.name.clone(), entry);
        self.stats.counters_created += 1;

        Ok(())
    }

    pub fn remove_debug_counter(&mut self, name: &str) -> Result<(), DebugCounterOrchError> {
        let entry = self.debug_counters.remove(name)
            .ok_or_else(|| {
                let record = AuditRecord::new(
                    AuditCategory::ErrorCondition,
                    "DebugCounterOrch",
                    format!("remove_counter_failed: {}", name),
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(name)
                .with_object_type("debug_counter")
                .with_error("Counter not found");
                audit_log!(record);

                DebugCounterOrchError::CounterNotFound(name.to_string())
            })?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| DebugCounterOrchError::SaiError("No callbacks set".to_string()))?;

        // Unregister from flex counter if enabled
        if self.config.enable_flex_counter {
            let _ = callbacks.unregister_flex_counter(&entry.name);
        }

        // Remove all drop reasons
        for reason in &entry.drop_reasons {
            let _ = callbacks.remove_drop_reason_from_counter(entry.counter_id, reason);
        }

        // Remove counter
        callbacks.remove_debug_counter(entry.counter_id)
            .map_err(DebugCounterOrchError::SaiError)?;

        let record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "DebugCounterOrch",
            format!("remove_counter: {}", name),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(name)
        .with_object_type("debug_counter")
        .with_details(serde_json::json!({
            "counter_id": format!("{:#x}", entry.counter_id),
            "drop_reasons_removed": entry.drop_reasons.len(),
        }));
        audit_log!(record);

        self.stats.counters_removed += 1;

        Ok(())
    }

    pub fn add_drop_reason(&mut self, counter_name: &str, drop_reason: &str) -> Result<(), DebugCounterOrchError> {
        let entry = self.debug_counters.get_mut(counter_name)
            .ok_or_else(|| DebugCounterOrchError::CounterNotFound(counter_name.to_string()))?;

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| DebugCounterOrchError::SaiError("No callbacks set".to_string()))?,
        );

        callbacks.add_drop_reason_to_counter(entry.counter_id, drop_reason)
            .map_err(DebugCounterOrchError::SaiError)?;

        entry.add_drop_reason(drop_reason.to_string());
        self.stats.drop_reasons_added += 1;

        Ok(())
    }

    pub fn remove_drop_reason(&mut self, counter_name: &str, drop_reason: &str) -> Result<(), DebugCounterOrchError> {
        let entry = self.debug_counters.get_mut(counter_name)
            .ok_or_else(|| DebugCounterOrchError::CounterNotFound(counter_name.to_string()))?;

        if !entry.drop_reasons.contains(drop_reason) {
            return Err(DebugCounterOrchError::DropReasonNotFound(drop_reason.to_string()));
        }

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| DebugCounterOrchError::SaiError("No callbacks set".to_string()))?;

        callbacks.remove_drop_reason_from_counter(entry.counter_id, drop_reason)
            .map_err(DebugCounterOrchError::SaiError)?;

        entry.remove_drop_reason(drop_reason);
        self.stats.drop_reasons_removed += 1;

        // Track as free counter if no drop reasons left
        if entry.drop_reasons.is_empty() {
            self.free_counters.push(FreeCounter::new(
                entry.name.clone(),
                entry.counter_type.as_str().to_string(),
            ));
        }

        Ok(())
    }

    pub fn reconcile_drop_reasons(&mut self, counter_name: &str) -> Result<(), DebugCounterOrchError> {
        let entry = self.debug_counters.get(counter_name)
            .ok_or_else(|| DebugCounterOrchError::CounterNotFound(counter_name.to_string()))?;

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| DebugCounterOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let is_ingress = entry.counter_type.is_ingress();
        let available_reasons = callbacks.get_available_drop_reasons(is_ingress);

        // Remove invalid reasons
        let current_reasons: Vec<String> = entry.drop_reasons.iter().cloned().collect();
        let mut removed_count = 0;
        for reason in current_reasons {
            if !available_reasons.contains(&reason) {
                self.remove_drop_reason(counter_name, &reason)?;
                removed_count += 1;
            }
        }

        let record = AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "DebugCounterOrch",
            format!("reconcile_drop_reasons: {}", counter_name),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(counter_name)
        .with_object_type("debug_counter")
        .with_details(serde_json::json!({
            "counter_type": entry.counter_type.as_str(),
            "removed_count": removed_count,
            "remaining_reasons": entry.drop_reasons.len() - removed_count,
        }));
        audit_log!(record);

        Ok(())
    }

    pub fn get_free_counters(&self) -> &[FreeCounter] {
        &self.free_counters
    }

    pub fn clear_free_counters(&mut self) {
        self.free_counters.clear();
    }

    pub fn stats(&self) -> &DebugCounterOrchStats {
        &self.stats
    }

    pub fn counter_count(&self) -> usize {
        self.debug_counters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCallbacks;

    impl DebugCounterOrchCallbacks for MockCallbacks {
        fn create_debug_counter(&self, _counter_type: DebugCounterType) -> Result<RawSaiObjectId, String> {
            Ok(0x1000)
        }

        fn remove_debug_counter(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn add_drop_reason_to_counter(&self, _counter_id: RawSaiObjectId, _drop_reason: &str) -> Result<(), String> {
            Ok(())
        }

        fn remove_drop_reason_from_counter(&self, _counter_id: RawSaiObjectId, _drop_reason: &str) -> Result<(), String> {
            Ok(())
        }

        fn register_flex_counter(&self, _counter_id: RawSaiObjectId, _counter_name: &str) -> Result<(), String> {
            Ok(())
        }

        fn unregister_flex_counter(&self, _counter_name: &str) -> Result<(), String> {
            Ok(())
        }

        fn get_available_drop_reasons(&self, _is_ingress: bool) -> Vec<String> {
            vec!["L3_ANY".to_string(), "L2_ANY".to_string()]
        }
    }

    #[test]
    fn test_create_debug_counter() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        config.add_drop_reason("L3_ANY".to_string());

        assert!(orch.create_debug_counter(config).is_ok());
        assert_eq!(orch.counter_count(), 1);
        assert_eq!(orch.stats().counters_created, 1);
        assert_eq!(orch.stats().drop_reasons_added, 1);
    }

    #[test]
    fn test_add_remove_drop_reason() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        orch.create_debug_counter(config).unwrap();

        assert!(orch.add_drop_reason("counter1", "L3_ANY").is_ok());
        assert_eq!(orch.stats().drop_reasons_added, 1);

        assert!(orch.remove_drop_reason("counter1", "L3_ANY").is_ok());
        assert_eq!(orch.stats().drop_reasons_removed, 1);
    }

    #[test]
    fn test_free_counter_tracking() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        config.add_drop_reason("L3_ANY".to_string());
        orch.create_debug_counter(config).unwrap();

        assert_eq!(orch.get_free_counters().len(), 0);

        orch.remove_drop_reason("counter1", "L3_ANY").unwrap();
        assert_eq!(orch.get_free_counters().len(), 1);
    }

    #[test]
    fn test_flex_counter_integration() {
        let mut config = DebugCounterOrchConfig::default();
        config.enable_flex_counter = true;

        let mut orch = DebugCounterOrch::new(config);
        orch.set_callbacks(Arc::new(MockCallbacks));

        let counter_config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::SwitchIngressDrops);
        assert!(orch.create_debug_counter(counter_config).is_ok());
        assert_eq!(orch.stats().flex_counter_registrations, 1);
    }

    #[test]
    fn test_reconcile_drop_reasons() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        config.add_drop_reason("L3_ANY".to_string());
        config.add_drop_reason("INVALID_REASON".to_string());
        orch.create_debug_counter(config).unwrap();

        assert!(orch.reconcile_drop_reasons("counter1").is_ok());
    }

    // === Debug Counter Types Tests ===

    #[test]
    fn test_create_port_level_counter() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("port_counter".to_string(), DebugCounterType::PortIngressDrops);
        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("port_counter").unwrap();
        assert!(entry.counter_type.is_port_counter());
        assert!(entry.counter_type.is_ingress());
    }

    #[test]
    fn test_create_switch_level_counter() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("switch_counter".to_string(), DebugCounterType::SwitchIngressDrops);
        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("switch_counter").unwrap();
        assert!(entry.counter_type.is_switch_counter());
        assert!(entry.counter_type.is_ingress());
    }

    #[test]
    fn test_create_ingress_and_egress_counters() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let ingress_config = DebugCounterConfig::new("ingress_counter".to_string(), DebugCounterType::PortIngressDrops);
        let egress_config = DebugCounterConfig::new("egress_counter".to_string(), DebugCounterType::PortEgressDrops);

        assert!(orch.create_debug_counter(ingress_config).is_ok());
        assert!(orch.create_debug_counter(egress_config).is_ok());

        let ingress = orch.get_counter("ingress_counter").unwrap();
        let egress = orch.get_counter("egress_counter").unwrap();

        assert!(ingress.counter_type.is_ingress());
        assert!(egress.counter_type.is_egress());
        assert_eq!(orch.counter_count(), 2);
    }

    // === Counter Creation Tests ===

    #[test]
    fn test_create_counter_with_multiple_drop_reasons() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("multi_reason_counter".to_string(), DebugCounterType::PortIngressDrops);
        config.add_drop_reason("L3_ANY".to_string());
        config.add_drop_reason("L2_ANY".to_string());
        config.add_drop_reason("ACL_ANY".to_string());

        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("multi_reason_counter").unwrap();
        assert_eq!(entry.drop_reason_count(), 3);
        assert_eq!(orch.stats().drop_reasons_added, 3);
    }

    #[test]
    fn test_create_counter_with_description() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("described_counter".to_string(), DebugCounterType::SwitchIngressDrops)
            .with_description("Counter for monitoring L3 drops".to_string());

        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("described_counter").unwrap();
        assert_eq!(entry.description, Some("Counter for monitoring L3 drops".to_string()));
    }

    #[test]
    fn test_create_counter_all_types() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let types = vec![
            ("port_ingress", DebugCounterType::PortIngressDrops),
            ("port_egress", DebugCounterType::PortEgressDrops),
            ("switch_ingress", DebugCounterType::SwitchIngressDrops),
            ("switch_egress", DebugCounterType::SwitchEgressDrops),
        ];

        for (name, counter_type) in types {
            let config = DebugCounterConfig::new(name.to_string(), counter_type);
            assert!(orch.create_debug_counter(config).is_ok(), "Failed to create counter: {}", name);
        }

        assert_eq!(orch.counter_count(), 4);
        assert_eq!(orch.stats().counters_created, 4);
    }

    // === Drop Reasons Tests ===

    #[test]
    fn test_add_multiple_drop_reasons_sequentially() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        orch.create_debug_counter(config).unwrap();

        let drop_reasons = vec!["L3_ANY", "L2_ANY", "ACL_ANY", "TTL", "VLAN"];
        for reason in &drop_reasons {
            assert!(orch.add_drop_reason("counter1", reason).is_ok());
        }

        let entry = orch.get_counter("counter1").unwrap();
        assert_eq!(entry.drop_reason_count(), drop_reasons.len());
        assert_eq!(orch.stats().drop_reasons_added, drop_reasons.len() as u64);
    }

    #[test]
    fn test_remove_multiple_drop_reasons() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        config.add_drop_reason("L3_ANY".to_string());
        config.add_drop_reason("L2_ANY".to_string());
        config.add_drop_reason("ACL_ANY".to_string());
        orch.create_debug_counter(config).unwrap();

        assert!(orch.remove_drop_reason("counter1", "L3_ANY").is_ok());
        assert!(orch.remove_drop_reason("counter1", "L2_ANY").is_ok());

        let entry = orch.get_counter("counter1").unwrap();
        assert_eq!(entry.drop_reason_count(), 1);
        assert_eq!(orch.stats().drop_reasons_removed, 2);
    }

    #[test]
    fn test_l2_drop_reasons() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("l2_counter".to_string(), DebugCounterType::PortIngressDrops);
        config.add_drop_reason("STP".to_string());
        config.add_drop_reason("VLAN_TAG_NOT_ALLOWED".to_string());
        config.add_drop_reason("L2_ANY".to_string());

        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("l2_counter").unwrap();
        assert_eq!(entry.drop_reason_count(), 3);
    }

    #[test]
    fn test_l3_drop_reasons() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("l3_counter".to_string(), DebugCounterType::SwitchIngressDrops);
        config.add_drop_reason("L3_ANY".to_string());
        config.add_drop_reason("EXCEEDS_L3_MTU".to_string());
        config.add_drop_reason("IP_HEADER_ERROR".to_string());
        config.add_drop_reason("UC_DIP_MC_DMAC".to_string());
        config.add_drop_reason("DIP_LOOPBACK".to_string());

        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("l3_counter").unwrap();
        assert_eq!(entry.drop_reason_count(), 5);
    }

    // === Counter Operations Tests ===

    #[test]
    fn test_remove_debug_counter() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("counter_to_remove".to_string(), DebugCounterType::PortIngressDrops);
        orch.create_debug_counter(config).unwrap();

        assert_eq!(orch.counter_count(), 1);
        assert!(orch.remove_debug_counter("counter_to_remove").is_ok());
        assert_eq!(orch.counter_count(), 0);
        assert_eq!(orch.stats().counters_removed, 1);
    }

    #[test]
    fn test_multiple_counters_simultaneously() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        for i in 1..=5 {
            let mut config = DebugCounterConfig::new(
                format!("counter_{}", i),
                DebugCounterType::PortIngressDrops
            );
            config.add_drop_reason(format!("REASON_{}", i));
            assert!(orch.create_debug_counter(config).is_ok());
        }

        assert_eq!(orch.counter_count(), 5);

        for i in 1..=5 {
            let entry = orch.get_counter(&format!("counter_{}", i));
            assert!(entry.is_some());
        }
    }

    #[test]
    fn test_counter_exists() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        assert!(!orch.counter_exists("nonexistent"));

        let config = DebugCounterConfig::new("existing_counter".to_string(), DebugCounterType::PortIngressDrops);
        orch.create_debug_counter(config).unwrap();

        assert!(orch.counter_exists("existing_counter"));
        assert!(!orch.counter_exists("still_nonexistent"));
    }

    // === Counter Configuration Tests ===

    #[test]
    fn test_add_drop_reason_after_creation() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        orch.create_debug_counter(config).unwrap();

        let entry = orch.get_counter("counter1").unwrap();
        assert_eq!(entry.drop_reason_count(), 0);

        assert!(orch.add_drop_reason("counter1", "L3_ANY").is_ok());
        assert!(orch.add_drop_reason("counter1", "L2_ANY").is_ok());

        let entry = orch.get_counter("counter1").unwrap();
        assert_eq!(entry.drop_reason_count(), 2);
    }

    #[test]
    fn test_get_counter_mut() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("mutable_counter".to_string(), DebugCounterType::SwitchIngressDrops);
        orch.create_debug_counter(config).unwrap();

        if let Some(entry) = orch.get_counter_mut("mutable_counter") {
            entry.description = Some("Modified description".to_string());
            entry.add_drop_reason("NEW_REASON".to_string());
        }

        let entry = orch.get_counter("mutable_counter").unwrap();
        assert_eq!(entry.description, Some("Modified description".to_string()));
        assert!(entry.drop_reasons.contains("NEW_REASON"));
    }

    #[test]
    fn test_clear_free_counters() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        config.add_drop_reason("L3_ANY".to_string());
        orch.create_debug_counter(config).unwrap();

        orch.remove_drop_reason("counter1", "L3_ANY").unwrap();
        assert_eq!(orch.get_free_counters().len(), 1);

        orch.clear_free_counters();
        assert_eq!(orch.get_free_counters().len(), 0);
    }

    // === Statistics Tracking Tests ===

    #[test]
    fn test_statistics_tracking() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config1 = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        config1.add_drop_reason("L3_ANY".to_string());
        config1.add_drop_reason("L2_ANY".to_string());
        orch.create_debug_counter(config1).unwrap();

        let config2 = DebugCounterConfig::new("counter2".to_string(), DebugCounterType::SwitchIngressDrops);
        orch.create_debug_counter(config2).unwrap();

        assert_eq!(orch.stats().counters_created, 2);
        assert_eq!(orch.stats().drop_reasons_added, 2);

        orch.add_drop_reason("counter2", "ACL_ANY").unwrap();
        assert_eq!(orch.stats().drop_reasons_added, 3);

        orch.remove_drop_reason("counter1", "L3_ANY").unwrap();
        assert_eq!(orch.stats().drop_reasons_removed, 1);

        orch.remove_debug_counter("counter1").unwrap();
        assert_eq!(orch.stats().counters_removed, 1);
    }

    #[test]
    fn test_flex_counter_statistics() {
        let mut config = DebugCounterOrchConfig::default();
        config.enable_flex_counter = true;

        let mut orch = DebugCounterOrch::new(config);
        orch.set_callbacks(Arc::new(MockCallbacks));

        let counter_config1 = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        let counter_config2 = DebugCounterConfig::new("counter2".to_string(), DebugCounterType::SwitchIngressDrops);

        orch.create_debug_counter(counter_config1).unwrap();
        orch.create_debug_counter(counter_config2).unwrap();

        assert_eq!(orch.stats().flex_counter_registrations, 2);
    }

    // === Error Handling Tests ===

    #[test]
    fn test_duplicate_counter_error() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config1 = DebugCounterConfig::new("duplicate".to_string(), DebugCounterType::PortIngressDrops);
        assert!(orch.create_debug_counter(config1).is_ok());

        let config2 = DebugCounterConfig::new("duplicate".to_string(), DebugCounterType::SwitchIngressDrops);
        let result = orch.create_debug_counter(config2);

        assert!(result.is_err());
        match result {
            Err(DebugCounterOrchError::CounterExists(name)) => assert_eq!(name, "duplicate"),
            _ => panic!("Expected CounterExists error"),
        }
    }

    #[test]
    fn test_counter_not_found_error() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.remove_debug_counter("nonexistent");
        assert!(result.is_err());
        match result {
            Err(DebugCounterOrchError::CounterNotFound(name)) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected CounterNotFound error"),
        }
    }

    #[test]
    fn test_drop_reason_not_found_error() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        orch.create_debug_counter(config).unwrap();

        let result = orch.remove_drop_reason("counter1", "NONEXISTENT_REASON");
        assert!(result.is_err());
        match result {
            Err(DebugCounterOrchError::DropReasonNotFound(reason)) => {
                assert_eq!(reason, "NONEXISTENT_REASON")
            }
            _ => panic!("Expected DropReasonNotFound error"),
        }
    }

    #[test]
    fn test_add_drop_reason_to_nonexistent_counter() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.add_drop_reason("nonexistent", "L3_ANY");
        assert!(result.is_err());
        match result {
            Err(DebugCounterOrchError::CounterNotFound(name)) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected CounterNotFound error"),
        }
    }

    struct MockCallbacksWithFailure {
        fail_create: bool,
    }

    impl DebugCounterOrchCallbacks for MockCallbacksWithFailure {
        fn create_debug_counter(&self, _counter_type: DebugCounterType) -> Result<RawSaiObjectId, String> {
            if self.fail_create {
                Err("SAI creation failed".to_string())
            } else {
                Ok(0x1000)
            }
        }

        fn remove_debug_counter(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn add_drop_reason_to_counter(&self, _counter_id: RawSaiObjectId, _drop_reason: &str) -> Result<(), String> {
            Ok(())
        }

        fn remove_drop_reason_from_counter(&self, _counter_id: RawSaiObjectId, _drop_reason: &str) -> Result<(), String> {
            Ok(())
        }

        fn register_flex_counter(&self, _counter_id: RawSaiObjectId, _counter_name: &str) -> Result<(), String> {
            Ok(())
        }

        fn unregister_flex_counter(&self, _counter_name: &str) -> Result<(), String> {
            Ok(())
        }

        fn get_available_drop_reasons(&self, _is_ingress: bool) -> Vec<String> {
            vec!["L3_ANY".to_string(), "L2_ANY".to_string()]
        }
    }

    #[test]
    fn test_sai_creation_failure() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacksWithFailure { fail_create: true }));

        let config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        let result = orch.create_debug_counter(config);

        assert!(result.is_err());
        match result {
            Err(DebugCounterOrchError::SaiError(_)) => {},
            _ => panic!("Expected SaiError"),
        }
        assert_eq!(orch.counter_count(), 0);
    }

    #[test]
    fn test_no_callbacks_set_error() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        // Don't set callbacks

        let config = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        let result = orch.create_debug_counter(config);

        assert!(result.is_err());
        match result {
            Err(DebugCounterOrchError::SaiError(msg)) => assert!(msg.contains("No callbacks")),
            _ => panic!("Expected SaiError for no callbacks"),
        }
    }

    // === Edge Cases Tests ===

    #[test]
    fn test_counter_with_no_drop_reasons() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = DebugCounterConfig::new("empty_counter".to_string(), DebugCounterType::PortIngressDrops);
        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("empty_counter").unwrap();
        assert_eq!(entry.drop_reason_count(), 0);
    }

    #[test]
    fn test_counter_with_many_drop_reasons() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = DebugCounterConfig::new("full_counter".to_string(), DebugCounterType::SwitchIngressDrops);

        // Add many drop reasons
        let reasons = vec![
            "L2_ANY", "L3_ANY", "ACL_ANY", "TUNNEL_ANY",
            "STP", "VLAN_TAG_NOT_ALLOWED", "INGRESS_VLAN_FILTER",
            "FDB_SA_MISS", "FDB_SA_MOVE", "FDB_DA_MISS",
            "EXCEEDS_L3_MTU", "TTL", "L3_LOOPBACK",
            "NON_ROUTABLE", "NO_L3_HEADER", "IP_HEADER_ERROR",
            "UC_DIP_MC_DMAC", "DIP_LOOPBACK", "SIP_LOOPBACK",
            "SIP_MC", "DIP_LINK_LOCAL"
        ];

        for reason in &reasons {
            config.add_drop_reason(reason.to_string());
        }

        assert!(orch.create_debug_counter(config).is_ok());

        let entry = orch.get_counter("full_counter").unwrap();
        assert_eq!(entry.drop_reason_count(), reasons.len());
    }

    #[test]
    fn test_removing_nonexistent_counter() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.remove_debug_counter("does_not_exist");
        assert!(result.is_err());

        // Stats should not be updated
        assert_eq!(orch.stats().counters_removed, 0);
    }

    #[test]
    fn test_multiple_counters_same_drop_reason() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config1 = DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops);
        config1.add_drop_reason("L3_ANY".to_string());

        let mut config2 = DebugCounterConfig::new("counter2".to_string(), DebugCounterType::SwitchIngressDrops);
        config2.add_drop_reason("L3_ANY".to_string());

        assert!(orch.create_debug_counter(config1).is_ok());
        assert!(orch.create_debug_counter(config2).is_ok());

        let entry1 = orch.get_counter("counter1").unwrap();
        let entry2 = orch.get_counter("counter2").unwrap();

        assert!(entry1.drop_reasons.contains("L3_ANY"));
        assert!(entry2.drop_reasons.contains("L3_ANY"));
        assert_eq!(orch.counter_count(), 2);
    }

    #[test]
    fn test_reconcile_with_nonexistent_counter() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.reconcile_drop_reasons("nonexistent");
        assert!(result.is_err());
        match result {
            Err(DebugCounterOrchError::CounterNotFound(name)) => assert_eq!(name, "nonexistent"),
            _ => panic!("Expected CounterNotFound error"),
        }
    }

    #[test]
    fn test_free_counter_tracking_multiple() {
        let mut orch = DebugCounterOrch::new(DebugCounterOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Create counters with drop reasons
        for i in 1..=3 {
            let mut config = DebugCounterConfig::new(
                format!("counter_{}", i),
                DebugCounterType::PortIngressDrops
            );
            config.add_drop_reason("L3_ANY".to_string());
            orch.create_debug_counter(config).unwrap();
        }

        // Remove all drop reasons
        for i in 1..=3 {
            orch.remove_drop_reason(&format!("counter_{}", i), "L3_ANY").unwrap();
        }

        assert_eq!(orch.get_free_counters().len(), 3);

        // Verify counter names in free counters
        let free_counters = orch.get_free_counters();
        for i in 1..=3 {
            assert!(free_counters.iter().any(|fc| fc.name == format!("counter_{}", i)));
        }
    }
}
