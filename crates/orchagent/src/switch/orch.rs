//! Switch orchestration logic.

use std::sync::Arc;
use super::types::{SwitchConfig, SwitchState, SwitchHashConfig, SwitchCapabilities, RawSaiObjectId};

pub type Result<T> = std::result::Result<T, SwitchOrchError>;

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
    fn initialize_switch(&self, capabilities: &SwitchCapabilities) -> Result<SwitchState>;
    fn set_hash_algorithm(&self, is_ecmp: bool, config: &SwitchHashConfig) -> Result<()>;
    fn get_capabilities(&self) -> Result<SwitchCapabilities>;
    fn set_switch_attribute(&self, attr_name: &str, attr_value: &str) -> Result<()>;
    fn get_switch_attribute(&self, attr_name: &str) -> Result<String>;
    fn on_switch_initialized(&self, state: &SwitchState);
    fn on_hash_updated(&self, is_ecmp: bool);
    fn on_warm_restart_begin(&self);
    fn on_warm_restart_end(&self, success: bool);
}

pub struct SwitchOrch<C: SwitchOrchCallbacks> {
    config: SwitchOrchConfig,
    stats: SwitchOrchStats,
    state: Option<SwitchState>,
    switch_config: SwitchConfig,
    callbacks: Option<Arc<C>>,
}

impl<C: SwitchOrchCallbacks> SwitchOrch<C> {
    pub fn new(config: SwitchOrchConfig) -> Self {
        Self {
            config,
            stats: SwitchOrchStats::default(),
            state: None,
            switch_config: SwitchConfig::default(),
            callbacks: None,
        }
    }

    pub fn with_callbacks(mut self, callbacks: Arc<C>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    pub fn initialize(&mut self) -> Result<()> {
        if self.state.is_some() {
            return Err(SwitchOrchError::ConfigurationError("Switch already initialized".into()));
        }

        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        let capabilities = callbacks.get_capabilities()?;
        let mut state = callbacks.initialize_switch(&capabilities)?;
        state.capabilities = capabilities;

        self.state = Some(state.clone());
        callbacks.on_switch_initialized(&state);

        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.state.is_some()
    }

    pub fn get_state(&self) -> Option<&SwitchState> {
        self.state.as_ref()
    }

    pub fn set_ecmp_hash(&mut self, config: SwitchHashConfig) -> Result<()> {
        if self.state.is_none() {
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.set_hash_algorithm(true, &config)?;

        self.switch_config.ecmp_hash = config;
        self.stats.hash_updates += 1;
        callbacks.on_hash_updated(true);

        Ok(())
    }

    pub fn set_lag_hash(&mut self, config: SwitchHashConfig) -> Result<()> {
        if self.state.is_none() {
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.set_hash_algorithm(false, &config)?;

        self.switch_config.lag_hash = config;
        self.stats.hash_updates += 1;
        callbacks.on_hash_updated(false);

        Ok(())
    }

    pub fn set_attribute(&mut self, name: String, value: String) -> Result<()> {
        if self.state.is_none() {
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.set_switch_attribute(&name, &value)?;

        if let Some(state) = &mut self.state {
            state.attributes.insert(name, value);
        }

        Ok(())
    }

    pub fn get_attribute(&self, name: &str) -> Result<String> {
        if self.state.is_none() {
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.get_switch_attribute(name)
    }

    pub fn query_capabilities(&mut self) -> Result<SwitchCapabilities> {
        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        let caps = callbacks.get_capabilities()?;
        self.stats.capability_queries += 1;

        if let Some(state) = &mut self.state {
            state.capabilities = caps.clone();
        }

        Ok(caps)
    }

    pub fn begin_warm_restart(&mut self) -> Result<()> {
        if self.state.is_none() {
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.on_warm_restart_begin();
        self.stats.warm_restarts += 1;

        Ok(())
    }

    pub fn end_warm_restart(&mut self, success: bool) -> Result<()> {
        if self.state.is_none() {
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self.callbacks.as_ref().ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.on_warm_restart_end(success);

        Ok(())
    }

    pub fn get_ecmp_hash(&self) -> &SwitchHashConfig {
        &self.switch_config.ecmp_hash
    }

    pub fn get_lag_hash(&self) -> &SwitchHashConfig {
        &self.switch_config.lag_hash
    }

    pub fn stats(&self) -> &SwitchOrchStats {
        &self.stats
    }

    pub fn stats_mut(&mut self) -> &mut SwitchOrchStats {
        &mut self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSwitchCallbacks;

    impl SwitchOrchCallbacks for MockSwitchCallbacks {
        fn initialize_switch(&self, _caps: &SwitchCapabilities) -> Result<SwitchState> {
            Ok(SwitchState {
                switch_oid: 1,
                cpu_port_oid: 100,
                default_vlan_oid: 200,
                default_1q_bridge_oid: 300,
                capabilities: SwitchCapabilities::default(),
                attributes: Default::default(),
            })
        }

        fn set_hash_algorithm(&self, _is_ecmp: bool, _config: &SwitchHashConfig) -> Result<()> {
            Ok(())
        }

        fn get_capabilities(&self) -> Result<SwitchCapabilities> {
            Ok(SwitchCapabilities::default())
        }

        fn set_switch_attribute(&self, _name: &str, _value: &str) -> Result<()> {
            Ok(())
        }

        fn get_switch_attribute(&self, name: &str) -> Result<String> {
            match name {
                "cpu_port" => Ok("100".to_string()),
                _ => Err(SwitchOrchError::ConfigurationError("Unknown attribute".into())),
            }
        }

        fn on_switch_initialized(&self, _state: &SwitchState) {}
        fn on_hash_updated(&self, _is_ecmp: bool) {}
        fn on_warm_restart_begin(&self) {}
        fn on_warm_restart_end(&self, _success: bool) {}
    }

    #[test]
    fn test_new_switch_orch_with_default_config() {
        let config = SwitchOrchConfig::default();
        let orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config);

        assert!(!orch.is_initialized());
        assert_eq!(orch.stats().hash_updates, 0);
        assert_eq!(orch.stats().capability_queries, 0);
        assert_eq!(orch.stats().warm_restarts, 0);
    }

    #[test]
    fn test_initialize_switch() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(!orch.is_initialized());
        assert!(orch.initialize().is_ok());
        assert!(orch.is_initialized());
        assert!(orch.get_state().is_some());
    }

    #[test]
    fn test_initialize_twice_returns_error() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        assert!(orch.initialize().is_err());
    }

    #[test]
    fn test_set_ecmp_hash() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());

        let new_config = SwitchHashConfig {
            algorithm: super::super::types::SwitchHashAlgorithm::Xor,
            fields: vec![super::super::types::SwitchHashField::SrcIp],
            seed: 42,
        };

        assert!(orch.set_ecmp_hash(new_config.clone()).is_ok());
        assert_eq!(orch.stats().hash_updates, 1);
        assert_eq!(orch.get_ecmp_hash().seed, 42);
    }

    #[test]
    fn test_set_lag_hash() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());

        let new_config = SwitchHashConfig {
            algorithm: super::super::types::SwitchHashAlgorithm::Random,
            fields: vec![super::super::types::SwitchHashField::DstIp],
            seed: 100,
        };

        assert!(orch.set_lag_hash(new_config.clone()).is_ok());
        assert_eq!(orch.stats().hash_updates, 1);
        assert_eq!(orch.get_lag_hash().seed, 100);
    }

    #[test]
    fn test_set_attribute() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        assert!(orch.set_attribute("max_mtu".to_string(), "9216".to_string()).is_ok());

        if let Some(state) = orch.get_state() {
            assert_eq!(state.attributes.get("max_mtu"), Some(&"9216".to_string()));
        }
    }

    #[test]
    fn test_get_attribute() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        let result = orch.get_attribute("cpu_port");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "100");
    }

    #[test]
    fn test_query_capabilities() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        assert_eq!(orch.stats().capability_queries, 0);

        let result = orch.query_capabilities();
        assert!(result.is_ok());
        assert_eq!(orch.stats().capability_queries, 1);
    }

    #[test]
    fn test_warm_restart_sequence() {
        let config = SwitchOrchConfig {
            enable_warm_restart: true,
            warm_restart_read_timer: 60,
            warm_restart_timer: 120,
        };
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        assert_eq!(orch.stats().warm_restarts, 0);

        assert!(orch.begin_warm_restart().is_ok());
        assert_eq!(orch.stats().warm_restarts, 1);

        assert!(orch.end_warm_restart(true).is_ok());
    }

    #[test]
    fn test_operations_fail_without_initialization() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> = SwitchOrch::new(config)
            .with_callbacks(Arc::new(MockSwitchCallbacks));

        let hash_config = SwitchHashConfig::default();
        assert!(orch.set_ecmp_hash(hash_config.clone()).is_err());
        assert!(orch.set_lag_hash(hash_config).is_err());
        assert!(orch.set_attribute("test".into(), "val".into()).is_err());
        assert!(orch.get_attribute("test").is_err());
        assert!(orch.begin_warm_restart().is_err());
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
}
