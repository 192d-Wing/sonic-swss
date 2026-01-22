//! CoPP orchestration logic.

use super::types::{CoppStats, CoppTrapEntry, CoppTrapKey};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum CoppOrchError {
    TrapNotFound(CoppTrapKey),
    InvalidQueue(u8),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct CoppOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct CoppOrchStats {
    pub stats: CoppStats,
    pub errors: u64,
}

pub trait CoppOrchCallbacks: Send + Sync {}

pub struct CoppOrch {
    config: CoppOrchConfig,
    stats: CoppOrchStats,
    traps: HashMap<CoppTrapKey, CoppTrapEntry>,
}

impl CoppOrch {
    pub fn new(config: CoppOrchConfig) -> Self {
        Self {
            config,
            stats: CoppOrchStats::default(),
            traps: HashMap::new(),
        }
    }

    pub fn get_trap(&self, key: &CoppTrapKey) -> Option<&CoppTrapEntry> {
        self.traps.get(key)
    }

    pub fn stats(&self) -> &CoppOrchStats {
        &self.stats
    }
}
