//! Buffer orchestration logic.

use super::types::{BufferPoolEntry, BufferProfileEntry, BufferStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum BufferOrchError {
    PoolNotFound(String),
    ProfileNotFound(String),
    InvalidThreshold(String),
    SaiError(String),
    RefCountError(String),
}

#[derive(Debug, Clone, Default)]
pub struct BufferOrchConfig {
    pub enable_ingress_buffer_drop: bool,
    pub enable_egress_buffer_drop: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BufferOrchStats {
    pub stats: BufferStats,
    pub errors: u64,
}

pub trait BufferOrchCallbacks: Send + Sync {
    fn on_pool_created(&self, pool: &BufferPoolEntry);
    fn on_pool_removed(&self, pool_name: &str);
    fn on_profile_created(&self, profile: &BufferProfileEntry);
    fn on_profile_removed(&self, profile_name: &str);
}

pub struct BufferOrch {
    config: BufferOrchConfig,
    stats: BufferOrchStats,
    pools: HashMap<String, BufferPoolEntry>,
    profiles: HashMap<String, BufferProfileEntry>,
}

impl BufferOrch {
    pub fn new(config: BufferOrchConfig) -> Self {
        Self {
            config,
            stats: BufferOrchStats::default(),
            pools: HashMap::new(),
            profiles: HashMap::new(),
        }
    }

    pub fn get_pool(&self, name: &str) -> Option<&BufferPoolEntry> {
        self.pools.get(name)
    }

    pub fn get_profile(&self, name: &str) -> Option<&BufferProfileEntry> {
        self.profiles.get(name)
    }

    pub fn stats(&self) -> &BufferOrchStats {
        &self.stats
    }
}
