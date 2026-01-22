//! Policy-Based Hashing orchestration logic.

use super::types::{PbhHashEntry, PbhRuleEntry, PbhStats, PbhTableEntry};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum PbhOrchError {
    HashNotFound(String),
    TableNotFound(String),
    RuleNotFound(String),
    InvalidPriority(u32),
    InvalidHashField(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct PbhOrchConfig {
    pub enable_flow_counters: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PbhOrchStats {
    pub stats: PbhStats,
    pub errors: u64,
}

pub trait PbhOrchCallbacks: Send + Sync {
    fn on_hash_created(&self, hash: &PbhHashEntry);
    fn on_hash_removed(&self, hash_name: &str);
    fn on_table_created(&self, table: &PbhTableEntry);
    fn on_table_removed(&self, table_name: &str);
    fn on_rule_created(&self, rule: &PbhRuleEntry);
    fn on_rule_removed(&self, table_name: &str, rule_name: &str);
}

pub struct PbhOrch {
    config: PbhOrchConfig,
    stats: PbhOrchStats,
    hashes: HashMap<String, PbhHashEntry>,
    tables: HashMap<String, PbhTableEntry>,
    rules: HashMap<(String, String), PbhRuleEntry>,
}

impl PbhOrch {
    pub fn new(config: PbhOrchConfig) -> Self {
        Self {
            config,
            stats: PbhOrchStats::default(),
            hashes: HashMap::new(),
            tables: HashMap::new(),
            rules: HashMap::new(),
        }
    }

    pub fn get_hash(&self, name: &str) -> Option<&PbhHashEntry> {
        self.hashes.get(name)
    }

    pub fn get_table(&self, name: &str) -> Option<&PbhTableEntry> {
        self.tables.get(name)
    }

    pub fn get_rule(&self, table_name: &str, rule_name: &str) -> Option<&PbhRuleEntry> {
        self.rules.get(&(table_name.to_string(), rule_name.to_string()))
    }

    pub fn stats(&self) -> &PbhOrchStats {
        &self.stats
    }
}
