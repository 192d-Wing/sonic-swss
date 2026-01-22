//! FDB orchestration logic.

use super::types::{FdbEntry, FdbFlushStats, FdbKey, RawSaiObjectId};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FdbOrchError {
    EntryNotFound(FdbKey),
    PortNotFound(String),
    VlanNotFound(u16),
    InvalidMacAddress(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct FdbOrchConfig {
    pub aging_time: u32,
    pub enable_flush_on_port_down: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FdbOrchStats {
    pub entries_added: u64,
    pub entries_removed: u64,
    pub entries_updated: u64,
    pub flush_stats: FdbFlushStats,
}

pub trait FdbOrchCallbacks: Send + Sync {
    fn on_fdb_entry_added(&self, entry: &FdbEntry);
    fn on_fdb_entry_removed(&self, key: &FdbKey);
    fn on_fdb_flush(&self, port: Option<&str>, vlan: Option<u16>);
}

pub struct FdbOrch {
    config: FdbOrchConfig,
    stats: FdbOrchStats,
    entries: HashMap<FdbKey, FdbEntry>,
    vlan_to_vlan_oid: HashMap<u16, RawSaiObjectId>,
}

impl FdbOrch {
    pub fn new(config: FdbOrchConfig) -> Self {
        Self {
            config,
            stats: FdbOrchStats::default(),
            entries: HashMap::new(),
            vlan_to_vlan_oid: HashMap::new(),
        }
    }

    pub fn get_entry(&self, key: &FdbKey) -> Option<&FdbEntry> {
        self.entries.get(key)
    }

    pub fn stats(&self) -> &FdbOrchStats {
        &self.stats
    }
}
