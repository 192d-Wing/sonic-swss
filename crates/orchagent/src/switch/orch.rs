//! Switch orchestration logic.

use super::types::{
    RawSaiObjectId, SwitchCapabilities, SwitchConfig, SwitchHashConfig, SwitchState,
};
use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::{audit_log, debug_log, error_log, info_log, security_audit, warn_log};
use std::sync::Arc;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SwitchOrchError>;

/// Switch orchestration errors with NIST-compliant error messages.
#[derive(Debug, Clone, Error)]
pub enum SwitchOrchError {
    /// Switch not initialized (callbacks not configured or initialize() not called)
    #[error("Switch orchestrator not initialized")]
    NotInitialized,

    /// Invalid hash algorithm specified
    #[error("Invalid hash algorithm: {0}")]
    InvalidHashAlgorithm(String),

    /// Invalid hash field specified
    #[error("Invalid hash field: {0}")]
    InvalidHashField(String),

    /// SAI operation failed
    #[error("SAI operation failed: {0}")]
    SaiError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Switch already initialized
    #[error("Switch already initialized")]
    AlreadyInitialized,
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
        info_log!("SwitchOrch", "Initializing switch orchestrator");

        if self.state.is_some() {
            warn_log!("SwitchOrch", "Switch already initialized");
            audit_log!(AuditRecord::new(
                AuditCategory::SystemLifecycle,
                "SwitchOrch",
                "initialize"
            )
            .with_error("Switch already initialized"));
            return Err(SwitchOrchError::AlreadyInitialized);
        }

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("SwitchOrch", "Callbacks not configured");
            SwitchOrchError::NotInitialized
        })?;

        let capabilities = callbacks.get_capabilities().map_err(|e| {
            error_log!("SwitchOrch", error = %e, "Failed to get switch capabilities");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "SwitchOrch",
                "get_capabilities"
            )
            .with_error(e.to_string()));
            e
        })?;

        debug_log!(
            "SwitchOrch",
            max_ports = capabilities.max_ports,
            max_vlans = capabilities.max_vlans,
            "Retrieved switch capabilities"
        );

        let mut state = callbacks.initialize_switch(&capabilities).map_err(|e| {
            error_log!("SwitchOrch", error = %e, "Failed to initialize switch");
            audit_log!(AuditRecord::new(
                AuditCategory::SystemLifecycle,
                "SwitchOrch",
                "initialize_switch"
            )
            .with_error(e.to_string()));
            e
        })?;

        state.capabilities = capabilities;
        let switch_oid = state.switch_oid;

        self.state = Some(state.clone());
        callbacks.on_switch_initialized(&state);

        info_log!(
            "SwitchOrch",
            switch_oid = switch_oid,
            cpu_port = state.cpu_port_oid,
            "Switch initialized successfully"
        );
        audit_log!(
            AuditRecord::new(AuditCategory::SystemLifecycle, "SwitchOrch", "initialize")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(format!("0x{:x}", switch_oid))
                .with_object_type("switch")
                .with_details(serde_json::json!({
                    "switch_oid": format!("0x{:x}", switch_oid),
                    "cpu_port_oid": format!("0x{:x}", state.cpu_port_oid),
                    "default_vlan_oid": format!("0x{:x}", state.default_vlan_oid),
                    "warm_restart_enabled": self.config.enable_warm_restart
                }))
        );

        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.state.is_some()
    }

    pub fn get_state(&self) -> Option<&SwitchState> {
        self.state.as_ref()
    }

    pub fn set_ecmp_hash(&mut self, config: SwitchHashConfig) -> Result<()> {
        debug_log!("SwitchOrch", algorithm = ?config.algorithm, seed = config.seed, "Setting ECMP hash configuration");

        if self.state.is_none() {
            error_log!("SwitchOrch", "Cannot set ECMP hash: switch not initialized");
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.set_hash_algorithm(true, &config).map_err(|e| {
            error_log!("SwitchOrch", error = %e, "SAI set_hash_algorithm failed for ECMP");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "SwitchOrch",
                "set_ecmp_hash"
            )
            .with_object_type("ecmp_hash")
            .with_error(e.to_string()));
            e
        })?;

        let old_seed = self.switch_config.ecmp_hash.seed;
        self.switch_config.ecmp_hash = config.clone();
        self.stats.hash_updates += 1;
        callbacks.on_hash_updated(true);

        info_log!("SwitchOrch", algorithm = ?config.algorithm, old_seed = old_seed, new_seed = config.seed, "ECMP hash configuration updated");
        audit_log!(AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "SwitchOrch",
            "set_ecmp_hash"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_type("ecmp_hash")
        .with_details(serde_json::json!({
            "algorithm": format!("{:?}", config.algorithm),
            "seed": config.seed,
            "fields_count": config.fields.len()
        })));

        Ok(())
    }

    pub fn set_lag_hash(&mut self, config: SwitchHashConfig) -> Result<()> {
        debug_log!("SwitchOrch", algorithm = ?config.algorithm, seed = config.seed, "Setting LAG hash configuration");

        if self.state.is_none() {
            error_log!("SwitchOrch", "Cannot set LAG hash: switch not initialized");
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.set_hash_algorithm(false, &config).map_err(|e| {
            error_log!("SwitchOrch", error = %e, "SAI set_hash_algorithm failed for LAG");
            audit_log!(
                AuditRecord::new(AuditCategory::SaiOperation, "SwitchOrch", "set_lag_hash")
                    .with_object_type("lag_hash")
                    .with_error(e.to_string())
            );
            e
        })?;

        let old_seed = self.switch_config.lag_hash.seed;
        self.switch_config.lag_hash = config.clone();
        self.stats.hash_updates += 1;
        callbacks.on_hash_updated(false);

        info_log!("SwitchOrch", algorithm = ?config.algorithm, old_seed = old_seed, new_seed = config.seed, "LAG hash configuration updated");
        audit_log!(AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "SwitchOrch",
            "set_lag_hash"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_type("lag_hash")
        .with_details(serde_json::json!({
            "algorithm": format!("{:?}", config.algorithm),
            "seed": config.seed,
            "fields_count": config.fields.len()
        })));

        Ok(())
    }

    pub fn set_attribute(&mut self, name: String, value: String) -> Result<()> {
        debug_log!("SwitchOrch", attribute = %name, value = %value, "Setting switch attribute");

        if self.state.is_none() {
            error_log!("SwitchOrch", attribute = %name, "Cannot set attribute: switch not initialized");
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or(SwitchOrchError::NotInitialized)?;

        let old_value = self
            .state
            .as_ref()
            .and_then(|s| s.attributes.get(&name).cloned());

        callbacks.set_switch_attribute(&name, &value).map_err(|e| {
            error_log!("SwitchOrch", attribute = %name, error = %e, "SAI set_switch_attribute failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "SwitchOrch",
                "set_switch_attribute"
            )
            .with_object_id(&name)
            .with_object_type("switch_attribute")
            .with_error(e.to_string()));
            e
        })?;

        if let Some(state) = &mut self.state {
            state.attributes.insert(name.clone(), value.clone());
        }

        info_log!("SwitchOrch", attribute = %name, old_value = ?old_value, new_value = %value, "Switch attribute updated");
        audit_log!(AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "SwitchOrch",
            "set_attribute"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&name)
        .with_object_type("switch_attribute")
        .with_details(serde_json::json!({
            "attribute_name": name,
            "old_value": old_value,
            "new_value": value
        })));

        Ok(())
    }

    pub fn get_attribute(&self, name: &str) -> Result<String> {
        if self.state.is_none() {
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.get_switch_attribute(name)
    }

    pub fn query_capabilities(&mut self) -> Result<SwitchCapabilities> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or(SwitchOrchError::NotInitialized)?;
        let caps = callbacks.get_capabilities()?;
        self.stats.capability_queries += 1;

        if let Some(state) = &mut self.state {
            state.capabilities = caps.clone();
        }

        Ok(caps)
    }

    pub fn begin_warm_restart(&mut self) -> Result<()> {
        info_log!("SwitchOrch", "Beginning warm restart sequence");

        if self.state.is_none() {
            error_log!(
                "SwitchOrch",
                "Cannot begin warm restart: switch not initialized"
            );
            security_audit!(AuditRecord::new(
                AuditCategory::WarmRestart,
                "SwitchOrch",
                "begin_warm_restart"
            )
            .with_error("Switch not initialized"));
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.on_warm_restart_begin();
        self.stats.warm_restarts += 1;

        // Warm restart is a security-relevant event (NIST AU-2)
        security_audit!(AuditRecord::new(
            AuditCategory::WarmRestart,
            "SwitchOrch",
            "begin_warm_restart"
        )
        .with_outcome(AuditOutcome::InProgress)
        .with_object_type("switch")
        .with_details(serde_json::json!({
            "warm_restart_count": self.stats.warm_restarts,
            "warm_restart_timer": self.config.warm_restart_timer
        })));

        Ok(())
    }

    pub fn end_warm_restart(&mut self, success: bool) -> Result<()> {
        info_log!(
            "SwitchOrch",
            success = success,
            "Ending warm restart sequence"
        );

        if self.state.is_none() {
            error_log!(
                "SwitchOrch",
                "Cannot end warm restart: switch not initialized"
            );
            return Err(SwitchOrchError::NotInitialized);
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or(SwitchOrchError::NotInitialized)?;
        callbacks.on_warm_restart_end(success);

        let outcome = if success {
            AuditOutcome::Success
        } else {
            AuditOutcome::Failure
        };

        // Warm restart completion is a security-relevant event (NIST AU-2)
        security_audit!(AuditRecord::new(
            AuditCategory::WarmRestart,
            "SwitchOrch",
            "end_warm_restart"
        )
        .with_outcome(outcome)
        .with_object_type("switch")
        .with_details(serde_json::json!({
            "success": success,
            "total_warm_restarts": self.stats.warm_restarts
        })));

        if success {
            info_log!("SwitchOrch", "Warm restart completed successfully");
        } else {
            warn_log!("SwitchOrch", "Warm restart completed with failures");
        }

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
                _ => Err(SwitchOrchError::ConfigurationError(
                    "Unknown attribute".into(),
                )),
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
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(!orch.is_initialized());
        assert!(orch.initialize().is_ok());
        assert!(orch.is_initialized());
        assert!(orch.get_state().is_some());
    }

    #[test]
    fn test_initialize_twice_returns_error() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        assert!(orch.initialize().is_err());
    }

    #[test]
    fn test_set_ecmp_hash() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

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
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

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
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        assert!(orch
            .set_attribute("max_mtu".to_string(), "9216".to_string())
            .is_ok());

        if let Some(state) = orch.get_state() {
            assert_eq!(state.attributes.get("max_mtu"), Some(&"9216".to_string()));
        }
    }

    #[test]
    fn test_get_attribute() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        let result = orch.get_attribute("cpu_port");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "100");
    }

    #[test]
    fn test_query_capabilities() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

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
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

        assert!(orch.initialize().is_ok());
        assert_eq!(orch.stats().warm_restarts, 0);

        assert!(orch.begin_warm_restart().is_ok());
        assert_eq!(orch.stats().warm_restarts, 1);

        assert!(orch.end_warm_restart(true).is_ok());
    }

    #[test]
    fn test_operations_fail_without_initialization() {
        let config = SwitchOrchConfig::default();
        let mut orch: SwitchOrch<MockSwitchCallbacks> =
            SwitchOrch::new(config).with_callbacks(Arc::new(MockSwitchCallbacks));

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
        let err6 = SwitchOrchError::AlreadyInitialized;

        assert!(matches!(err1, SwitchOrchError::NotInitialized));
        assert!(matches!(err2, SwitchOrchError::InvalidHashAlgorithm(_)));
        assert!(matches!(err3, SwitchOrchError::InvalidHashField(_)));
        assert!(matches!(err4, SwitchOrchError::SaiError(_)));
        assert!(matches!(err5, SwitchOrchError::ConfigurationError(_)));
        assert!(matches!(err6, SwitchOrchError::AlreadyInitialized));

        // Test thiserror Display implementations
        assert_eq!(err1.to_string(), "Switch orchestrator not initialized");
        assert_eq!(err6.to_string(), "Switch already initialized");
    }
}
