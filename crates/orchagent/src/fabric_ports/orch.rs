//! Fabric ports orchestration logic (stub).

use super::types::FabricPortState;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FabricPortsOrchError {
    PortNotFound(u32),
}

#[derive(Debug, Clone, Default)]
pub struct FabricPortsOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct FabricPortsOrchStats {
    pub ports_monitored: u64,
}

pub trait FabricPortsOrchCallbacks: Send + Sync {}

pub struct FabricPortsOrch {
    config: FabricPortsOrchConfig,
    stats: FabricPortsOrchStats,
    ports: HashMap<u32, FabricPortState>,
}

impl FabricPortsOrch {
    pub fn new(config: FabricPortsOrchConfig) -> Self {
        Self {
            config,
            stats: FabricPortsOrchStats::default(),
            ports: HashMap::new(),
        }
    }
}
