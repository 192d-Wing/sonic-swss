//! Router interface orchestration logic (stub).

use super::types::IntfsEntry;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum IntfsOrchError {
    InterfaceNotFound(String),
}

#[derive(Debug, Clone, Default)]
pub struct IntfsOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct IntfsOrchStats {
    pub interfaces_created: u64,
}

pub trait IntfsOrchCallbacks: Send + Sync {}

pub struct IntfsOrch {
    config: IntfsOrchConfig,
    stats: IntfsOrchStats,
    interfaces: HashMap<String, IntfsEntry>,
}

impl IntfsOrch {
    pub fn new(config: IntfsOrchConfig) -> Self {
        Self {
            config,
            stats: IntfsOrchStats::default(),
            interfaces: HashMap::new(),
        }
    }
}
