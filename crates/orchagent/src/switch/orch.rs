//! Switch orchestration logic.

use super::types::{SwitchConfig, SwitchState};

#[derive(Debug, Clone)]
pub enum SwitchOrchError {
    NotInitialized,
    InvalidHashAlgorithm(String),
    InvalidHashField(String),
    SaiError(String),
    ConfigurationError(String),
}

#[derive(Debug, Clone, Default)]
pub struct SwitchOrchConfig {
    pub enable_warm_restart: bool,
    pub warm_restart_read_timer: u32,
    pub warm_restart_timer: u32,
}

#[derive(Debug, Clone, Default)]
pub struct SwitchOrchStats {
    pub hash_updates: u64,
    pub capability_queries: u64,
    pub warm_restarts: u64,
}

pub trait SwitchOrchCallbacks: Send + Sync {
    fn on_switch_initialized(&self, state: &SwitchState);
    fn on_hash_updated(&self, is_ecmp: bool);
    fn on_warm_restart_begin(&self);
    fn on_warm_restart_end(&self, success: bool);
}

pub struct SwitchOrch {
    config: SwitchOrchConfig,
    stats: SwitchOrchStats,
    state: Option<SwitchState>,
    switch_config: SwitchConfig,
}

impl SwitchOrch {
    pub fn new(config: SwitchOrchConfig) -> Self {
        Self {
            config,
            stats: SwitchOrchStats::default(),
            state: None,
            switch_config: SwitchConfig::default(),
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.state.is_some()
    }

    pub fn get_state(&self) -> Option<&SwitchState> {
        self.state.as_ref()
    }

    pub fn stats(&self) -> &SwitchOrchStats {
        &self.stats
    }
}
