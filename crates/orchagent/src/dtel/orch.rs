//! DTel orchestration logic (stub).

use super::types::{DtelEventType, IntSessionEntry};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum DtelOrchError {
    SessionExists(String),
    SessionNotFound(String),
}

#[derive(Debug, Clone, Default)]
pub struct DtelOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct DtelOrchStats {
    pub sessions_created: u64,
}

pub trait DtelOrchCallbacks: Send + Sync {}

pub struct DtelOrch {
    config: DtelOrchConfig,
    stats: DtelOrchStats,
}

impl DtelOrch {
    pub fn new(config: DtelOrchConfig) -> Self {
        Self { config, stats: DtelOrchStats::default() }
    }
}
