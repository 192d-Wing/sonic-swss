//! NAT orchestration logic.

use super::types::{NatEntry, NatEntryKey, NatPoolEntry, NatPoolKey, NatStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum NatOrchError {
    EntryNotFound(NatEntryKey),
    PoolNotFound(NatPoolKey),
    AclNotFound(String),
    InvalidIpRange(String),
    InvalidPortRange(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct NatOrchConfig {
    pub enable_hairpin: bool,
    pub tcp_timeout: u32,
    pub udp_timeout: u32,
}

impl NatOrchConfig {
    pub fn with_timeouts(mut self, tcp: u32, udp: u32) -> Self {
        self.tcp_timeout = tcp;
        self.udp_timeout = udp;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct NatOrchStats {
    pub stats: NatStats,
    pub errors: u64,
}

pub trait NatOrchCallbacks: Send + Sync {
    fn on_entry_created(&self, entry: &NatEntry);
    fn on_entry_removed(&self, key: &NatEntryKey);
    fn on_pool_created(&self, pool: &NatPoolEntry);
    fn on_pool_removed(&self, key: &NatPoolKey);
}

pub struct NatOrch {
    config: NatOrchConfig,
    stats: NatOrchStats,
    entries: HashMap<NatEntryKey, NatEntry>,
    pools: HashMap<NatPoolKey, NatPoolEntry>,
}

impl NatOrch {
    pub fn new(config: NatOrchConfig) -> Self {
        Self {
            config,
            stats: NatOrchStats::default(),
            entries: HashMap::new(),
            pools: HashMap::new(),
        }
    }

    pub fn get_entry(&self, key: &NatEntryKey) -> Option<&NatEntry> {
        self.entries.get(key)
    }

    pub fn get_pool(&self, key: &NatPoolKey) -> Option<&NatPoolEntry> {
        self.pools.get(key)
    }

    pub fn stats(&self) -> &NatOrchStats {
        &self.stats
    }
}
