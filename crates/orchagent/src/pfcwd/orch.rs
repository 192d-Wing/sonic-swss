//! PFC Watchdog orchestration logic.

use super::types::{DetectionTime, PfcWdAction, PfcWdConfig, PfcWdEntry, PfcWdStats, RestorationTime};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum PfcWdOrchError {
    QueueExists(String),
    QueueNotFound(String),
    InvalidConfig(String),
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
            return Err(PfcWdOrchError::QueueExists(config.queue_name.clone()));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let wd_id = callbacks.create_watchdog(&config)
            .map_err(PfcWdOrchError::SaiError)?;

        let entry = PfcWdEntry::from_config(config.clone(), wd_id);
        self.queues.insert(config.queue_name, entry);
        self.stats.queues_registered += 1;

        Ok(())
    }

    pub fn unregister_queue(&mut self, queue_name: &str) -> Result<(), PfcWdOrchError> {
        let entry = self.queues.remove(queue_name)
            .ok_or_else(|| PfcWdOrchError::QueueNotFound(queue_name.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks.remove_watchdog(entry.watchdog_id)
            .map_err(PfcWdOrchError::SaiError)?;

        self.stats.queues_unregistered += 1;

        Ok(())
    }

    pub fn start_watchdog(&mut self, queue_name: &str) -> Result<(), PfcWdOrchError> {
        let entry = self.queues.get_mut(queue_name)
            .ok_or_else(|| PfcWdOrchError::QueueNotFound(queue_name.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks.start_watchdog(entry.watchdog_id)
            .map_err(PfcWdOrchError::SaiError)?;

        entry.enabled = true;

        Ok(())
    }

    pub fn stop_watchdog(&mut self, queue_name: &str) -> Result<(), PfcWdOrchError> {
        let entry = self.queues.get_mut(queue_name)
            .ok_or_else(|| PfcWdOrchError::QueueNotFound(queue_name.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| PfcWdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks.stop_watchdog(entry.watchdog_id)
            .map_err(PfcWdOrchError::SaiError)?;

        entry.enabled = false;

        Ok(())
    }

    pub fn handle_storm_detected(&mut self, queue_name: &str) {
        if let Some(entry) = self.queues.get_mut(queue_name) {
            entry.storm_detected = true;
            self.stats.storms_detected += 1;
        }
    }

    pub fn handle_storm_restored(&mut self, queue_name: &str) {
        if let Some(entry) = self.queues.get_mut(queue_name) {
            entry.storm_detected = false;
            self.stats.storms_restored += 1;
        }
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
}
