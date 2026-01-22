//! Counter check orchestration logic.

use super::types::{CounterCheckEntry, CounterCheckKey, CounterCheckStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum CounterCheckOrchError {
    CheckNotFound(CounterCheckKey),
    PortNotFound(String),
}

#[derive(Debug, Clone, Default)]
pub struct CounterCheckOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct CounterCheckOrchStats {
    pub stats: CounterCheckStats,
}

pub trait CounterCheckOrchCallbacks: Send + Sync {}

pub struct CounterCheckOrch {
    config: CounterCheckOrchConfig,
    stats: CounterCheckOrchStats,
    checks: HashMap<CounterCheckKey, CounterCheckEntry>,
}

impl CounterCheckOrch {
    pub fn new(config: CounterCheckOrchConfig) -> Self {
        Self {
            config,
            stats: CounterCheckOrchStats::default(),
            checks: HashMap::new(),
        }
    }

    pub fn get_check(&self, key: &CounterCheckKey) -> Option<&CounterCheckEntry> {
        self.checks.get(key)
    }

    pub fn stats(&self) -> &CounterCheckOrchStats {
        &self.stats
    }
}
