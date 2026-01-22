//! Mirror session orchestration logic (stub).

use super::types::MirrorEntry;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum MirrorOrchError {
    SessionExists(String),
}

#[derive(Debug, Clone, Default)]
pub struct MirrorOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct MirrorOrchStats {
    pub sessions_created: u64,
}

pub trait MirrorOrchCallbacks: Send + Sync {}

pub struct MirrorOrch {
    config: MirrorOrchConfig,
    stats: MirrorOrchStats,
    sessions: HashMap<String, MirrorEntry>,
}

impl MirrorOrch {
    pub fn new(config: MirrorOrchConfig) -> Self {
        Self {
            config,
            stats: MirrorOrchStats::default(),
            sessions: HashMap::new(),
        }
    }
}
