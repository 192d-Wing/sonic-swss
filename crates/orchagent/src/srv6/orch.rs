//! SRv6 orchestration logic.

use super::types::{Srv6LocalSidEntry, Srv6Sid, Srv6SidListEntry, Srv6Stats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Srv6OrchError {
    LocalSidNotFound(Srv6Sid),
    SidListNotFound(String),
    InvalidSid(String),
    InvalidEndpointBehavior(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct Srv6OrchConfig {
    pub enable_my_sid_table: bool,
    pub enable_usp: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Srv6OrchStats {
    pub stats: Srv6Stats,
    pub errors: u64,
}

pub trait Srv6OrchCallbacks: Send + Sync {
    fn on_local_sid_created(&self, entry: &Srv6LocalSidEntry);
    fn on_local_sid_removed(&self, sid: &Srv6Sid);
    fn on_sidlist_created(&self, entry: &Srv6SidListEntry);
    fn on_sidlist_removed(&self, name: &str);
}

pub struct Srv6Orch {
    config: Srv6OrchConfig,
    stats: Srv6OrchStats,
    local_sids: HashMap<Srv6Sid, Srv6LocalSidEntry>,
    sidlists: HashMap<String, Srv6SidListEntry>,
}

impl Srv6Orch {
    pub fn new(config: Srv6OrchConfig) -> Self {
        Self {
            config,
            stats: Srv6OrchStats::default(),
            local_sids: HashMap::new(),
            sidlists: HashMap::new(),
        }
    }

    pub fn get_local_sid(&self, sid: &Srv6Sid) -> Option<&Srv6LocalSidEntry> {
        self.local_sids.get(sid)
    }

    pub fn get_sidlist(&self, name: &str) -> Option<&Srv6SidListEntry> {
        self.sidlists.get(name)
    }

    pub fn stats(&self) -> &Srv6OrchStats {
        &self.stats
    }
}
