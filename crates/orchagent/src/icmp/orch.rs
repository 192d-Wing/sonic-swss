//! ICMP echo orchestration logic.

use super::types::{IcmpEchoEntry, IcmpEchoKey, IcmpStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum IcmpOrchError {
    EntryNotFound(IcmpEchoKey),
}

#[derive(Debug, Clone, Default)]
pub struct IcmpOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct IcmpOrchStats {
    pub stats: IcmpStats,
}

pub trait IcmpOrchCallbacks: Send + Sync {}

pub struct IcmpOrch {
    config: IcmpOrchConfig,
    stats: IcmpOrchStats,
    entries: HashMap<IcmpEchoKey, IcmpEchoEntry>,
}

impl IcmpOrch {
    pub fn new(config: IcmpOrchConfig) -> Self {
        Self {
            config,
            stats: IcmpOrchStats::default(),
            entries: HashMap::new(),
        }
    }

    pub fn get_entry(&self, key: &IcmpEchoKey) -> Option<&IcmpEchoEntry> {
        self.entries.get(key)
    }

    pub fn stats(&self) -> &IcmpOrchStats {
        &self.stats
    }
}
