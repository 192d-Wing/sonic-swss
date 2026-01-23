//! CoPP orchestration logic.

use super::types::{CoppStats, CoppTrapEntry, CoppTrapKey, CoppTrapConfig, RawSaiObjectId};
use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::{audit_log, debug_log, info_log, warn_log, error_log};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoppOrchError>;

/// CoPP orchestration errors with NIST-compliant error messages.
#[derive(Debug, Clone, Error)]
pub enum CoppOrchError {
    /// Trap with the specified key was not found
    #[error("CoPP trap not found: {0:?}")]
    TrapNotFound(CoppTrapKey),

    /// Trap with the specified key already exists
    #[error("CoPP trap already exists: {0:?}")]
    TrapExists(CoppTrapKey),

    /// Invalid CPU queue number (must be 0-7)
    #[error("Invalid CPU queue {0}: must be in range 0-7")]
    InvalidQueue(u8),

    /// Invalid rate limit value (must be non-zero)
    #[error("Invalid rate limit {0}: CIR and CBS must be non-zero")]
    InvalidRate(u64),

    /// SAI operation failed
    #[error("SAI operation failed: {0}")]
    SaiError(String),

    /// Callbacks not configured
    #[error("CoPP orchestrator not initialized: callbacks not configured")]
    NotInitialized,
}

#[derive(Debug, Clone, Default)]
pub struct CoppOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct CoppOrchStats {
    pub stats: CoppStats,
    pub errors: u64,
    pub dropped_packets: u64,
    pub rate_limited_packets: u64,
}

pub trait CoppOrchCallbacks: Send + Sync {
    fn create_trap(&self, key: &CoppTrapKey, config: &CoppTrapConfig) -> Result<RawSaiObjectId>;
    fn remove_trap(&self, trap_id: RawSaiObjectId) -> Result<()>;
    fn update_trap_rate(&self, trap_id: RawSaiObjectId, cir: u64, cbs: u64) -> Result<()>;
    fn get_trap_stats(&self, trap_id: RawSaiObjectId) -> Result<(u64, u64)>;
    fn on_trap_created(&self, key: &CoppTrapKey, trap_id: RawSaiObjectId);
    fn on_trap_removed(&self, key: &CoppTrapKey);
}

pub struct CoppOrch<C: CoppOrchCallbacks> {
    #[allow(dead_code)]
    config: CoppOrchConfig,
    stats: CoppOrchStats,
    traps: HashMap<CoppTrapKey, CoppTrapEntry>,
    callbacks: Option<Arc<C>>,
}

impl<C: CoppOrchCallbacks> CoppOrch<C> {
    pub fn new(config: CoppOrchConfig) -> Self {
        Self {
            config,
            stats: CoppOrchStats::default(),
            traps: HashMap::new(),
            callbacks: None,
        }
    }

    pub fn with_callbacks(mut self, callbacks: Arc<C>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    pub fn add_trap(&mut self, key: CoppTrapKey, config: CoppTrapConfig) -> Result<RawSaiObjectId> {
        debug_log!("CoppOrch", trap_id = %key.trap_id, "Adding CoPP trap");

        if self.traps.contains_key(&key) {
            warn_log!("CoppOrch", trap_id = %key.trap_id, "Trap already exists");
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "CoppOrch",
                "add_trap"
            )
            .with_object_id(&key.trap_id)
            .with_object_type("copp_trap")
            .with_error(format!("Trap already exists: {}", key.trap_id)));
            return Err(CoppOrchError::TrapExists(key));
        }

        if let Some(queue) = config.queue {
            if queue >= 8 {
                error_log!("CoppOrch", trap_id = %key.trap_id, queue = queue, "Invalid CPU queue number");
                audit_log!(AuditRecord::new(
                    AuditCategory::ErrorCondition,
                    "CoppOrch",
                    "add_trap"
                )
                .with_object_id(&key.trap_id)
                .with_object_type("copp_trap")
                .with_error(format!("Invalid queue {}: must be 0-7", queue)));
                return Err(CoppOrchError::InvalidQueue(queue));
            }
        }

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("CoppOrch", "Callbacks not configured");
            CoppOrchError::NotInitialized
        })?;

        let trap_id = callbacks.create_trap(&key, &config).map_err(|e| {
            error_log!("CoppOrch", trap_id = %key.trap_id, error = %e, "SAI create_trap failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "CoppOrch",
                "create_trap"
            )
            .with_object_id(&key.trap_id)
            .with_object_type("copp_trap")
            .with_error(e.to_string()));
            e
        })?;

        let mut entry = CoppTrapEntry::new(key.clone(), config.clone());
        entry.trap_oid = trap_id;

        self.traps.insert(key.clone(), entry);
        self.stats.stats.traps_created += 1;

        callbacks.on_trap_created(&key, trap_id);

        info_log!("CoppOrch", trap_id = %key.trap_id, oid = trap_id, queue = ?config.queue, "CoPP trap created successfully");
        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "CoppOrch",
            "add_trap"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("0x{:x}", trap_id))
        .with_object_type("copp_trap")
        .with_details(serde_json::json!({
            "trap_key": key.trap_id,
            "queue": config.queue,
            "action": format!("{:?}", config.trap_action)
        })));

        Ok(trap_id)
    }

    pub fn remove_trap(&mut self, key: &CoppTrapKey) -> Result<()> {
        debug_log!("CoppOrch", trap_id = %key.trap_id, "Removing CoPP trap");

        let entry = self.traps.remove(key).ok_or_else(|| {
            warn_log!("CoppOrch", trap_id = %key.trap_id, "Trap not found for removal");
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceDelete,
                "CoppOrch",
                "remove_trap"
            )
            .with_object_id(&key.trap_id)
            .with_object_type("copp_trap")
            .with_error("Trap not found"));
            CoppOrchError::TrapNotFound(key.clone())
        })?;

        let trap_oid = entry.trap_oid;
        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("CoppOrch", "Callbacks not configured");
            CoppOrchError::NotInitialized
        })?;

        callbacks.remove_trap(trap_oid).map_err(|e| {
            error_log!("CoppOrch", trap_id = %key.trap_id, oid = trap_oid, error = %e, "SAI remove_trap failed");
            // Re-insert the entry since removal failed
            self.traps.insert(key.clone(), entry.clone());
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "CoppOrch",
                "remove_trap"
            )
            .with_object_id(format!("0x{:x}", trap_oid))
            .with_object_type("copp_trap")
            .with_error(e.to_string()));
            e
        })?;

        self.stats.stats.traps_created = self.stats.stats.traps_created.saturating_sub(1);
        callbacks.on_trap_removed(key);

        info_log!("CoppOrch", trap_id = %key.trap_id, oid = trap_oid, "CoPP trap removed successfully");
        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "CoppOrch",
            "remove_trap"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("0x{:x}", trap_oid))
        .with_object_type("copp_trap")
        .with_details(serde_json::json!({
            "trap_key": key.trap_id
        })));

        Ok(())
    }

    pub fn update_trap_rate(&mut self, key: &CoppTrapKey, cir: u64, cbs: u64) -> Result<()> {
        debug_log!("CoppOrch", trap_id = %key.trap_id, cir = cir, cbs = cbs, "Updating CoPP trap rate");

        if cir == 0 || cbs == 0 {
            error_log!("CoppOrch", trap_id = %key.trap_id, cir = cir, cbs = cbs, "Invalid rate limit values");
            audit_log!(AuditRecord::new(
                AuditCategory::ConfigurationChange,
                "CoppOrch",
                "update_trap_rate"
            )
            .with_object_id(&key.trap_id)
            .with_object_type("copp_trap")
            .with_error(format!("Invalid rate: CIR={}, CBS={} (must be non-zero)", cir, cbs)));
            return Err(CoppOrchError::InvalidRate(cir));
        }

        let entry = self.traps.get_mut(key).ok_or_else(|| {
            warn_log!("CoppOrch", trap_id = %key.trap_id, "Trap not found for rate update");
            CoppOrchError::TrapNotFound(key.clone())
        })?;

        let trap_oid = entry.trap_oid;
        let old_cir = entry.config.cir;
        let old_cbs = entry.config.cbs;

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("CoppOrch", "Callbacks not configured");
            CoppOrchError::NotInitialized
        })?;

        callbacks.update_trap_rate(trap_oid, cir, cbs).map_err(|e| {
            error_log!("CoppOrch", trap_id = %key.trap_id, oid = trap_oid, error = %e, "SAI update_trap_rate failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "CoppOrch",
                "update_trap_rate"
            )
            .with_object_id(format!("0x{:x}", trap_oid))
            .with_object_type("copp_trap")
            .with_error(e.to_string()));
            e
        })?;

        entry.config.cir = Some(cir);
        entry.config.cbs = Some(cbs);

        info_log!("CoppOrch", trap_id = %key.trap_id, oid = trap_oid, old_cir = ?old_cir, old_cbs = ?old_cbs, new_cir = cir, new_cbs = cbs, "CoPP trap rate updated successfully");
        audit_log!(AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "CoppOrch",
            "update_trap_rate"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("0x{:x}", trap_oid))
        .with_object_type("copp_trap")
        .with_details(serde_json::json!({
            "trap_key": key.trap_id,
            "old_cir": old_cir,
            "old_cbs": old_cbs,
            "new_cir": cir,
            "new_cbs": cbs
        })));

        Ok(())
    }

    pub fn get_trap(&self, key: &CoppTrapKey) -> Option<&CoppTrapEntry> {
        self.traps.get(key)
    }

    pub fn get_all_traps(&self) -> Vec<&CoppTrapEntry> {
        self.traps.values().collect()
    }

    pub fn trap_exists(&self, key: &CoppTrapKey) -> bool {
        self.traps.contains_key(key)
    }

    pub fn trap_count(&self) -> usize {
        self.traps.len()
    }

    pub fn stats(&self) -> &CoppOrchStats {
        &self.stats
    }

    pub fn update_stats(&mut self, dropped: u64, rate_limited: u64) {
        self.stats.dropped_packets += dropped;
        self.stats.rate_limited_packets += rate_limited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::CoppTrapAction;

    struct MockCoppCallbacks;

    impl CoppOrchCallbacks for MockCoppCallbacks {
        fn create_trap(&self, _key: &CoppTrapKey, _config: &CoppTrapConfig) -> Result<RawSaiObjectId> {
            Ok(0x1000)
        }

        fn remove_trap(&self, _trap_id: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn update_trap_rate(&self, _trap_id: RawSaiObjectId, _cir: u64, _cbs: u64) -> Result<()> {
            Ok(())
        }

        fn get_trap_stats(&self, _trap_id: RawSaiObjectId) -> Result<(u64, u64)> {
            Ok((0, 0))
        }

        fn on_trap_created(&self, _key: &CoppTrapKey, _trap_id: RawSaiObjectId) {}
        fn on_trap_removed(&self, _key: &CoppTrapKey) {}
    }

    fn create_test_config() -> CoppTrapConfig {
        CoppTrapConfig {
            trap_action: CoppTrapAction::Trap,
            trap_priority: Some(4),
            queue: Some(4),
            meter_type: Some("packets".to_string()),
            mode: Some("sr_tcm".to_string()),
            color: Some("aware".to_string()),
            cbs: Some(600),
            cir: Some(600),
            pbs: Some(600),
            pir: Some(600),
        }
    }

    #[test]
    fn test_copp_orch_new() {
        let orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default());
        assert_eq!(orch.trap_count(), 0);
        assert_eq!(orch.stats().stats.traps_created, 0);
        assert_eq!(orch.stats().errors, 0);
        assert_eq!(orch.stats().dropped_packets, 0);
        assert_eq!(orch.stats().rate_limited_packets, 0);
    }

    #[test]
    fn test_add_trap_success() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let config = create_test_config();

        let result = orch.add_trap(key.clone(), config);
        assert!(result.is_ok());
        assert_eq!(orch.trap_count(), 1);
        assert_eq!(orch.stats().stats.traps_created, 1);
        assert!(orch.trap_exists(&key));
    }

    #[test]
    fn test_add_trap_duplicate() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let config = create_test_config();

        assert!(orch.add_trap(key.clone(), config.clone()).is_ok());
        assert!(orch.add_trap(key, config).is_err());
        assert_eq!(orch.trap_count(), 1);
    }

    #[test]
    fn test_add_trap_invalid_queue() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let mut config = create_test_config();
        config.queue = Some(8);

        let result = orch.add_trap(key, config);
        assert!(result.is_err());
        assert_eq!(orch.trap_count(), 0);
    }

    #[test]
    fn test_remove_trap_success() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let config = create_test_config();

        assert!(orch.add_trap(key.clone(), config).is_ok());
        assert_eq!(orch.trap_count(), 1);

        assert!(orch.remove_trap(&key).is_ok());
        assert_eq!(orch.trap_count(), 0);
        assert!(!orch.trap_exists(&key));
    }

    #[test]
    fn test_remove_trap_not_found() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("nonexistent".to_string());
        let result = orch.remove_trap(&key);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_trap_rate_success() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let config = create_test_config();

        assert!(orch.add_trap(key.clone(), config).is_ok());
        assert!(orch.update_trap_rate(&key, 1000, 1000).is_ok());

        let trap = orch.get_trap(&key).unwrap();
        assert_eq!(trap.config.cir, Some(1000));
        assert_eq!(trap.config.cbs, Some(1000));
    }

    #[test]
    fn test_update_trap_rate_invalid() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let config = create_test_config();

        assert!(orch.add_trap(key.clone(), config).is_ok());
        assert!(orch.update_trap_rate(&key, 0, 1000).is_err());
        assert!(orch.update_trap_rate(&key, 1000, 0).is_err());
    }

    #[test]
    fn test_get_trap_found() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let config = create_test_config();

        assert!(orch.add_trap(key.clone(), config).is_ok());
        let trap = orch.get_trap(&key);
        assert!(trap.is_some());
        assert_eq!(trap.unwrap().key.trap_id, "bgp");
    }

    #[test]
    fn test_get_trap_not_found() {
        let orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default());
        let key = CoppTrapKey::new("nonexistent".to_string());
        assert!(orch.get_trap(&key).is_none());
    }

    #[test]
    fn test_get_all_traps() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let config = create_test_config();
        let trap_names = vec!["bgp", "arp", "lacp"];

        for name in trap_names.iter() {
            let key = CoppTrapKey::new(name.to_string());
            assert!(orch.add_trap(key, config.clone()).is_ok());
        }

        let all_traps = orch.get_all_traps();
        assert_eq!(all_traps.len(), 3);
    }

    #[test]
    fn test_trap_exists() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let key = CoppTrapKey::new("bgp".to_string());
        let config = create_test_config();

        assert!(!orch.trap_exists(&key));
        assert!(orch.add_trap(key.clone(), config).is_ok());
        assert!(orch.trap_exists(&key));
    }

    #[test]
    fn test_stats_tracking() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default());

        assert_eq!(orch.stats().dropped_packets, 0);
        assert_eq!(orch.stats().rate_limited_packets, 0);

        orch.update_stats(100, 50);
        assert_eq!(orch.stats().dropped_packets, 100);
        assert_eq!(orch.stats().rate_limited_packets, 50);

        orch.update_stats(50, 25);
        assert_eq!(orch.stats().dropped_packets, 150);
        assert_eq!(orch.stats().rate_limited_packets, 75);
    }

    #[test]
    fn test_multiple_trap_operations() {
        let mut orch: CoppOrch<MockCoppCallbacks> = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(MockCoppCallbacks));

        let config = create_test_config();
        let trap_names = vec!["bgp", "arp", "lacp", "lldp", "dhcp"];

        for (i, name) in trap_names.iter().enumerate() {
            let key = CoppTrapKey::new(name.to_string());
            let mut cfg = config.clone();
            cfg.queue = Some((i % 8) as u8);
            assert!(orch.add_trap(key, cfg).is_ok());
        }

        assert_eq!(orch.trap_count(), 5);
        assert_eq!(orch.stats().stats.traps_created, 5);

        for name in trap_names.iter() {
            let key = CoppTrapKey::new(name.to_string());
            assert!(orch.trap_exists(&key));
        }

        let bgp_key = CoppTrapKey::new("bgp".to_string());
        assert!(orch.remove_trap(&bgp_key).is_ok());
        assert_eq!(orch.trap_count(), 4);
    }
}
