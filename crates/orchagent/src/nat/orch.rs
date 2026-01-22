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

    pub fn add_entry(&mut self, entry: NatEntry) -> Result<(), NatOrchError> {
        let key = entry.key.clone();

        if self.entries.contains_key(&key) {
            return Err(NatOrchError::SaiError("NAT entry already exists".to_string()));
        }

        self.stats.stats.entries_created = self.stats.stats.entries_created.saturating_add(1);
        self.entries.insert(key, entry);

        Ok(())
    }

    pub fn remove_entry(&mut self, key: &NatEntryKey) -> Result<NatEntry, NatOrchError> {
        self.entries.remove(key)
            .ok_or_else(|| NatOrchError::EntryNotFound(key.clone()))
    }

    pub fn get_snat_entries(&self) -> Vec<&NatEntry> {
        self.entries
            .values()
            .filter(|entry| entry.is_snat())
            .collect()
    }

    pub fn get_dnat_entries(&self) -> Vec<&NatEntry> {
        self.entries
            .values()
            .filter(|entry| entry.is_dnat())
            .collect()
    }

    pub fn get_double_nat_entries(&self) -> Vec<&NatEntry> {
        self.entries
            .values()
            .filter(|entry| entry.is_double_nat())
            .collect()
    }

    pub fn get_pool(&self, key: &NatPoolKey) -> Option<&NatPoolEntry> {
        self.pools.get(key)
    }

    pub fn add_pool(&mut self, entry: NatPoolEntry) -> Result<(), NatOrchError> {
        let key = entry.key.clone();

        if self.pools.contains_key(&key) {
            return Err(NatOrchError::SaiError("NAT pool already exists".to_string()));
        }

        // Validate IP range
        let (start, end) = entry.config.ip_range;
        if start > end {
            return Err(NatOrchError::InvalidIpRange(
                format!("Start IP {} > End IP {}", start, end)
            ));
        }

        // Validate port range if present
        if let Some((start_port, end_port)) = entry.config.port_range {
            if start_port > end_port {
                return Err(NatOrchError::InvalidPortRange(
                    format!("Start port {} > End port {}", start_port, end_port)
                ));
            }
        }

        self.stats.stats.pools_created = self.stats.stats.pools_created.saturating_add(1);
        self.pools.insert(key, entry);

        Ok(())
    }

    pub fn remove_pool(&mut self, key: &NatPoolKey) -> Result<NatPoolEntry, NatOrchError> {
        self.pools.remove(key)
            .ok_or_else(|| NatOrchError::PoolNotFound(key.clone()))
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    pub fn stats(&self) -> &NatOrchStats {
        &self.stats
    }
}
