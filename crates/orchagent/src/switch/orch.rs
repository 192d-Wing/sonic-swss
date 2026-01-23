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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_switch_orch_with_default_config() {
        let config = SwitchOrchConfig::default();
        let orch = SwitchOrch::new(config);

        assert!(!orch.is_initialized());
        assert_eq!(orch.stats().hash_updates, 0);
        assert_eq!(orch.stats().capability_queries, 0);
        assert_eq!(orch.stats().warm_restarts, 0);
    }

    #[test]
    fn test_new_switch_orch_with_warm_restart_enabled() {
        let config = SwitchOrchConfig {
            enable_warm_restart: true,
            warm_restart_read_timer: 60,
            warm_restart_timer: 120,
        };
        let orch = SwitchOrch::new(config.clone());

        assert!(!orch.is_initialized());
        assert_eq!(orch.config.enable_warm_restart, true);
        assert_eq!(orch.config.warm_restart_read_timer, 60);
        assert_eq!(orch.config.warm_restart_timer, 120);
    }

    #[test]
    fn test_is_initialized_returns_false_by_default() {
        let config = SwitchOrchConfig::default();
        let orch = SwitchOrch::new(config);

        assert!(!orch.is_initialized());
    }

    #[test]
    fn test_get_state_returns_none_when_not_initialized() {
        let config = SwitchOrchConfig::default();
        let orch = SwitchOrch::new(config);

        assert!(orch.get_state().is_none());
    }

    #[test]
    fn test_stats_returns_initial_zero_values() {
        let config = SwitchOrchConfig::default();
        let orch = SwitchOrch::new(config);
        let stats = orch.stats();

        assert_eq!(stats.hash_updates, 0);
        assert_eq!(stats.capability_queries, 0);
        assert_eq!(stats.warm_restarts, 0);
    }

    #[test]
    fn test_switch_orch_config_default() {
        let config = SwitchOrchConfig::default();

        assert_eq!(config.enable_warm_restart, false);
        assert_eq!(config.warm_restart_read_timer, 0);
        assert_eq!(config.warm_restart_timer, 0);
    }

    #[test]
    fn test_switch_orch_stats_default() {
        let stats = SwitchOrchStats::default();

        assert_eq!(stats.hash_updates, 0);
        assert_eq!(stats.capability_queries, 0);
        assert_eq!(stats.warm_restarts, 0);
    }

    #[test]
    fn test_switch_orch_error_variants() {
        let err1 = SwitchOrchError::NotInitialized;
        let err2 = SwitchOrchError::InvalidHashAlgorithm("unknown".to_string());
        let err3 = SwitchOrchError::InvalidHashField("invalid_field".to_string());
        let err4 = SwitchOrchError::SaiError("SAI error".to_string());
        let err5 = SwitchOrchError::ConfigurationError("Config error".to_string());

        assert!(matches!(err1, SwitchOrchError::NotInitialized));
        assert!(matches!(err2, SwitchOrchError::InvalidHashAlgorithm(_)));
        assert!(matches!(err3, SwitchOrchError::InvalidHashField(_)));
        assert!(matches!(err4, SwitchOrchError::SaiError(_)));
        assert!(matches!(err5, SwitchOrchError::ConfigurationError(_)));
    }

    #[test]
    fn test_switch_orch_error_clone() {
        let err = SwitchOrchError::InvalidHashAlgorithm("crc128".to_string());
        let cloned = err.clone();

        assert!(matches!(cloned, SwitchOrchError::InvalidHashAlgorithm(_)));
    }

    #[test]
    fn test_switch_orch_config_clone() {
        let config = SwitchOrchConfig {
            enable_warm_restart: true,
            warm_restart_read_timer: 30,
            warm_restart_timer: 90,
        };
        let cloned = config.clone();

        assert_eq!(cloned.enable_warm_restart, true);
        assert_eq!(cloned.warm_restart_read_timer, 30);
        assert_eq!(cloned.warm_restart_timer, 90);
    }

    #[test]
    fn test_switch_orch_stats_clone() {
        let stats = SwitchOrchStats {
            hash_updates: 10,
            capability_queries: 5,
            warm_restarts: 2,
        };
        let cloned = stats.clone();

        assert_eq!(cloned.hash_updates, 10);
        assert_eq!(cloned.capability_queries, 5);
        assert_eq!(cloned.warm_restarts, 2);
    }

    #[test]
    fn test_multiple_switch_orch_instances() {
        let config1 = SwitchOrchConfig {
            enable_warm_restart: true,
            warm_restart_read_timer: 60,
            warm_restart_timer: 120,
        };
        let config2 = SwitchOrchConfig::default();

        let orch1 = SwitchOrch::new(config1);
        let orch2 = SwitchOrch::new(config2);

        assert_eq!(orch1.config.enable_warm_restart, true);
        assert_eq!(orch2.config.enable_warm_restart, false);
    }
}
