//! QoS orchestration logic.

use super::types::{QosMapEntry, QosStats, SchedulerEntry, WredProfile};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum QosOrchError {
    MapNotFound(String),
    SchedulerNotFound(String),
    WredNotFound(String),
    InvalidMapping(u8, u8),
    InvalidWeight(u8),
    InvalidThreshold(u32),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct QosOrchConfig {
    pub enable_wred: bool,
    pub enable_ecn: bool,
}

#[derive(Debug, Clone, Default)]
pub struct QosOrchStats {
    pub stats: QosStats,
    pub errors: u64,
}

pub trait QosOrchCallbacks: Send + Sync {
    fn on_map_created(&self, map: &QosMapEntry);
    fn on_map_removed(&self, map_name: &str);
    fn on_scheduler_created(&self, scheduler: &SchedulerEntry);
    fn on_scheduler_removed(&self, scheduler_name: &str);
    fn on_wred_profile_created(&self, profile: &WredProfile);
    fn on_wred_profile_removed(&self, profile_name: &str);
}

pub struct QosOrch {
    config: QosOrchConfig,
    stats: QosOrchStats,
    qos_maps: HashMap<String, QosMapEntry>,
    schedulers: HashMap<String, SchedulerEntry>,
    wred_profiles: HashMap<String, WredProfile>,
}

impl QosOrch {
    pub fn new(config: QosOrchConfig) -> Self {
        Self {
            config,
            stats: QosOrchStats::default(),
            qos_maps: HashMap::new(),
            schedulers: HashMap::new(),
            wred_profiles: HashMap::new(),
        }
    }

    pub fn get_map(&self, name: &str) -> Option<&QosMapEntry> {
        self.qos_maps.get(name)
    }

    pub fn get_map_mut(&mut self, name: &str) -> Option<&mut QosMapEntry> {
        self.qos_maps.get_mut(name)
    }

    pub fn add_map(&mut self, entry: QosMapEntry) -> Result<(), QosOrchError> {
        let name = entry.name.clone();

        if self.qos_maps.contains_key(&name) {
            return Err(QosOrchError::SaiError("QoS map already exists".to_string()));
        }

        // Validate mappings (TC/Queue typically 0-7, DSCP 0-63)
        for (&from, &to) in &entry.mappings {
            if from > 63 || to > 63 {
                return Err(QosOrchError::InvalidMapping(from, to));
            }
        }

        self.stats.stats.maps_created = self.stats.stats.maps_created.saturating_add(1);
        self.qos_maps.insert(name, entry);

        Ok(())
    }

    pub fn remove_map(&mut self, name: &str) -> Result<QosMapEntry, QosOrchError> {
        self.qos_maps.remove(name)
            .ok_or_else(|| QosOrchError::MapNotFound(name.to_string()))
    }

    pub fn update_map_mapping(&mut self, name: &str, from: u8, to: u8) -> Result<(), QosOrchError> {
        if from > 63 || to > 63 {
            return Err(QosOrchError::InvalidMapping(from, to));
        }

        let map = self.qos_maps.get_mut(name)
            .ok_or_else(|| QosOrchError::MapNotFound(name.to_string()))?;

        map.add_mapping(from, to);
        Ok(())
    }

    pub fn get_scheduler(&self, name: &str) -> Option<&SchedulerEntry> {
        self.schedulers.get(name)
    }

    pub fn add_scheduler(&mut self, entry: SchedulerEntry) -> Result<(), QosOrchError> {
        let name = entry.name.clone();

        if self.schedulers.contains_key(&name) {
            return Err(QosOrchError::SaiError("Scheduler already exists".to_string()));
        }

        // Validate weight (typically 1-100)
        if entry.config.weight == 0 {
            return Err(QosOrchError::InvalidWeight(entry.config.weight));
        }

        self.stats.stats.schedulers_created = self.stats.stats.schedulers_created.saturating_add(1);
        self.schedulers.insert(name, entry);

        Ok(())
    }

    pub fn remove_scheduler(&mut self, name: &str) -> Result<SchedulerEntry, QosOrchError> {
        self.schedulers.remove(name)
            .ok_or_else(|| QosOrchError::SchedulerNotFound(name.to_string()))
    }

    pub fn get_wred_profile(&self, name: &str) -> Option<&WredProfile> {
        self.wred_profiles.get(name)
    }

    pub fn add_wred_profile(&mut self, profile: WredProfile) -> Result<(), QosOrchError> {
        let name = profile.name.clone();

        if self.wred_profiles.contains_key(&name) {
            return Err(QosOrchError::SaiError("WRED profile already exists".to_string()));
        }

        // Validate thresholds
        if let (Some(min), Some(max)) = (profile.green_min_threshold, profile.green_max_threshold) {
            if min > max {
                return Err(QosOrchError::InvalidThreshold(min));
            }
        }

        if let (Some(min), Some(max)) = (profile.yellow_min_threshold, profile.yellow_max_threshold) {
            if min > max {
                return Err(QosOrchError::InvalidThreshold(min));
            }
        }

        if let (Some(min), Some(max)) = (profile.red_min_threshold, profile.red_max_threshold) {
            if min > max {
                return Err(QosOrchError::InvalidThreshold(min));
            }
        }

        self.stats.stats.wred_profiles_created = self.stats.stats.wred_profiles_created.saturating_add(1);
        self.wred_profiles.insert(name, profile);

        Ok(())
    }

    pub fn remove_wred_profile(&mut self, name: &str) -> Result<WredProfile, QosOrchError> {
        self.wred_profiles.remove(name)
            .ok_or_else(|| QosOrchError::WredNotFound(name.to_string()))
    }

    pub fn map_count(&self) -> usize {
        self.qos_maps.len()
    }

    pub fn scheduler_count(&self) -> usize {
        self.schedulers.len()
    }

    pub fn wred_profile_count(&self) -> usize {
        self.wred_profiles.len()
    }

    pub fn stats(&self) -> &QosOrchStats {
        &self.stats
    }
}
