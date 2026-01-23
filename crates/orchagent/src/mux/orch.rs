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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mux_orch_new_default_config() {
        let config = MuxOrchConfig::default();
        let orch = MuxOrch::new(config);

        assert_eq!(orch.stats.stats.state_changes, 0);
        assert_eq!(orch.stats.errors, 0);
        assert_eq!(orch.ports.len(), 0);
    }

    #[test]
    fn test_mux_orch_new_with_config() {
        let config = MuxOrchConfig {
            enable_active_active: true,
            state_change_timeout_ms: 5000,
        };
        let orch = MuxOrch::new(config);

        assert_eq!(orch.stats().errors, 0);
    }

    #[test]
    fn test_mux_orch_config_with_timeout() {
        let config = MuxOrchConfig::default().with_timeout(10000);

        assert_eq!(config.state_change_timeout_ms, 10000);
    }

    #[test]
    fn test_mux_orch_get_port_not_found() {
        let orch = MuxOrch::new(MuxOrchConfig::default());

        assert!(orch.get_port("Ethernet0").is_none());
    }

    #[test]
    fn test_mux_orch_stats_access() {
        let orch = MuxOrch::new(MuxOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.stats.state_changes, 0);
        assert_eq!(stats.stats.active_transitions, 0);
        assert_eq!(stats.stats.standby_transitions, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_mux_orch_empty_initialization() {
        let orch = MuxOrch::new(MuxOrchConfig::default());

        assert_eq!(orch.ports.len(), 0);
        assert!(orch.get_port("any_port").is_none());
    }

    #[test]
    fn test_mux_orch_config_clone() {
        let config1 = MuxOrchConfig {
            enable_active_active: true,
            state_change_timeout_ms: 3000,
        };
        let config2 = config1.clone();

        assert_eq!(config1.enable_active_active, config2.enable_active_active);
        assert_eq!(config1.state_change_timeout_ms, config2.state_change_timeout_ms);
    }

    #[test]
    fn test_mux_orch_stats_default() {
        let stats = MuxOrchStats::default();

        assert_eq!(stats.stats.state_changes, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_mux_orch_stats_clone() {
        let mut stats1 = MuxOrchStats::default();
        stats1.errors = 10;
        stats1.stats.state_changes = 5;

        let stats2 = stats1.clone();

        assert_eq!(stats1.errors, stats2.errors);
        assert_eq!(stats1.stats.state_changes, stats2.stats.state_changes);
    }

    #[test]
    fn test_mux_orch_error_port_not_found() {
        let error = MuxOrchError::PortNotFound("Ethernet0".to_string());

        match error {
            MuxOrchError::PortNotFound(name) => {
                assert_eq!(name, "Ethernet0");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_mux_orch_error_invalid_state() {
        let error = MuxOrchError::InvalidState("bad_state".to_string());

        match error {
            MuxOrchError::InvalidState(state) => {
                assert_eq!(state, "bad_state");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_mux_orch_error_tunnel_creation_failed() {
        let error = MuxOrchError::TunnelCreationFailed("reason".to_string());

        match error {
            MuxOrchError::TunnelCreationFailed(reason) => {
                assert_eq!(reason, "reason");
            }
            _ => panic!("Wrong error type"),
        }
    }
}
