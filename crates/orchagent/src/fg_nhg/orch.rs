//! Fine-Grained Next Hop Group orchestration logic.

use super::types::{FgNhgEntry, FgNhgPrefix, FgNhgStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FgNhgOrchError {
    NhgNotFound(FgNhgPrefix),
    InvalidBucketSize(u32),
    InvalidWeight(u32),
    MemberNotFound(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct FgNhgOrchConfig {
    pub default_bucket_size: u32,
    pub enable_rebalancing: bool,
}

impl FgNhgOrchConfig {
    pub fn with_bucket_size(mut self, size: u32) -> Self {
        self.default_bucket_size = size;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct FgNhgOrchStats {
    pub stats: FgNhgStats,
    pub errors: u64,
}

pub trait FgNhgOrchCallbacks: Send + Sync {
    fn on_nhg_created(&self, entry: &FgNhgEntry);
    fn on_nhg_removed(&self, prefix: &FgNhgPrefix);
    fn on_member_added(&self, prefix: &FgNhgPrefix, member_ip: &str);
    fn on_member_removed(&self, prefix: &FgNhgPrefix, member_ip: &str);
}

pub struct FgNhgOrch {
    config: FgNhgOrchConfig,
    stats: FgNhgOrchStats,
    nhgs: HashMap<FgNhgPrefix, FgNhgEntry>,
}

impl FgNhgOrch {
    pub fn new(config: FgNhgOrchConfig) -> Self {
        Self {
            config,
            stats: FgNhgOrchStats::default(),
            nhgs: HashMap::new(),
        }
    }

    pub fn get_nhg(&self, prefix: &FgNhgPrefix) -> Option<&FgNhgEntry> {
        self.nhgs.get(prefix)
    }

    pub fn stats(&self) -> &FgNhgOrchStats {
        &self.stats
    }
}
