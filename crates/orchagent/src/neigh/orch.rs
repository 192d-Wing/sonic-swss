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

    pub fn stats(&self) -> &NeighOrchStats {
        &self.stats
    }
}
