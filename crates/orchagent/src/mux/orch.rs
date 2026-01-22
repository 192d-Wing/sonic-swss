//! MUX cable orchestration logic.

use super::types::{MuxPortEntry, MuxState, MuxStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum MuxOrchError {
    PortNotFound(String),
    InvalidState(String),
    TunnelCreationFailed(String),
    AclCreationFailed(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct MuxOrchConfig {
    pub enable_active_active: bool,
    pub state_change_timeout_ms: u32,
}

impl MuxOrchConfig {
    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.state_change_timeout_ms = timeout_ms;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct MuxOrchStats {
    pub stats: MuxStats,
    pub errors: u64,
}

pub trait MuxOrchCallbacks: Send + Sync {
    fn on_port_added(&self, entry: &MuxPortEntry);
    fn on_port_removed(&self, port_name: &str);
    fn on_state_change(&self, port_name: &str, old_state: MuxState, new_state: MuxState);
}

pub struct MuxOrch {
    config: MuxOrchConfig,
    stats: MuxOrchStats,
    ports: HashMap<String, MuxPortEntry>,
}

impl MuxOrch {
    pub fn new(config: MuxOrchConfig) -> Self {
        Self {
            config,
            stats: MuxOrchStats::default(),
            ports: HashMap::new(),
        }
    }

    pub fn get_port(&self, name: &str) -> Option<&MuxPortEntry> {
        self.ports.get(name)
    }

    pub fn stats(&self) -> &MuxOrchStats {
        &self.stats
    }
}
