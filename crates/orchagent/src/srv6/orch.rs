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

    pub fn add_local_sid(&mut self, entry: Srv6LocalSidEntry) -> Result<(), Srv6OrchError> {
        let sid = entry.config.sid.clone();

        if self.local_sids.contains_key(&sid) {
            return Err(Srv6OrchError::SaiError("Local SID already exists".to_string()));
        }

        self.stats.stats.local_sids_created = self.stats.stats.local_sids_created.saturating_add(1);
        self.local_sids.insert(sid, entry);

        Ok(())
    }

    pub fn remove_local_sid(&mut self, sid: &Srv6Sid) -> Result<Srv6LocalSidEntry, Srv6OrchError> {
        self.local_sids.remove(sid)
            .ok_or_else(|| Srv6OrchError::LocalSidNotFound(sid.clone()))
    }

    pub fn get_sidlist(&self, name: &str) -> Option<&Srv6SidListEntry> {
        self.sidlists.get(name)
    }

    pub fn add_sidlist(&mut self, entry: Srv6SidListEntry) -> Result<(), Srv6OrchError> {
        let name = entry.config.name.clone();

        if self.sidlists.contains_key(&name) {
            return Err(Srv6OrchError::SaiError("SID list already exists".to_string()));
        }

        // Validate SIDs in the list
        for sid in &entry.config.sids {
            if let Err(e) = Srv6Sid::from_str(sid.as_str()) {
                return Err(Srv6OrchError::InvalidSid(e));
            }
        }

        self.stats.stats.sidlists_created = self.stats.stats.sidlists_created.saturating_add(1);
        self.sidlists.insert(name, entry);

        Ok(())
    }

    pub fn remove_sidlist(&mut self, name: &str) -> Result<Srv6SidListEntry, Srv6OrchError> {
        self.sidlists.remove(name)
            .ok_or_else(|| Srv6OrchError::SidListNotFound(name.to_string()))
    }

    pub fn get_sidlists_using_sid(&self, sid: &Srv6Sid) -> Vec<&Srv6SidListEntry> {
        self.sidlists
            .values()
            .filter(|entry| entry.config.sids.contains(sid))
            .collect()
    }

    pub fn local_sid_count(&self) -> usize {
        self.local_sids.len()
    }

    pub fn sidlist_count(&self) -> usize {
        self.sidlists.len()
    }

    pub fn stats(&self) -> &Srv6OrchStats {
        &self.stats
    }
}
