//! Chassis orchestration logic.

use super::types::{ChassisStats, SystemPortEntry, SystemPortKey};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum ChassisOrchError {
    SystemPortNotFound(SystemPortKey),
    InvalidSwitchId(u32),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct ChassisOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct ChassisOrchStats {
    pub stats: ChassisStats,
    pub errors: u64,
}

pub trait ChassisOrchCallbacks: Send + Sync {}

pub struct ChassisOrch {
    config: ChassisOrchConfig,
    stats: ChassisOrchStats,
    system_ports: HashMap<SystemPortKey, SystemPortEntry>,
}

impl ChassisOrch {
    pub fn new(config: ChassisOrchConfig) -> Self {
        Self {
            config,
            stats: ChassisOrchStats::default(),
            system_ports: HashMap::new(),
        }
    }

    pub fn get_system_port(&self, key: &SystemPortKey) -> Option<&SystemPortEntry> {
        self.system_ports.get(key)
    }

    pub fn stats(&self) -> &ChassisOrchStats {
        &self.stats
    }
}
