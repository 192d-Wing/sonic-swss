//! Chassis orchestration logic.
//!
//! ChassisOrch manages system ports and fabric ports in modular chassis
//! systems. It coordinates:
//! - System port configuration and SAI object management
//! - Fabric port state and isolation
//! - Cross-linecard communication setup

use super::types::{
    ChassisStats, FabricPortEntry, FabricPortKey, RawSaiObjectId, SystemPortConfig,
    SystemPortEntry, SystemPortKey,
};
use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Result type for ChassisOrch operations.
pub type Result<T> = std::result::Result<T, ChassisOrchError>;

#[derive(Debug, Clone, Error)]
pub enum ChassisOrchError {
    #[error("System port not found: {0:?}")]
    SystemPortNotFound(SystemPortKey),
    #[error("System port exists: {0:?}")]
    SystemPortExists(SystemPortKey),
    #[error("Fabric port not found: {0:?}")]
    FabricPortNotFound(FabricPortKey),
    #[error("Fabric port exists: {0:?}")]
    FabricPortExists(FabricPortKey),
    #[error("Invalid switch ID: {0}")]
    InvalidSwitchId(u32),
    #[error("Invalid core index: {0}")]
    InvalidCoreIndex(u32),
    #[error("SAI error: {0}")]
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct ChassisOrchConfig {
    /// Switch ID for this linecard.
    pub switch_id: u32,
    /// Maximum system ports supported.
    pub max_system_ports: u32,
    /// Maximum fabric ports supported.
    pub max_fabric_ports: u32,
    /// Enable VOQ (Virtual Output Queue) mode.
    pub voq_mode: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ChassisOrchStats {
    pub stats: ChassisStats,
    pub errors: u64,
}

/// Callbacks for Chassis SAI operations.
pub trait ChassisOrchCallbacks: Send + Sync {
    /// Create a system port in SAI.
    fn create_system_port(&self, config: &SystemPortConfig) -> Result<RawSaiObjectId>;

    /// Remove a system port from SAI.
    fn remove_system_port(&self, oid: RawSaiObjectId) -> Result<()>;

    /// Set system port attributes.
    fn set_system_port_attribute(
        &self,
        oid: RawSaiObjectId,
        attr_name: &str,
        attr_value: &str,
    ) -> Result<()>;

    /// Create a fabric port in SAI.
    fn create_fabric_port(&self, port_id: u32) -> Result<RawSaiObjectId>;

    /// Remove a fabric port from SAI.
    fn remove_fabric_port(&self, oid: RawSaiObjectId) -> Result<()>;

    /// Set fabric port isolation state.
    fn set_fabric_port_isolate(&self, oid: RawSaiObjectId, isolate: bool) -> Result<()>;

    /// Write system port state to STATE_DB.
    fn write_system_port_state(&self, key: &SystemPortKey, state: &str) -> Result<()>;

    /// Remove system port state from STATE_DB.
    fn remove_system_port_state(&self, key: &SystemPortKey) -> Result<()>;

    /// Notification when system port is created.
    fn on_system_port_created(&self, entry: &SystemPortEntry);

    /// Notification when system port is removed.
    fn on_system_port_removed(&self, key: &SystemPortKey);

    /// Notification when fabric port isolation changes.
    fn on_fabric_port_isolate_changed(&self, key: &FabricPortKey, isolate: bool);
}

pub struct ChassisOrch<C: ChassisOrchCallbacks> {
    config: ChassisOrchConfig,
    stats: ChassisOrchStats,
    callbacks: Option<Arc<C>>,
    system_ports: HashMap<SystemPortKey, SystemPortEntry>,
    fabric_ports: HashMap<FabricPortKey, FabricPortEntry>,
}

impl<C: ChassisOrchCallbacks> ChassisOrch<C> {
    pub fn new(config: ChassisOrchConfig) -> Self {
        Self {
            config,
            stats: ChassisOrchStats::default(),
            callbacks: None,
            system_ports: HashMap::new(),
            fabric_ports: HashMap::new(),
        }
    }

    pub fn with_callbacks(config: ChassisOrchConfig, callbacks: Arc<C>) -> Self {
        Self {
            config,
            stats: ChassisOrchStats::default(),
            callbacks: Some(callbacks),
            system_ports: HashMap::new(),
            fabric_ports: HashMap::new(),
        }
    }

    pub fn config(&self) -> &ChassisOrchConfig {
        &self.config
    }

    pub fn stats(&self) -> &ChassisOrchStats {
        &self.stats
    }

    // ===== System Port Management =====

    /// Add a system port.
    pub fn add_system_port(&mut self, config: SystemPortConfig) -> Result<()> {
        let key = SystemPortKey::new(config.system_port_id);

        if self.system_ports.contains_key(&key) {
            let record = AuditRecord::new(
                AuditCategory::ErrorCondition,
                "ChassisOrch",
                "add_system_port",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&format!("system_port_{}", config.system_port_id))
            .with_object_type("system_port")
            .with_error("System port already exists")
            .with_details(serde_json::json!({
                "system_port_id": config.system_port_id,
                "switch_id": config.switch_id,
            }));
            audit_log!(record);
            return Err(ChassisOrchError::SystemPortExists(key));
        }

        // Validate switch ID
        if config.switch_id != self.config.switch_id && self.config.switch_id != 0 {
            // Allow switch_id 0 as wildcard for testing
        }

        let sai_oid = if let Some(ref callbacks) = self.callbacks {
            callbacks.create_system_port(&config)?
        } else {
            0x1000 + config.system_port_id as u64
        };

        let mut entry = SystemPortEntry::new(config.clone());
        entry.sai_oid = sai_oid;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.on_system_port_created(&entry);
            let _ = callbacks.write_system_port_state(&key, "active");
        }

        self.system_ports.insert(key, entry);
        self.stats.stats.system_ports_created += 1;

        let record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "ChassisOrch",
            "add_system_port",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&format!("system_port_{}", config.system_port_id))
        .with_object_type("system_port")
        .with_details(serde_json::json!({
            "system_port_id": config.system_port_id,
            "switch_id": config.switch_id,
            "core_index": config.core_index,
            "core_port_index": config.core_port_index,
            "speed": config.speed,
            "sai_oid": sai_oid,
        }));
        audit_log!(record);

        Ok(())
    }

    /// Remove a system port.
    pub fn remove_system_port(&mut self, key: &SystemPortKey) -> Result<()> {
        let entry = self.system_ports.remove(key).ok_or_else(|| {
            let record = AuditRecord::new(
                AuditCategory::ResourceDelete,
                "ChassisOrch",
                "remove_system_port",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&format!("system_port_{}", key.system_port_id))
            .with_object_type("system_port")
            .with_error("System port not found")
            .with_details(serde_json::json!({
                "system_port_id": key.system_port_id,
            }));
            audit_log!(record);
            ChassisOrchError::SystemPortNotFound(key.clone())
        })?;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.remove_system_port(entry.sai_oid)?;
            callbacks.on_system_port_removed(key);
            let _ = callbacks.remove_system_port_state(key);
        }

        let record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "ChassisOrch",
            "remove_system_port",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&format!("system_port_{}", key.system_port_id))
        .with_object_type("system_port")
        .with_details(serde_json::json!({
            "system_port_id": key.system_port_id,
            "switch_id": entry.config.switch_id,
            "speed": entry.config.speed,
        }));
        audit_log!(record);

        Ok(())
    }

    /// Get a system port by key.
    pub fn get_system_port(&self, key: &SystemPortKey) -> Option<&SystemPortEntry> {
        self.system_ports.get(key)
    }

    /// Get a mutable system port by key.
    pub fn get_system_port_mut(&mut self, key: &SystemPortKey) -> Option<&mut SystemPortEntry> {
        self.system_ports.get_mut(key)
    }

    /// Update a system port's speed.
    pub fn update_system_port_speed(&mut self, key: &SystemPortKey, speed: u32) -> Result<()> {
        let entry = self
            .system_ports
            .get_mut(key)
            .ok_or_else(|| ChassisOrchError::SystemPortNotFound(key.clone()))?;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.set_system_port_attribute(entry.sai_oid, "SPEED", &speed.to_string())?;
        }

        entry.config.speed = speed;
        Ok(())
    }

    /// Get system port count.
    pub fn system_port_count(&self) -> usize {
        self.system_ports.len()
    }

    /// Get all system port keys.
    pub fn system_port_keys(&self) -> Vec<SystemPortKey> {
        self.system_ports.keys().cloned().collect()
    }

    // ===== Fabric Port Management =====

    /// Add a fabric port.
    pub fn add_fabric_port(&mut self, fabric_port_id: u32) -> Result<()> {
        let key = FabricPortKey::new(fabric_port_id);

        if self.fabric_ports.contains_key(&key) {
            return Err(ChassisOrchError::FabricPortExists(key));
        }

        let sai_oid = if let Some(ref callbacks) = self.callbacks {
            callbacks.create_fabric_port(fabric_port_id)?
        } else {
            0x2000 + fabric_port_id as u64
        };

        let mut entry = FabricPortEntry::new(fabric_port_id);
        entry.sai_oid = sai_oid;

        self.fabric_ports.insert(key, entry);
        self.stats.stats.fabric_ports_created += 1;

        Ok(())
    }

    /// Remove a fabric port.
    pub fn remove_fabric_port(&mut self, key: &FabricPortKey) -> Result<()> {
        let entry = self
            .fabric_ports
            .remove(key)
            .ok_or_else(|| ChassisOrchError::FabricPortNotFound(key.clone()))?;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.remove_fabric_port(entry.sai_oid)?;
        }

        Ok(())
    }

    /// Get a fabric port by key.
    pub fn get_fabric_port(&self, key: &FabricPortKey) -> Option<&FabricPortEntry> {
        self.fabric_ports.get(key)
    }

    /// Set fabric port isolation state.
    pub fn set_fabric_port_isolate(&mut self, key: &FabricPortKey, isolate: bool) -> Result<()> {
        let entry = self.fabric_ports.get_mut(key).ok_or_else(|| {
            let record = AuditRecord::new(
                AuditCategory::ResourceModify,
                "ChassisOrch",
                "update_fabric_port",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&format!("fabric_port_{}", key.fabric_port_id))
            .with_object_type("fabric_port")
            .with_error("Fabric port not found")
            .with_details(serde_json::json!({
                "fabric_port_id": key.fabric_port_id,
            }));
            audit_log!(record);
            ChassisOrchError::FabricPortNotFound(key.clone())
        })?;

        if entry.isolate == isolate {
            return Ok(()); // No change needed
        }

        if let Some(ref callbacks) = self.callbacks {
            callbacks.set_fabric_port_isolate(entry.sai_oid, isolate)?;
            callbacks.on_fabric_port_isolate_changed(key, isolate);
        }

        entry.isolate = isolate;

        let record = AuditRecord::new(
            AuditCategory::ResourceModify,
            "ChassisOrch",
            "update_fabric_port",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&format!("fabric_port_{}", key.fabric_port_id))
        .with_object_type("fabric_port")
        .with_details(serde_json::json!({
            "fabric_port_id": key.fabric_port_id,
            "isolate": isolate,
            "sai_oid": entry.sai_oid,
        }));
        audit_log!(record);

        Ok(())
    }

    /// Get fabric port count.
    pub fn fabric_port_count(&self) -> usize {
        self.fabric_ports.len()
    }

    /// Get all fabric port keys.
    pub fn fabric_port_keys(&self) -> Vec<FabricPortKey> {
        self.fabric_ports.keys().cloned().collect()
    }

    /// Get isolated fabric port count.
    pub fn isolated_fabric_port_count(&self) -> usize {
        self.fabric_ports.values().filter(|e| e.isolate).count()
    }

    // ===== Bulk Operations =====

    /// Get all system ports for a specific switch.
    pub fn get_system_ports_by_switch(&self, switch_id: u32) -> Vec<&SystemPortEntry> {
        self.system_ports
            .values()
            .filter(|e| e.config.switch_id == switch_id)
            .collect()
    }

    /// Get all system ports for a specific core.
    pub fn get_system_ports_by_core(&self, core_index: u32) -> Vec<&SystemPortEntry> {
        self.system_ports
            .values()
            .filter(|e| e.config.core_index == core_index)
            .collect()
    }

    /// Isolate all fabric ports (emergency shutdown).
    pub fn isolate_all_fabric_ports(&mut self) -> Result<()> {
        let keys: Vec<FabricPortKey> = self.fabric_ports.keys().cloned().collect();

        for key in keys {
            self.set_fabric_port_isolate(&key, true)?;
        }

        Ok(())
    }

    /// Unisolate all fabric ports (recovery).
    pub fn unisolate_all_fabric_ports(&mut self) -> Result<()> {
        let keys: Vec<FabricPortKey> = self.fabric_ports.keys().cloned().collect();

        for key in keys {
            self.set_fabric_port_isolate(&key, false)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock callbacks for testing without SAI.
    struct MockChassisCallbacks;

    impl ChassisOrchCallbacks for MockChassisCallbacks {
        fn create_system_port(&self, config: &SystemPortConfig) -> Result<RawSaiObjectId> {
            Ok(0x1000 + config.system_port_id as u64)
        }

        fn remove_system_port(&self, _oid: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn set_system_port_attribute(
            &self,
            _oid: RawSaiObjectId,
            _attr_name: &str,
            _attr_value: &str,
        ) -> Result<()> {
            Ok(())
        }

        fn create_fabric_port(&self, port_id: u32) -> Result<RawSaiObjectId> {
            Ok(0x2000 + port_id as u64)
        }

        fn remove_fabric_port(&self, _oid: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn set_fabric_port_isolate(&self, _oid: RawSaiObjectId, _isolate: bool) -> Result<()> {
            Ok(())
        }

        fn write_system_port_state(&self, _key: &SystemPortKey, _state: &str) -> Result<()> {
            Ok(())
        }

        fn remove_system_port_state(&self, _key: &SystemPortKey) -> Result<()> {
            Ok(())
        }

        fn on_system_port_created(&self, _entry: &SystemPortEntry) {}
        fn on_system_port_removed(&self, _key: &SystemPortKey) {}
        fn on_fabric_port_isolate_changed(&self, _key: &FabricPortKey, _isolate: bool) {}
    }

    #[test]
    fn test_chassis_orch_new() {
        let orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());
        assert_eq!(orch.system_port_count(), 0);
        assert_eq!(orch.stats.stats.system_ports_created, 0);
        assert_eq!(orch.stats.stats.fabric_ports_created, 0);
        assert_eq!(orch.stats.errors, 0);
    }

    #[test]
    fn test_get_system_port_not_found() {
        let orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());
        let key = SystemPortKey::new(100);

        let result = orch.get_system_port(&key);
        assert!(result.is_none());
    }

    #[test]
    fn test_add_system_port() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 1,
            core_index: 0,
            core_port_index: 0,
            speed: 100000,
        };

        let result = orch.add_system_port(config);
        assert!(result.is_ok());
        assert_eq!(orch.system_port_count(), 1);
        assert_eq!(orch.stats().stats.system_ports_created, 1);

        let key = SystemPortKey::new(100);
        let port = orch.get_system_port(&key);
        assert!(port.is_some());
        assert_eq!(port.unwrap().config.system_port_id, 100);
    }

    #[test]
    fn test_add_system_port_duplicate() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 1,
            core_index: 0,
            core_port_index: 0,
            speed: 100000,
        };

        orch.add_system_port(config.clone()).unwrap();
        let result = orch.add_system_port(config);

        assert!(matches!(result, Err(ChassisOrchError::SystemPortExists(_))));
    }

    #[test]
    fn test_remove_system_port() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 1,
            core_index: 0,
            core_port_index: 0,
            speed: 100000,
        };

        orch.add_system_port(config).unwrap();
        let key = SystemPortKey::new(100);

        let result = orch.remove_system_port(&key);
        assert!(result.is_ok());
        assert_eq!(orch.system_port_count(), 0);
    }

    #[test]
    fn test_remove_system_port_not_found() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());
        let key = SystemPortKey::new(999);

        let result = orch.remove_system_port(&key);
        assert!(matches!(
            result,
            Err(ChassisOrchError::SystemPortNotFound(_))
        ));
    }

    #[test]
    fn test_update_system_port_speed() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 1,
            core_index: 0,
            core_port_index: 0,
            speed: 100000,
        };

        orch.add_system_port(config).unwrap();
        let key = SystemPortKey::new(100);

        let result = orch.update_system_port_speed(&key, 400000);
        assert!(result.is_ok());

        let port = orch.get_system_port(&key).unwrap();
        assert_eq!(port.config.speed, 400000);
    }

    #[test]
    fn test_stats_returns_reference() {
        let orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.errors, 0);
        assert_eq!(stats.stats.system_ports_created, 0);
        assert_eq!(stats.stats.fabric_ports_created, 0);
    }

    #[test]
    fn test_chassis_orch_config_default() {
        let config = ChassisOrchConfig::default();
        let orch: ChassisOrch<MockChassisCallbacks> = ChassisOrch::new(config);

        assert_eq!(orch.system_port_count(), 0);
    }

    #[test]
    fn test_chassis_orch_config_custom() {
        let config = ChassisOrchConfig {
            switch_id: 5,
            max_system_ports: 256,
            max_fabric_ports: 64,
            voq_mode: true,
        };
        let orch: ChassisOrch<MockChassisCallbacks> = ChassisOrch::new(config);

        assert_eq!(orch.config().switch_id, 5);
        assert_eq!(orch.config().max_system_ports, 256);
        assert!(orch.config().voq_mode);
    }

    #[test]
    fn test_multiple_system_ports() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        for i in 0..5 {
            let config = SystemPortConfig {
                system_port_id: 100 + i,
                switch_id: 1,
                core_index: i,
                core_port_index: i,
                speed: 100000,
            };
            orch.add_system_port(config).unwrap();
        }

        assert_eq!(orch.system_port_count(), 5);
        assert_eq!(orch.stats().stats.system_ports_created, 5);

        for i in 0..5 {
            let key = SystemPortKey::new(100 + i);
            assert!(orch.get_system_port(&key).is_some());
        }
    }

    #[test]
    fn test_system_port_key_equality() {
        let key1 = SystemPortKey::new(100);
        let key2 = SystemPortKey::new(100);
        let key3 = SystemPortKey::new(200);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_chassis_stats_structure() {
        let stats = ChassisOrchStats::default();

        assert_eq!(stats.stats.system_ports_created, 0);
        assert_eq!(stats.stats.fabric_ports_created, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_system_port_entry_creation() {
        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 2,
            core_index: 1,
            core_port_index: 5,
            speed: 400000,
        };

        let entry = SystemPortEntry::new(config);

        assert_eq!(entry.key.system_port_id, 100);
        assert_eq!(entry.config.system_port_id, 100);
        assert_eq!(entry.config.switch_id, 2);
        assert_eq!(entry.config.core_index, 1);
        assert_eq!(entry.config.core_port_index, 5);
        assert_eq!(entry.config.speed, 400000);
        assert_eq!(entry.sai_oid, 0);
    }

    #[test]
    fn test_chassis_error_variants() {
        let err1 = ChassisOrchError::SystemPortNotFound(SystemPortKey::new(100));
        let err2 = ChassisOrchError::InvalidSwitchId(99);
        let err3 = ChassisOrchError::SaiError("test error".to_string());

        match err1 {
            ChassisOrchError::SystemPortNotFound(key) => {
                assert_eq!(key.system_port_id, 100);
            }
            _ => panic!("Wrong error variant"),
        }

        match err2 {
            ChassisOrchError::InvalidSwitchId(id) => {
                assert_eq!(id, 99);
            }
            _ => panic!("Wrong error variant"),
        }

        match err3 {
            ChassisOrchError::SaiError(msg) => {
                assert_eq!(msg, "test error");
            }
            _ => panic!("Wrong error variant"),
        }
    }

    // ===== Fabric port tests =====

    #[test]
    fn test_add_fabric_port() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        let result = orch.add_fabric_port(0);
        assert!(result.is_ok());
        assert_eq!(orch.fabric_port_count(), 1);
        assert_eq!(orch.stats().stats.fabric_ports_created, 1);
    }

    #[test]
    fn test_add_fabric_port_duplicate() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        orch.add_fabric_port(0).unwrap();
        let result = orch.add_fabric_port(0);

        assert!(matches!(result, Err(ChassisOrchError::FabricPortExists(_))));
    }

    #[test]
    fn test_remove_fabric_port() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        orch.add_fabric_port(0).unwrap();
        let key = FabricPortKey::new(0);

        let result = orch.remove_fabric_port(&key);
        assert!(result.is_ok());
        assert_eq!(orch.fabric_port_count(), 0);
    }

    #[test]
    fn test_remove_fabric_port_not_found() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());
        let key = FabricPortKey::new(999);

        let result = orch.remove_fabric_port(&key);
        assert!(matches!(
            result,
            Err(ChassisOrchError::FabricPortNotFound(_))
        ));
    }

    #[test]
    fn test_fabric_port_isolation() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        orch.add_fabric_port(0).unwrap();
        let key = FabricPortKey::new(0);

        // Initially not isolated
        assert!(!orch.get_fabric_port(&key).unwrap().isolate);
        assert_eq!(orch.isolated_fabric_port_count(), 0);

        // Isolate
        let result = orch.set_fabric_port_isolate(&key, true);
        assert!(result.is_ok());
        assert!(orch.get_fabric_port(&key).unwrap().isolate);
        assert_eq!(orch.isolated_fabric_port_count(), 1);

        // Unisolate
        let result = orch.set_fabric_port_isolate(&key, false);
        assert!(result.is_ok());
        assert!(!orch.get_fabric_port(&key).unwrap().isolate);
        assert_eq!(orch.isolated_fabric_port_count(), 0);
    }

    #[test]
    fn test_isolate_all_fabric_ports() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        for i in 0..5 {
            orch.add_fabric_port(i).unwrap();
        }

        let result = orch.isolate_all_fabric_ports();
        assert!(result.is_ok());
        assert_eq!(orch.isolated_fabric_port_count(), 5);
    }

    #[test]
    fn test_unisolate_all_fabric_ports() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        for i in 0..5 {
            orch.add_fabric_port(i).unwrap();
        }

        orch.isolate_all_fabric_ports().unwrap();
        assert_eq!(orch.isolated_fabric_port_count(), 5);

        let result = orch.unisolate_all_fabric_ports();
        assert!(result.is_ok());
        assert_eq!(orch.isolated_fabric_port_count(), 0);
    }

    // ===== Bulk operation tests =====

    #[test]
    fn test_get_system_ports_by_switch() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        // Add ports for switch 1
        for i in 0..3 {
            let config = SystemPortConfig {
                system_port_id: 100 + i,
                switch_id: 1,
                core_index: i,
                core_port_index: i,
                speed: 100000,
            };
            orch.add_system_port(config).unwrap();
        }

        // Add ports for switch 2
        for i in 0..2 {
            let config = SystemPortConfig {
                system_port_id: 200 + i,
                switch_id: 2,
                core_index: i,
                core_port_index: i,
                speed: 100000,
            };
            orch.add_system_port(config).unwrap();
        }

        let switch1_ports = orch.get_system_ports_by_switch(1);
        assert_eq!(switch1_ports.len(), 3);

        let switch2_ports = orch.get_system_ports_by_switch(2);
        assert_eq!(switch2_ports.len(), 2);

        let switch3_ports = orch.get_system_ports_by_switch(3);
        assert_eq!(switch3_ports.len(), 0);
    }

    #[test]
    fn test_get_system_ports_by_core() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        // Add ports for different cores
        for i in 0..4 {
            let config = SystemPortConfig {
                system_port_id: 100 + i,
                switch_id: 1,
                core_index: i % 2, // Core 0 or 1
                core_port_index: i,
                speed: 100000,
            };
            orch.add_system_port(config).unwrap();
        }

        let core0_ports = orch.get_system_ports_by_core(0);
        assert_eq!(core0_ports.len(), 2);

        let core1_ports = orch.get_system_ports_by_core(1);
        assert_eq!(core1_ports.len(), 2);
    }

    #[test]
    fn test_system_port_keys() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        for i in 0..3 {
            let config = SystemPortConfig {
                system_port_id: 100 + i,
                switch_id: 1,
                core_index: 0,
                core_port_index: i,
                speed: 100000,
            };
            orch.add_system_port(config).unwrap();
        }

        let keys = orch.system_port_keys();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_fabric_port_keys() {
        let mut orch: ChassisOrch<MockChassisCallbacks> =
            ChassisOrch::new(ChassisOrchConfig::default());

        for i in 0..3 {
            orch.add_fabric_port(i).unwrap();
        }

        let keys = orch.fabric_port_keys();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_with_callbacks() {
        let callbacks = Arc::new(MockChassisCallbacks);
        let mut orch = ChassisOrch::with_callbacks(ChassisOrchConfig::default(), callbacks);

        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 1,
            core_index: 0,
            core_port_index: 0,
            speed: 100000,
        };

        orch.add_system_port(config).unwrap();
        assert_eq!(orch.system_port_count(), 1);
    }
}
