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

    pub fn get_pool_mut(&mut self, name: &str) -> Option<&mut BufferPoolEntry> {
        self.pools.get_mut(name)
    }

    pub fn add_pool(&mut self, entry: BufferPoolEntry) -> Result<(), BufferOrchError> {
        let name = entry.name.clone();

        if self.pools.contains_key(&name) {
            return Err(BufferOrchError::SaiError("Pool already exists".to_string()));
        }

        self.stats.stats.pools_created = self.stats.stats.pools_created.saturating_add(1);
        self.pools.insert(name, entry);

        Ok(())
    }

    pub fn remove_pool(&mut self, name: &str) -> Result<BufferPoolEntry, BufferOrchError> {
        let entry = self.pools.get(name)
            .ok_or_else(|| BufferOrchError::PoolNotFound(name.to_string()))?;

        if entry.ref_count > 0 {
            return Err(BufferOrchError::RefCountError(
                format!("Pool {} still has {} references", name, entry.ref_count)
            ));
        }

        self.pools.remove(name)
            .ok_or_else(|| BufferOrchError::PoolNotFound(name.to_string()))
    }

    pub fn increment_pool_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let pool = self.pools.get_mut(name)
            .ok_or_else(|| BufferOrchError::PoolNotFound(name.to_string()))?;
        Ok(pool.add_ref())
    }

    pub fn decrement_pool_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let pool = self.pools.get_mut(name)
            .ok_or_else(|| BufferOrchError::PoolNotFound(name.to_string()))?;
        pool.remove_ref()
            .map_err(|e| BufferOrchError::RefCountError(e))
    }

    pub fn get_profile(&self, name: &str) -> Option<&BufferProfileEntry> {
        self.profiles.get(name)
    }

    pub fn get_profile_mut(&mut self, name: &str) -> Option<&mut BufferProfileEntry> {
        self.profiles.get_mut(name)
    }

    pub fn add_profile(&mut self, entry: BufferProfileEntry) -> Result<(), BufferOrchError> {
        let name = entry.name.clone();

        if self.profiles.contains_key(&name) {
            return Err(BufferOrchError::SaiError("Profile already exists".to_string()));
        }

        // Verify pool exists
        if !self.pools.contains_key(&entry.config.pool_name) {
            return Err(BufferOrchError::PoolNotFound(entry.config.pool_name.clone()));
        }

        self.stats.stats.profiles_created = self.stats.stats.profiles_created.saturating_add(1);
        self.profiles.insert(name, entry);

        Ok(())
    }

    pub fn remove_profile(&mut self, name: &str) -> Result<BufferProfileEntry, BufferOrchError> {
        let entry = self.profiles.get(name)
            .ok_or_else(|| BufferOrchError::ProfileNotFound(name.to_string()))?;

        if entry.ref_count > 0 {
            return Err(BufferOrchError::RefCountError(
                format!("Profile {} still has {} references", name, entry.ref_count)
            ));
        }

        self.profiles.remove(name)
            .ok_or_else(|| BufferOrchError::ProfileNotFound(name.to_string()))
    }

    pub fn increment_profile_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let profile = self.profiles.get_mut(name)
            .ok_or_else(|| BufferOrchError::ProfileNotFound(name.to_string()))?;
        Ok(profile.add_ref())
    }

    pub fn decrement_profile_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let profile = self.profiles.get_mut(name)
            .ok_or_else(|| BufferOrchError::ProfileNotFound(name.to_string()))?;
        profile.remove_ref()
            .map_err(|e| BufferOrchError::RefCountError(e))
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    pub fn stats(&self) -> &BufferOrchStats {
        &self.stats
    }
}
