//! PFC Watchdog orchestration logic.

use super::types::{
    DetectionTime, PfcWdAction, PfcWdConfig, PfcWdEntry, PfcWdStats, RestorationTime,
};
use crate::{
    audit::{AuditCategory, AuditOutcome, AuditRecord},
    audit_log,
};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum PfcWdOrchError {
    #[error("Queue already exists: {0}")]
    QueueExists(String),
    #[error("Queue not found: {0}")]
    QueueNotFound(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("SAI error: {0}")]
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct PfcWdOrchConfig {
    pub poll_interval_ms: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PfcWdOrchStats {
    pub queues_registered: u64,
    pub queues_unregistered: u64,
    pub storms_detected: u64,
    pub storms_restored: u64,
}

pub trait PfcWdOrchCallbacks: Send + Sync {
    fn create_watchdog(&self, config: &PfcWdConfig) -> Result<RawSaiObjectId, String>;
    fn remove_watchdog(&self, wd_id: RawSaiObjectId) -> Result<(), String>;
    fn start_watchdog(&self, wd_id: RawSaiObjectId) -> Result<(), String>;
    fn stop_watchdog(&self, wd_id: RawSaiObjectId) -> Result<(), String>;
}

pub struct PfcWdOrch {
    config: PfcWdOrchConfig,
    stats: PfcWdOrchStats,
    callbacks: Option<Arc<dyn PfcWdOrchCallbacks>>,
    queues: HashMap<String, PfcWdEntry>,
}

impl PfcWdOrch {
    pub fn new(config: PfcWdOrchConfig) -> Self {
        Self {
            config,
            stats: PfcWdOrchStats::default(),
            callbacks: None,
            queues: HashMap::new(),
        }
    }

    pub fn set_callbacks(&mut self, callbacks: Arc<dyn PfcWdOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    pub fn queue_exists(&self, name: &str) -> bool {
        self.queues.contains_key(name)
    }

    pub fn queue_count(&self) -> usize {
        self.queues.len()
    }

    pub fn stats(&self) -> &PfcWdOrchStats {
        &self.stats
    }

    pub fn register_queue(&mut self, config: PfcWdConfig) -> Result<(), PfcWdOrchError> {
        if self.queues.contains_key(&config.queue_name) {
            let err = PfcWdOrchError::QueueExists(config.queue_name.clone());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "PfcWdOrch",
                "set_queue_action"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(config.queue_name.clone())
            .with_object_type("pfcwd_queue")
            .with_error(err.to_string()));
            return Err(err);
        }

        let callbacks = Arc::clone(
            self.callbacks
                .as_ref()
                .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let wd_id = match callbacks.create_watchdog(&config) {
            Ok(id) => id,
            Err(e) => {
                let err = PfcWdOrchError::SaiError(e);
                audit_log!(AuditRecord::new(
                    AuditCategory::ResourceCreate,
                    "PfcWdOrch",
                    "set_queue_action"
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(config.queue_name.clone())
                .with_object_type("pfcwd_queue")
                .with_error(err.to_string()));
                return Err(err);
            }
        };

        let entry = PfcWdEntry::from_config(config.clone(), wd_id);
        self.queues.insert(config.queue_name.clone(), entry);
        self.stats.queues_registered += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceModify,
            "PfcWdOrch",
            "set_queue_action"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(config.queue_name)
        .with_object_type("pfcwd_queue")
        .with_details(serde_json::json!({
            "action": format!("{:?}", config.action),
            "detection_time_ms": config.detection_time.as_millis(),
            "restoration_time_ms": config.restoration_time.as_millis(),
        })));

        Ok(())
    }

    pub fn unregister_queue(&mut self, queue_name: &str) -> Result<(), PfcWdOrchError> {
        let entry = match self.queues.remove(queue_name) {
            Some(e) => e,
            None => {
                let err = PfcWdOrchError::QueueNotFound(queue_name.to_string());
                audit_log!(AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "PfcWdOrch",
                    "set_queue_action"
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(queue_name)
                .with_object_type("pfcwd_queue")
                .with_error(err.to_string()));
                return Err(err);
            }
        };

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        if let Err(e) = callbacks.remove_watchdog(entry.watchdog_id) {
            let err = PfcWdOrchError::SaiError(e);
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceDelete,
                "PfcWdOrch",
                "set_queue_action"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(queue_name)
            .with_object_type("pfcwd_queue")
            .with_error(err.to_string()));
            return Err(err);
        }

        self.stats.queues_unregistered += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "PfcWdOrch",
            "set_queue_action"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(queue_name)
        .with_object_type("pfcwd_queue"));

        Ok(())
    }

    pub fn start_watchdog(&mut self, queue_name: &str) -> Result<(), PfcWdOrchError> {
        let entry = self
            .queues
            .get_mut(queue_name)
            .ok_or_else(|| PfcWdOrchError::QueueNotFound(queue_name.to_string()))?;

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks
            .start_watchdog(entry.watchdog_id)
            .map_err(PfcWdOrchError::SaiError)?;

        entry.enabled = true;

        Ok(())
    }

    pub fn stop_watchdog(&mut self, queue_name: &str) -> Result<(), PfcWdOrchError> {
        let entry = self
            .queues
            .get_mut(queue_name)
            .ok_or_else(|| PfcWdOrchError::QueueNotFound(queue_name.to_string()))?;

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks
            .stop_watchdog(entry.watchdog_id)
            .map_err(PfcWdOrchError::SaiError)?;

        entry.enabled = false;

        Ok(())
    }

    pub fn handle_storm_detected(&mut self, queue_name: &str) {
        if let Some(entry) = self.queues.get_mut(queue_name) {
            entry.storm_detected = true;
            self.stats.storms_detected += 1;

            audit_log!(AuditRecord::new(
                AuditCategory::ResourceModify,
                "PfcWdOrch",
                "update_detection_time"
            )
            .with_outcome(AuditOutcome::Success)
            .with_object_id(queue_name)
            .with_object_type("pfcwd_queue")
            .with_details(serde_json::json!({
                "event": "storm_detected",
                "detection_time_ms": entry.detection_time.as_millis(),
            })));
        }
    }

    pub fn handle_storm_restored(&mut self, queue_name: &str) {
        if let Some(entry) = self.queues.get_mut(queue_name) {
            entry.storm_detected = false;
            self.stats.storms_restored += 1;

            audit_log!(AuditRecord::new(
                AuditCategory::ResourceModify,
                "PfcWdOrch",
                "update_restoration_time"
            )
            .with_outcome(AuditOutcome::Success)
            .with_object_id(queue_name)
            .with_object_type("pfcwd_queue")
            .with_details(serde_json::json!({
                "event": "storm_restored",
                "restoration_time_ms": entry.restoration_time.as_millis(),
            })));
        }
    }

    pub fn get_hw_stats(&self, queue_name: &str) -> Option<serde_json::Value> {
        if let Some(_entry) = self.queues.get(queue_name) {
            audit_log!(
                AuditRecord::new(AuditCategory::Read, "PfcWdOrch", "get_hw_stats")
                    .with_outcome(AuditOutcome::Success)
                    .with_object_id(queue_name)
                    .with_object_type("pfcwd_queue_stats")
                    .with_details(serde_json::json!({
                        "stats_type": "hardware_statistics",
                        "queues_registered": self.stats.queues_registered,
                        "storms_detected": self.stats.storms_detected,
                    }))
            );

            return Some(serde_json::json!({
                "queues_registered": self.stats.queues_registered,
                "queues_unregistered": self.stats.queues_unregistered,
                "storms_detected": self.stats.storms_detected,
                "storms_restored": self.stats.storms_restored,
            }));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCallbacks;
    impl PfcWdOrchCallbacks for MockCallbacks {
        fn create_watchdog(&self, _config: &PfcWdConfig) -> Result<RawSaiObjectId, String> {
            Ok(0x2000)
        }
        fn remove_watchdog(&self, _wd_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
        fn start_watchdog(&self, _wd_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
        fn stop_watchdog(&self, _wd_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_register_queue() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet0:3".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        assert!(orch.register_queue(config).is_ok());
        assert_eq!(orch.queue_count(), 1);
    }

    #[test]
    fn test_storm_handling() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet0:3".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        orch.register_queue(config).unwrap();

        orch.handle_storm_detected("Ethernet0:3");
        assert_eq!(orch.stats().storms_detected, 1);

        orch.handle_storm_restored("Ethernet0:3");
        assert_eq!(orch.stats().storms_restored, 1);
    }

    // PFC Watchdog Configuration Tests

    #[test]
    fn test_enable_pfcwd_on_port() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet4:3".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(300).unwrap(),
            RestorationTime::new(300).unwrap(),
        );

        orch.register_queue(config).unwrap();
        assert!(orch.queue_exists("Ethernet4:3"));

        orch.start_watchdog("Ethernet4:3").unwrap();
        assert_eq!(orch.stats().queues_registered, 1);
    }

    #[test]
    fn test_disable_pfcwd_on_port() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet8:5".to_string(),
            PfcWdAction::Forward,
            DetectionTime::new(250).unwrap(),
            RestorationTime::new(250).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.start_watchdog("Ethernet8:5").unwrap();
        orch.stop_watchdog("Ethernet8:5").unwrap();

        assert!(orch.queue_exists("Ethernet8:5"));
    }

    #[test]
    fn test_unregister_queue() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet12:2".to_string(),
            PfcWdAction::Alert,
            DetectionTime::new(400).unwrap(),
            RestorationTime::new(400).unwrap(),
        );

        orch.register_queue(config).unwrap();
        assert_eq!(orch.queue_count(), 1);

        orch.unregister_queue("Ethernet12:2").unwrap();
        assert_eq!(orch.queue_count(), 0);
        assert_eq!(orch.stats().queues_unregistered, 1);
    }

    #[test]
    fn test_per_queue_configuration() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config1 = PfcWdConfig::new(
            "Ethernet0:0".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        let config2 = PfcWdConfig::new(
            "Ethernet0:1".to_string(),
            PfcWdAction::Forward,
            DetectionTime::new(300).unwrap(),
            RestorationTime::new(300).unwrap(),
        );

        orch.register_queue(config1).unwrap();
        orch.register_queue(config2).unwrap();

        assert_eq!(orch.queue_count(), 2);
        assert!(orch.queue_exists("Ethernet0:0"));
        assert!(orch.queue_exists("Ethernet0:1"));
    }

    // Detection Parameters Tests

    #[test]
    fn test_detection_time_thresholds() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config_min = PfcWdConfig::new(
            "Ethernet16:3".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(100).unwrap(),
            RestorationTime::new(100).unwrap(),
        );

        let config_max = PfcWdConfig::new(
            "Ethernet20:3".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(5000).unwrap(),
            RestorationTime::new(5000).unwrap(),
        );

        assert!(orch.register_queue(config_min).is_ok());
        assert!(orch.register_queue(config_max).is_ok());
    }

    #[test]
    fn test_poll_interval_configuration() {
        let config = PfcWdOrchConfig {
            poll_interval_ms: 100,
        };

        let orch = PfcWdOrch::new(config);
        assert_eq!(orch.config.poll_interval_ms, 100);
    }

    #[test]
    fn test_queue_monitoring() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet24:4".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(500).unwrap(),
            RestorationTime::new(500).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.start_watchdog("Ethernet24:4").unwrap();

        assert!(orch.queue_exists("Ethernet24:4"));
    }

    // Recovery Actions Tests

    #[test]
    fn test_drop_action_on_storm() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet28:2".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.start_watchdog("Ethernet28:2").unwrap();
        orch.handle_storm_detected("Ethernet28:2");

        assert_eq!(orch.stats().storms_detected, 1);
    }

    #[test]
    fn test_forward_action_on_storm() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet32:1".to_string(),
            PfcWdAction::Forward,
            DetectionTime::new(300).unwrap(),
            RestorationTime::new(300).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.handle_storm_detected("Ethernet32:1");

        assert_eq!(orch.stats().storms_detected, 1);
    }

    #[test]
    fn test_alert_action_on_storm() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet36:7".to_string(),
            PfcWdAction::Alert,
            DetectionTime::new(250).unwrap(),
            RestorationTime::new(250).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.handle_storm_detected("Ethernet36:7");

        assert_eq!(orch.stats().storms_detected, 1);
    }

    #[test]
    fn test_restoration_after_storm_clears() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet40:6".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(400).unwrap(),
            RestorationTime::new(1000).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.handle_storm_detected("Ethernet40:6");
        orch.handle_storm_restored("Ethernet40:6");

        assert_eq!(orch.stats().storms_detected, 1);
        assert_eq!(orch.stats().storms_restored, 1);
    }

    // Queue Operations Tests

    #[test]
    fn test_multiple_queues_per_port() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        for queue_idx in 0..8 {
            let queue_name = format!("Ethernet44:{}", queue_idx);
            let config = PfcWdConfig::new(
                queue_name,
                PfcWdAction::Drop,
                DetectionTime::new(200).unwrap(),
                RestorationTime::new(200).unwrap(),
            );
            orch.register_queue(config).unwrap();
        }

        assert_eq!(orch.queue_count(), 8);
    }

    #[test]
    fn test_priority_based_configuration() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let high_priority = PfcWdConfig::new(
            "Ethernet48:7".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(100).unwrap(),
            RestorationTime::new(100).unwrap(),
        );

        let low_priority = PfcWdConfig::new(
            "Ethernet48:0".to_string(),
            PfcWdAction::Forward,
            DetectionTime::new(500).unwrap(),
            RestorationTime::new(500).unwrap(),
        );

        orch.register_queue(high_priority).unwrap();
        orch.register_queue(low_priority).unwrap();

        assert_eq!(orch.queue_count(), 2);
    }

    #[test]
    fn test_queue_state_tracking() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet52:3".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        orch.register_queue(config).unwrap();
        assert!(orch.queue_exists("Ethernet52:3"));

        orch.start_watchdog("Ethernet52:3").unwrap();
        orch.handle_storm_detected("Ethernet52:3");
        orch.handle_storm_restored("Ethernet52:3");

        assert_eq!(orch.stats().storms_detected, 1);
        assert_eq!(orch.stats().storms_restored, 1);
    }

    // Storm Detection Tests

    #[test]
    fn test_detecting_pfc_storms() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet56:2".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.start_watchdog("Ethernet56:2").unwrap();

        orch.handle_storm_detected("Ethernet56:2");
        assert_eq!(orch.stats().storms_detected, 1);
    }

    #[test]
    fn test_storm_recovery() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet60:4".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(300).unwrap(),
            RestorationTime::new(500).unwrap(),
        );

        orch.register_queue(config).unwrap();

        orch.handle_storm_detected("Ethernet60:4");
        assert_eq!(orch.stats().storms_detected, 1);

        orch.handle_storm_restored("Ethernet60:4");
        assert_eq!(orch.stats().storms_restored, 1);
    }

    #[test]
    fn test_multiple_simultaneous_storms() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        for i in 0..4 {
            let queue_name = format!("Ethernet64:{}", i);
            let config = PfcWdConfig::new(
                queue_name.clone(),
                PfcWdAction::Drop,
                DetectionTime::new(200).unwrap(),
                RestorationTime::new(200).unwrap(),
            );
            orch.register_queue(config).unwrap();
            orch.handle_storm_detected(&queue_name);
        }

        assert_eq!(orch.stats().storms_detected, 4);
    }

    // Statistics Tests

    #[test]
    fn test_stats_queues_registered() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        for i in 0..5 {
            let config = PfcWdConfig::new(
                format!("Ethernet68:{}", i),
                PfcWdAction::Drop,
                DetectionTime::new(200).unwrap(),
                RestorationTime::new(200).unwrap(),
            );
            orch.register_queue(config).unwrap();
        }

        assert_eq!(orch.stats().queues_registered, 5);
        assert_eq!(orch.queue_count(), 5);
    }

    #[test]
    fn test_stats_storm_counts() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet72:1".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        orch.register_queue(config).unwrap();

        for _ in 0..3 {
            orch.handle_storm_detected("Ethernet72:1");
            orch.handle_storm_restored("Ethernet72:1");
        }

        assert_eq!(orch.stats().storms_detected, 3);
        assert_eq!(orch.stats().storms_restored, 3);
    }

    #[test]
    fn test_stats_recovery_count() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet76:5".to_string(),
            PfcWdAction::Forward,
            DetectionTime::new(250).unwrap(),
            RestorationTime::new(250).unwrap(),
        );

        orch.register_queue(config).unwrap();
        orch.handle_storm_detected("Ethernet76:5");
        orch.handle_storm_restored("Ethernet76:5");

        assert_eq!(orch.stats().storms_restored, 1);
    }

    // Error Handling Tests

    #[test]
    fn test_invalid_port() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.start_watchdog("NonExistentPort:3");
        assert!(result.is_err());

        match result {
            Err(PfcWdOrchError::QueueNotFound(_)) => {}
            _ => panic!("Expected QueueNotFound error"),
        }
    }

    #[test]
    fn test_queue_not_found() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.unregister_queue("Ethernet80:7");
        assert!(result.is_err());

        match result {
            Err(PfcWdOrchError::QueueNotFound(_)) => {}
            _ => panic!("Expected QueueNotFound error"),
        }
    }

    #[test]
    fn test_duplicate_queue_registration() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config1 = PfcWdConfig::new(
            "Ethernet84:2".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        let config2 = PfcWdConfig::new(
            "Ethernet84:2".to_string(),
            PfcWdAction::Forward,
            DetectionTime::new(300).unwrap(),
            RestorationTime::new(300).unwrap(),
        );

        assert!(orch.register_queue(config1).is_ok());

        let result = orch.register_queue(config2);
        assert!(result.is_err());

        match result {
            Err(PfcWdOrchError::QueueExists(_)) => {}
            _ => panic!("Expected QueueExists error"),
        }
    }

    #[test]
    fn test_no_callbacks_set() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());

        let config = PfcWdConfig::new(
            "Ethernet88:0".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        let result = orch.register_queue(config);
        assert!(result.is_err());

        match result {
            Err(PfcWdOrchError::InvalidConfig(_)) => {}
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    // Edge Cases Tests

    #[test]
    fn test_very_long_restoration_time() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet92:3".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(60000).unwrap(),
        );

        assert!(orch.register_queue(config).is_ok());
    }

    #[test]
    fn test_rapid_enable_disable() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet96:4".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(200).unwrap(),
        );

        orch.register_queue(config).unwrap();

        for _ in 0..10 {
            orch.start_watchdog("Ethernet96:4").unwrap();
            orch.stop_watchdog("Ethernet96:4").unwrap();
        }

        assert!(orch.queue_exists("Ethernet96:4"));
    }

    #[test]
    fn test_storm_on_nonexistent_queue() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        orch.handle_storm_detected("NonExistent:0");
        assert_eq!(orch.stats().storms_detected, 0);

        orch.handle_storm_restored("NonExistent:0");
        assert_eq!(orch.stats().storms_restored, 0);
    }

    #[test]
    fn test_restoration_time_zero() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = PfcWdConfig::new(
            "Ethernet100:6".to_string(),
            PfcWdAction::Drop,
            DetectionTime::new(200).unwrap(),
            RestorationTime::new(0).unwrap(),
        );

        assert!(orch.register_queue(config).is_ok());
    }

    #[test]
    fn test_multiple_register_unregister_cycles() {
        let mut orch = PfcWdOrch::new(PfcWdOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        for _ in 0..5 {
            let config = PfcWdConfig::new(
                "Ethernet104:1".to_string(),
                PfcWdAction::Drop,
                DetectionTime::new(200).unwrap(),
                RestorationTime::new(200).unwrap(),
            );

            orch.register_queue(config).unwrap();
            assert_eq!(orch.queue_count(), 1);

            orch.unregister_queue("Ethernet104:1").unwrap();
            assert_eq!(orch.queue_count(), 0);
        }

        assert_eq!(orch.stats().queues_registered, 5);
        assert_eq!(orch.stats().queues_unregistered, 5);
    }
}
