//! Debug counter orchestration logic.

use super::types::{DebugCounterConfig, DebugCounterEntry, DebugCounterType, DropReason, FreeCounter};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;

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
            return Err(DebugCounterOrchError::CounterExists(config.name.clone()));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| DebugCounterOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let counter_id = callbacks.create_debug_counter(config.counter_type)
            .map_err(DebugCounterOrchError::SaiError)?;

        let mut entry = DebugCounterEntry::new(config.name.clone(), config.counter_type, counter_id);
        entry.description = config.description;

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

        self.debug_counters.insert(config.name.clone(), entry);
        self.stats.counters_created += 1;

        Ok(())
    }

    pub fn remove_debug_counter(&mut self, name: &str) -> Result<(), DebugCounterOrchError> {
        let entry = self.debug_counters.remove(name)
            .ok_or_else(|| DebugCounterOrchError::CounterNotFound(name.to_string()))?;

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
        for reason in current_reasons {
            if !available_reasons.contains(&reason) {
                self.remove_drop_reason(counter_name, &reason)?;
            }
        }

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
}
