//! Neighbor orchestration logic.

use super::types::{NeighborEntry, NeighborKey, NeighborStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum NeighOrchError {
    NeighborNotFound(NeighborKey),
    InvalidMac(String),
    InvalidIp(String),
    InterfaceNotFound(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct NeighOrchConfig {
    pub enable_kernel_sync: bool,
    pub restore_neighbors: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NeighOrchStats {
    pub stats: NeighborStats,
    pub errors: u64,
}

pub trait NeighOrchCallbacks: Send + Sync {
    fn on_neighbor_added(&self, entry: &NeighborEntry);
    fn on_neighbor_removed(&self, key: &NeighborKey);
    fn on_neighbor_updated(&self, entry: &NeighborEntry);
}

pub struct NeighOrch {
    config: NeighOrchConfig,
    stats: NeighOrchStats,
    neighbors: HashMap<NeighborKey, NeighborEntry>,
}

impl NeighOrch {
    pub fn new(config: NeighOrchConfig) -> Self {
        Self {
            config,
            stats: NeighOrchStats::default(),
            neighbors: HashMap::new(),
        }
    }

    pub fn get_neighbor(&self, key: &NeighborKey) -> Option<&NeighborEntry> {
        self.neighbors.get(key)
    }

    pub fn add_neighbor(&mut self, entry: NeighborEntry) -> Result<(), NeighOrchError> {
        let key = entry.key.clone();

        if self.neighbors.contains_key(&key) {
            return self.update_neighbor(entry);
        }

        // Update stats based on IP version
        if entry.is_ipv4() {
            self.stats.stats.ipv4_neighbors = self.stats.stats.ipv4_neighbors.saturating_add(1);
        } else {
            self.stats.stats.ipv6_neighbors = self.stats.stats.ipv6_neighbors.saturating_add(1);
        }

        self.stats.stats.neighbors_added = self.stats.stats.neighbors_added.saturating_add(1);
        self.neighbors.insert(key, entry);

        Ok(())
    }

    pub fn remove_neighbor(&mut self, key: &NeighborKey) -> Result<NeighborEntry, NeighOrchError> {
        let entry = self.neighbors.remove(key)
            .ok_or_else(|| NeighOrchError::NeighborNotFound(key.clone()))?;

        // Update stats based on IP version
        if entry.is_ipv4() {
            self.stats.stats.ipv4_neighbors = self.stats.stats.ipv4_neighbors.saturating_sub(1);
        } else {
            self.stats.stats.ipv6_neighbors = self.stats.stats.ipv6_neighbors.saturating_sub(1);
        }

        self.stats.stats.neighbors_removed = self.stats.stats.neighbors_removed.saturating_add(1);

        Ok(entry)
    }

    pub fn update_neighbor(&mut self, entry: NeighborEntry) -> Result<(), NeighOrchError> {
        let key = entry.key.clone();

        if !self.neighbors.contains_key(&key) {
            return Err(NeighOrchError::NeighborNotFound(key));
        }

        self.stats.stats.neighbors_updated = self.stats.stats.neighbors_updated.saturating_add(1);
        self.neighbors.insert(key, entry);

        Ok(())
    }

    pub fn get_neighbors_by_interface(&self, interface: &str) -> Vec<&NeighborEntry> {
        self.neighbors
            .values()
            .filter(|entry| entry.key.interface == interface)
            .collect()
    }

    pub fn clear_interface(&mut self, interface: &str) -> usize {
        let keys_to_remove: Vec<_> = self.neighbors
            .keys()
            .filter(|key| key.interface == interface)
            .cloned()
            .collect();

        let count = keys_to_remove.len();
        for key in keys_to_remove {
            let _ = self.remove_neighbor(&key);
        }

        count
    }

    pub fn neighbor_count(&self) -> usize {
        self.neighbors.len()
    }

    pub fn stats(&self) -> &NeighOrchStats {
        &self.stats
    }
}
