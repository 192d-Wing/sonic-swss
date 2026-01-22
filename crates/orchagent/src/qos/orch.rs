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

    pub fn get_scheduler(&self, name: &str) -> Option<&SchedulerEntry> {
        self.schedulers.get(name)
    }

    pub fn get_wred_profile(&self, name: &str) -> Option<&WredProfile> {
        self.wred_profiles.get(name)
    }

    pub fn stats(&self) -> &QosOrchStats {
        &self.stats
    }
}
