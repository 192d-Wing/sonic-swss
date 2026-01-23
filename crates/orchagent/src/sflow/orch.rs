//! SflowOrch implementation.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use sonic_sai::types::RawSaiObjectId;

use super::types::{PortSflowInfo, SampleDirection, SflowConfig, SflowSession};

/// Sflow orchestrator error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SflowOrchError {
    /// Port not found.
    PortNotFound(String),
    /// Port not ready.
    PortNotReady,
    /// Invalid configuration.
    InvalidConfig(String),
    /// SAI error.
    SaiError(String),
    /// Session not found.
    SessionNotFound(RawSaiObjectId),
}

impl std::fmt::Display for SflowOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PortNotFound(alias) => write!(f, "Port not found: {}", alias),
            Self::PortNotReady => write!(f, "Ports not ready"),
            Self::InvalidConfig(msg) => write!(f, "Invalid sflow config: {}", msg),
            Self::SaiError(msg) => write!(f, "SAI error: {}", msg),
            Self::SessionNotFound(oid) => write!(f, "Sflow session not found: 0x{:x}", oid),
        }
    }
}

impl std::error::Error for SflowOrchError {}

/// Callbacks for SflowOrch operations.
pub trait SflowOrchCallbacks: Send + Sync {
    /// Creates a samplepacket session via SAI.
    fn create_samplepacket_session(&self, rate: NonZeroU32) -> Result<RawSaiObjectId, String>;

    /// Removes a samplepacket session via SAI.
    fn remove_samplepacket_session(&self, session_id: RawSaiObjectId) -> Result<(), String>;

    /// Enables ingress sampling on a port.
    fn enable_port_ingress_sample(&self, port_id: RawSaiObjectId, session_id: RawSaiObjectId) -> Result<(), String>;

    /// Disables ingress sampling on a port.
    fn disable_port_ingress_sample(&self, port_id: RawSaiObjectId) -> Result<(), String>;

    /// Enables egress sampling on a port.
    fn enable_port_egress_sample(&self, port_id: RawSaiObjectId, session_id: RawSaiObjectId) -> Result<(), String>;

    /// Disables egress sampling on a port.
    fn disable_port_egress_sample(&self, port_id: RawSaiObjectId) -> Result<(), String>;

    /// Gets port SAI object ID by alias.
    fn get_port_id(&self, alias: &str) -> Option<RawSaiObjectId>;

    /// Returns true if all ports are ready.
    fn all_ports_ready(&self) -> bool;
}

/// Sflow orchestrator configuration.
#[derive(Debug, Clone, Default)]
pub struct SflowOrchConfig {
    // Currently no configuration options, but reserved for future use
}

/// Sflow orchestrator statistics.
#[derive(Debug, Clone, Default)]
pub struct SflowOrchStats {
    /// Number of sessions created.
    pub sessions_created: u64,
    /// Number of sessions destroyed.
    pub sessions_destroyed: u64,
    /// Number of ports configured.
    pub ports_configured: u64,
    /// Number of ports unconfigured.
    pub ports_unconfigured: u64,
    /// Number of rate updates.
    pub rate_updates: u64,
    /// Number of direction updates.
    pub direction_updates: u64,
}

/// Sflow orchestrator for packet sampling.
pub struct SflowOrch {
    /// Configuration.
    config: SflowOrchConfig,
    /// Global sflow enable/disable status.
    enabled: bool,
    /// Map from port SAI OID to port sflow info.
    port_info: HashMap<RawSaiObjectId, PortSflowInfo>,
    /// Map from sample rate to session.
    sessions: HashMap<NonZeroU32, SflowSession>,
    /// Reverse index: session ID -> rate (for O(1) lookups).
    session_to_rate: HashMap<RawSaiObjectId, NonZeroU32>,
    /// Callbacks for SAI and port queries.
    callbacks: Option<Arc<dyn SflowOrchCallbacks>>,
    /// Whether the orch is initialized.
    initialized: bool,
    /// Statistics.
    stats: SflowOrchStats,
}

impl std::fmt::Debug for SflowOrch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SflowOrch")
            .field("config", &self.config)
            .field("enabled", &self.enabled)
            .field("port_count", &self.port_info.len())
            .field("session_count", &self.sessions.len())
            .field("initialized", &self.initialized)
            .field("stats", &self.stats)
            .finish()
    }
}

impl SflowOrch {
    /// Creates a new SflowOrch with the given configuration.
    pub fn new(config: SflowOrchConfig) -> Self {
        Self {
            config,
            enabled: false,
            port_info: HashMap::new(),
            sessions: HashMap::new(),
            session_to_rate: HashMap::new(),
            callbacks: None,
            initialized: false,
            stats: SflowOrchStats::default(),
        }
    }

    /// Sets the callbacks for this orch.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn SflowOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the configuration.
    pub fn config(&self) -> &SflowOrchConfig {
        &self.config
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &SflowOrchStats {
        &self.stats
    }

    /// Returns true if the orch is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Marks the orch as initialized.
    pub fn set_initialized(&mut self) {
        self.initialized = true;
    }

    /// Returns true if sflow is globally enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets the global sflow enable/disable status.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns the number of configured ports.
    pub fn port_count(&self) -> usize {
        self.port_info.len()
    }

    /// Returns the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Gets port sflow info by port SAI OID.
    pub fn get_port_info(&self, port_id: RawSaiObjectId) -> Option<&PortSflowInfo> {
        self.port_info.get(&port_id)
    }

    /// Gets the sample rate for a session ID.
    pub fn get_session_rate(&self, session_id: RawSaiObjectId) -> Option<NonZeroU32> {
        self.session_to_rate.get(&session_id).copied()
    }

    /// Creates a new samplepacket session with the given rate.
    fn create_session(&mut self, rate: NonZeroU32) -> Result<(), SflowOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| SflowOrchError::InvalidConfig("No callbacks set".to_string()))?;

        // Check if session already exists
        if self.sessions.contains_key(&rate) {
            return Ok(()); // Already exists
        }

        let session_id = callbacks
            .create_samplepacket_session(rate)
            .map_err(SflowOrchError::SaiError)?;

        let session = SflowSession::new(session_id, rate);
        self.sessions.insert(rate, session);
        self.session_to_rate.insert(session_id, rate);
        self.stats.sessions_created += 1;

        Ok(())
    }

    /// Destroys a samplepacket session.
    fn destroy_session(&mut self, rate: NonZeroU32) -> Result<(), SflowOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| SflowOrchError::InvalidConfig("No callbacks set".to_string()))?;

        let session = self
            .sessions
            .remove(&rate)
            .ok_or_else(|| SflowOrchError::InvalidConfig(format!("Session not found for rate {}", rate)))?;

        self.session_to_rate.remove(&session.session_id);

        callbacks
            .remove_samplepacket_session(session.session_id)
            .map_err(SflowOrchError::SaiError)?;

        self.stats.sessions_destroyed += 1;

        Ok(())
    }

    /// Applies sflow sampling to a port.
    fn apply_port_sampling(
        &self,
        port_id: RawSaiObjectId,
        session_id: RawSaiObjectId,
        direction: SampleDirection,
    ) -> Result<(), SflowOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| SflowOrchError::InvalidConfig("No callbacks set".to_string()))?;

        if direction.has_ingress() {
            callbacks
                .enable_port_ingress_sample(port_id, session_id)
                .map_err(SflowOrchError::SaiError)?;
        }

        if direction.has_egress() {
            callbacks
                .enable_port_egress_sample(port_id, session_id)
                .map_err(SflowOrchError::SaiError)?;
        }

        Ok(())
    }

    /// Removes sflow sampling from a port.
    fn remove_port_sampling(
        &self,
        port_id: RawSaiObjectId,
        direction: SampleDirection,
    ) -> Result<(), SflowOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| SflowOrchError::InvalidConfig("No callbacks set".to_string()))?;

        if direction.has_ingress() {
            callbacks
                .disable_port_ingress_sample(port_id)
                .map_err(SflowOrchError::SaiError)?;
        }

        if direction.has_egress() {
            callbacks
                .disable_port_egress_sample(port_id)
                .map_err(SflowOrchError::SaiError)?;
        }

        Ok(())
    }

    /// Configures sflow on a port.
    pub fn configure_port(&mut self, alias: &str, config: SflowConfig) -> Result<(), SflowOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| SflowOrchError::InvalidConfig("No callbacks set".to_string()))?;

        // Check if ports are ready
        if !callbacks.all_ports_ready() {
            return Err(SflowOrchError::PortNotReady);
        }

        // Check if sflow is globally enabled
        if !self.enabled {
            return Ok(()); // Silently ignore if disabled
        }

        // Get port ID
        let port_id = callbacks
            .get_port_id(alias)
            .ok_or_else(|| SflowOrchError::PortNotFound(alias.to_string()))?;

        // Get rate (required)
        let rate = config
            .rate
            .ok_or_else(|| SflowOrchError::InvalidConfig("Sample rate required".to_string()))?;

        // Get or create session
        self.create_session(rate)?;

        // Get session_id (not the mutable session itself)
        let session_id = self
            .sessions
            .get(&rate)
            .ok_or_else(|| SflowOrchError::InvalidConfig("Session should exist".to_string()))?
            .session_id;

        // Check if port already configured
        let is_existing = self.port_info.contains_key(&port_id);

        if is_existing {
            // Update existing configuration
            let old_session_id = self
                .port_info
                .get(&port_id)
                .ok_or_else(|| SflowOrchError::PortNotFound(alias.to_string()))?
                .session_id;

            let old_direction = self
                .port_info
                .get(&port_id)
                .ok_or_else(|| SflowOrchError::PortNotFound(alias.to_string()))?
                .direction;

            let old_rate = self
                .get_session_rate(old_session_id)
                .ok_or_else(|| SflowOrchError::SessionNotFound(old_session_id))?;

            // Handle rate change
            if old_rate != rate {
                // Remove from old session
                if let Some(old_session) = self.sessions.get_mut(&old_rate) {
                    let new_ref_count = old_session.remove_ref();
                    if new_ref_count == 0 {
                        // Destroy unused session
                        self.destroy_session(old_rate)?;
                    }
                }

                // Add to new session
                if let Some(new_session) = self.sessions.get_mut(&rate) {
                    new_session.add_ref();
                }

                // Reapply sampling with new session
                self.apply_port_sampling(port_id, session_id, config.direction)?;
                self.stats.rate_updates += 1;

                // Update session_id in port_info
                if let Some(info) = self.port_info.get_mut(&port_id) {
                    info.session_id = session_id;
                }
            }

            // Handle direction change
            if old_direction != config.direction {
                // Remove old direction
                self.remove_port_sampling(port_id, old_direction)?;
                // Apply new direction
                self.apply_port_sampling(port_id, session_id, config.direction)?;
                self.stats.direction_updates += 1;

                // Update direction in port_info
                if let Some(info) = self.port_info.get_mut(&port_id) {
                    info.direction = config.direction;
                }
            }

            // Update admin state
            if let Some(info) = self.port_info.get_mut(&port_id) {
                info.admin_state = config.admin_state;
            }
        } else {
            // New port configuration
            self.apply_port_sampling(port_id, session_id, config.direction)?;

            let info = PortSflowInfo::new(config.admin_state, config.direction, session_id);
            self.port_info.insert(port_id, info);

            // Increment ref count
            if let Some(session) = self.sessions.get_mut(&rate) {
                session.add_ref();
            }
            self.stats.ports_configured += 1;
        }

        Ok(())
    }

    /// Removes sflow configuration from a port.
    pub fn remove_port(&mut self, alias: &str) -> Result<(), SflowOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| SflowOrchError::InvalidConfig("No callbacks set".to_string()))?;

        // Get port ID
        let port_id = callbacks
            .get_port_id(alias)
            .ok_or_else(|| SflowOrchError::PortNotFound(alias.to_string()))?;

        // Get existing info
        let info = self
            .port_info
            .remove(&port_id)
            .ok_or_else(|| SflowOrchError::PortNotFound(alias.to_string()))?;

        // Remove sampling from port
        self.remove_port_sampling(port_id, info.direction)?;

        // Decrement session ref count
        let rate = self
            .get_session_rate(info.session_id)
            .ok_or_else(|| SflowOrchError::SessionNotFound(info.session_id))?;

        if let Some(session) = self.sessions.get_mut(&rate) {
            let new_ref_count = session.remove_ref();
            if new_ref_count == 0 {
                // Destroy unused session
                self.destroy_session(rate)?;
            }
        }

        self.stats.ports_unconfigured += 1;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestCallbacks {
        created_sessions: Mutex<Vec<(RawSaiObjectId, NonZeroU32)>>,
        removed_sessions: Mutex<Vec<RawSaiObjectId>>,
        port_ops: Mutex<Vec<String>>,
        next_session_id: Mutex<RawSaiObjectId>,
        ports_ready: bool,
    }

    impl TestCallbacks {
        fn new() -> Self {
            Self {
                created_sessions: Mutex::new(Vec::new()),
                removed_sessions: Mutex::new(Vec::new()),
                port_ops: Mutex::new(Vec::new()),
                next_session_id: Mutex::new(0x1000),
                ports_ready: true,
            }
        }

        fn with_ports_ready(ports_ready: bool) -> Self {
            Self {
                ports_ready,
                ..Self::new()
            }
        }
    }

    impl SflowOrchCallbacks for TestCallbacks {
        fn create_samplepacket_session(&self, rate: NonZeroU32) -> Result<RawSaiObjectId, String> {
            let mut next_id = self.next_session_id.lock().unwrap();
            let session_id = *next_id;
            *next_id += 1;
            self.created_sessions
                .lock()
                .unwrap()
                .push((session_id, rate));
            Ok(session_id)
        }

        fn remove_samplepacket_session(&self, session_id: RawSaiObjectId) -> Result<(), String> {
            self.removed_sessions.lock().unwrap().push(session_id);
            Ok(())
        }

        fn enable_port_ingress_sample(&self, port_id: RawSaiObjectId, session_id: RawSaiObjectId) -> Result<(), String> {
            self.port_ops
                .lock()
                .unwrap()
                .push(format!("enable_ingress:{}:{}", port_id, session_id));
            Ok(())
        }

        fn disable_port_ingress_sample(&self, port_id: RawSaiObjectId) -> Result<(), String> {
            self.port_ops
                .lock()
                .unwrap()
                .push(format!("disable_ingress:{}", port_id));
            Ok(())
        }

        fn enable_port_egress_sample(&self, port_id: RawSaiObjectId, session_id: RawSaiObjectId) -> Result<(), String> {
            self.port_ops
                .lock()
                .unwrap()
                .push(format!("enable_egress:{}:{}", port_id, session_id));
            Ok(())
        }

        fn disable_port_egress_sample(&self, port_id: RawSaiObjectId) -> Result<(), String> {
            self.port_ops
                .lock()
                .unwrap()
                .push(format!("disable_egress:{}", port_id));
            Ok(())
        }

        fn get_port_id(&self, alias: &str) -> Option<RawSaiObjectId> {
            // Simple mapping for testing
            match alias {
                "Ethernet0" => Some(0x100),
                "Ethernet4" => Some(0x104),
                _ => None,
            }
        }

        fn all_ports_ready(&self) -> bool {
            self.ports_ready
        }
    }

    #[test]
    fn test_sflow_orch_new() {
        let orch = SflowOrch::new(SflowOrchConfig::default());
        assert!(!orch.is_initialized());
        assert!(!orch.is_enabled());
        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.session_count(), 0);
    }

    #[test]
    fn test_set_enabled() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        assert!(!orch.is_enabled());

        orch.set_enabled(true);
        assert!(orch.is_enabled());

        orch.set_enabled(false);
        assert!(!orch.is_enabled());
    }

    #[test]
    fn test_configure_port() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.admin_state = true;
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Rx;

        let result = orch.configure_port("Ethernet0", config);
        assert!(result.is_ok());

        assert_eq!(orch.port_count(), 1);
        assert_eq!(orch.session_count(), 1);

        // Check session created
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].1, NonZeroU32::new(4096).unwrap());

        // Check port ops
        let ops = callbacks.port_ops.lock().unwrap();
        assert_eq!(ops.len(), 1);
        assert!(ops[0].starts_with("enable_ingress:256:")); // 0x100 = 256
    }

    #[test]
    fn test_configure_port_disabled() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        // Don't enable sflow

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        let result = orch.configure_port("Ethernet0", config);
        assert!(result.is_ok());

        // Should be silently ignored
        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.session_count(), 0);
    }

    #[test]
    fn test_configure_port_not_ready() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::with_ports_ready(false));
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        let result = orch.configure_port("Ethernet0", config);
        assert!(matches!(result, Err(SflowOrchError::PortNotReady)));
    }

    #[test]
    fn test_configure_port_both_direction() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Both;

        orch.configure_port("Ethernet0", config).unwrap();

        // Check both ingress and egress enabled
        let ops = callbacks.port_ops.lock().unwrap();
        assert_eq!(ops.len(), 2);
        assert!(ops.iter().any(|s| s.starts_with("enable_ingress:")));
        assert!(ops.iter().any(|s| s.starts_with("enable_egress:")));
    }

    #[test]
    fn test_session_sharing() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        // Configure two ports with same rate
        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config).unwrap();

        assert_eq!(orch.port_count(), 2);
        assert_eq!(orch.session_count(), 1); // Shared session

        // Only one session created
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 1);

        // Check ref count
        let session = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session.ref_count, 2);
    }

    #[test]
    fn test_rate_update() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        // Initial config
        orch.configure_port("Ethernet0", config.clone()).unwrap();

        // Update rate
        config.rate = NonZeroU32::new(8192);
        orch.configure_port("Ethernet0", config).unwrap();

        // Should have two sessions now (old one destroyed, new one created)
        assert_eq!(orch.session_count(), 1);
        assert_eq!(orch.stats().rate_updates, 1);

        // Old session should be destroyed
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_direction_update() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Rx;

        // Initial config
        orch.configure_port("Ethernet0", config.clone()).unwrap();

        // Update direction
        config.direction = SampleDirection::Both;
        orch.configure_port("Ethernet0", config).unwrap();

        assert_eq!(orch.stats().direction_updates, 1);

        // Check port ops: disable rx, then enable both rx+tx
        let ops = callbacks.port_ops.lock().unwrap();
        assert!(ops.iter().any(|s| s.starts_with("disable_ingress:")));
        assert!(ops.iter().filter(|s| s.starts_with("enable_ingress:")).count() == 2); // Initial + update
        assert!(ops.iter().any(|s| s.starts_with("enable_egress:")));
    }

    #[test]
    fn test_remove_port() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config).unwrap();
        assert_eq!(orch.port_count(), 1);

        orch.remove_port("Ethernet0").unwrap();
        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.session_count(), 0); // Session destroyed

        // Check session removed
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 1);

        // Check port ops
        let ops = callbacks.port_ops.lock().unwrap();
        assert!(ops.iter().any(|s| s.starts_with("disable_ingress:")));
    }

    #[test]
    fn test_remove_port_shared_session() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        // Configure two ports
        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config).unwrap();

        // Remove one port
        orch.remove_port("Ethernet0").unwrap();

        // Session should still exist
        assert_eq!(orch.session_count(), 1);
        {
            let removed = callbacks.removed_sessions.lock().unwrap();
            assert_eq!(removed.len(), 0);
        }

        // Remove second port
        orch.remove_port("Ethernet4").unwrap();

        // Now session should be destroyed
        assert_eq!(orch.session_count(), 0);
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_statistics() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config.clone()).unwrap();

        config.rate = NonZeroU32::new(8192);
        orch.configure_port("Ethernet0", config).unwrap();

        orch.remove_port("Ethernet4").unwrap();

        let stats = orch.stats();
        assert_eq!(stats.sessions_created, 2); // 4096 and 8192
        assert_eq!(stats.sessions_destroyed, 1); // 4096 destroyed
        assert_eq!(stats.ports_configured, 2);
        assert_eq!(stats.ports_unconfigured, 1);
        assert_eq!(stats.rate_updates, 1);
    }

    // ==================== Additional Comprehensive Tests ====================

    // 1. sFlow Session Management Tests

    #[test]
    fn test_session_creation_with_sample_rate() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(8192);

        orch.configure_port("Ethernet0", config).unwrap();

        assert_eq!(orch.session_count(), 1);
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].1, NonZeroU32::new(8192).unwrap());
    }

    #[test]
    fn test_session_sharing_multiple_ports_same_rate() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config).unwrap();

        // Only one session should be created
        assert_eq!(orch.session_count(), 1);
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 1);

        // Verify both ports share the same session
        let port0_info = orch.get_port_info(0x100).unwrap();
        let port1_info = orch.get_port_info(0x104).unwrap();
        assert_eq!(port0_info.session_id, port1_info.session_id);
    }

    #[test]
    fn test_session_reference_counting_increment() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        let session = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session.ref_count, 1);

        orch.configure_port("Ethernet4", config).unwrap();
        let session = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session.ref_count, 2);
    }

    #[test]
    fn test_session_removal_when_refcount_zero() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config).unwrap();
        assert_eq!(orch.session_count(), 1);

        orch.remove_port("Ethernet0").unwrap();
        assert_eq!(orch.session_count(), 0);

        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_unique_sessions_for_different_rates() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config1 = SflowConfig::new();
        config1.rate = NonZeroU32::new(4096);

        let mut config2 = SflowConfig::new();
        config2.rate = NonZeroU32::new(8192);

        orch.configure_port("Ethernet0", config1).unwrap();
        orch.configure_port("Ethernet4", config2).unwrap();

        // Two different sessions should be created
        assert_eq!(orch.session_count(), 2);
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 2);

        // Verify different session IDs
        let port0_info = orch.get_port_info(0x100).unwrap();
        let port1_info = orch.get_port_info(0x104).unwrap();
        assert_ne!(port0_info.session_id, port1_info.session_id);
    }

    // 2. Port Sampling Configuration Tests

    #[test]
    fn test_enable_rx_sampling_on_port() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Rx;

        orch.configure_port("Ethernet0", config).unwrap();

        let ops = callbacks.port_ops.lock().unwrap();
        assert!(ops.iter().any(|s| s.starts_with("enable_ingress:")));
        assert!(!ops.iter().any(|s| s.starts_with("enable_egress:")));
    }

    #[test]
    fn test_enable_tx_sampling_on_port() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Tx;

        orch.configure_port("Ethernet0", config).unwrap();

        let ops = callbacks.port_ops.lock().unwrap();
        assert!(!ops.iter().any(|s| s.starts_with("enable_ingress:")));
        assert!(ops.iter().any(|s| s.starts_with("enable_egress:")));
    }

    #[test]
    fn test_enable_both_directions_sampling() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Both;

        orch.configure_port("Ethernet0", config).unwrap();

        let ops = callbacks.port_ops.lock().unwrap();
        assert!(ops.iter().any(|s| s.starts_with("enable_ingress:")));
        assert!(ops.iter().any(|s| s.starts_with("enable_egress:")));
    }

    #[test]
    fn test_disable_sampling_on_port() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Both;

        orch.configure_port("Ethernet0", config).unwrap();
        orch.remove_port("Ethernet0").unwrap();

        let ops = callbacks.port_ops.lock().unwrap();
        assert!(ops.iter().any(|s| s.starts_with("disable_ingress:")));
        assert!(ops.iter().any(|s| s.starts_with("disable_egress:")));
    }

    #[test]
    fn test_update_sampling_direction_rx_to_tx() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Rx;

        orch.configure_port("Ethernet0", config.clone()).unwrap();

        config.direction = SampleDirection::Tx;
        orch.configure_port("Ethernet0", config).unwrap();

        let ops = callbacks.port_ops.lock().unwrap();
        assert!(ops.iter().any(|s| s.starts_with("disable_ingress:")));
        assert!(ops.iter().any(|s| s.starts_with("enable_egress:")));
    }

    #[test]
    fn test_update_sampling_direction_tx_to_both() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Tx;

        orch.configure_port("Ethernet0", config.clone()).unwrap();

        config.direction = SampleDirection::Both;
        orch.configure_port("Ethernet0", config).unwrap();

        assert_eq!(orch.stats().direction_updates, 1);
    }

    #[test]
    fn test_changing_rate_triggers_session_change() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        let old_session_id = orch.get_port_info(0x100).unwrap().session_id;

        config.rate = NonZeroU32::new(8192);
        orch.configure_port("Ethernet0", config).unwrap();
        let new_session_id = orch.get_port_info(0x100).unwrap().session_id;

        assert_ne!(old_session_id, new_session_id);
    }

    #[test]
    fn test_multiple_ports_share_same_session() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config).unwrap();

        let port0_session = orch.get_port_info(0x100).unwrap().session_id;
        let port1_session = orch.get_port_info(0x104).unwrap().session_id;

        assert_eq!(port0_session, port1_session);
        assert_eq!(orch.session_count(), 1);
    }

    // 3. Global Enable/Disable Tests

    #[test]
    fn test_global_enable_disable() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        assert!(!orch.is_enabled());

        orch.set_enabled(true);
        assert!(orch.is_enabled());

        orch.set_enabled(false);
        assert!(!orch.is_enabled());
    }

    #[test]
    fn test_global_disable_prevents_port_sampling() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(false);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        let result = orch.configure_port("Ethernet0", config);
        assert!(result.is_ok());
        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.session_count(), 0);
    }

    #[test]
    fn test_reenabling_global_allows_port_config() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.set_enabled(false);
        orch.configure_port("Ethernet0", config.clone()).unwrap();
        assert_eq!(orch.port_count(), 0);

        orch.set_enabled(true);
        orch.configure_port("Ethernet0", config).unwrap();
        assert_eq!(orch.port_count(), 1);
    }

    // 4. Reference Counting Tests

    #[test]
    fn test_session_refcount_increment_decrement() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        let session = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session.ref_count, 1);

        orch.configure_port("Ethernet4", config).unwrap();
        let session = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session.ref_count, 2);

        orch.remove_port("Ethernet0").unwrap();
        let session = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session.ref_count, 1);

        orch.remove_port("Ethernet4").unwrap();
        assert_eq!(orch.session_count(), 0);
    }

    #[test]
    fn test_session_persists_with_active_ports() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config).unwrap();

        orch.remove_port("Ethernet0").unwrap();

        // Session should still exist
        assert_eq!(orch.session_count(), 1);
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 0);
    }

    #[test]
    fn test_session_cleanup_when_last_port_removed() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config).unwrap();

        orch.remove_port("Ethernet0").unwrap();
        orch.remove_port("Ethernet4").unwrap();

        assert_eq!(orch.session_count(), 0);
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_multiple_ports_increment_same_session_refcount() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config).unwrap();

        let session = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session.ref_count, 2);
    }

    // 5. Error Handling Tests

    #[test]
    fn test_invalid_port_reference() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        let result = orch.configure_port("InvalidPort", config);
        assert!(matches!(result, Err(SflowOrchError::PortNotFound(_))));
    }

    #[test]
    fn test_remove_nonexistent_port() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let result = orch.remove_port("Ethernet0");
        assert!(matches!(result, Err(SflowOrchError::PortNotFound(_))));
    }

    #[test]
    fn test_configure_port_without_rate() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let config = SflowConfig::new(); // No rate set

        let result = orch.configure_port("Ethernet0", config);
        assert!(matches!(result, Err(SflowOrchError::InvalidConfig(_))));
    }

    #[test]
    fn test_configure_port_without_callbacks() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        let result = orch.configure_port("Ethernet0", config);
        assert!(matches!(result, Err(SflowOrchError::InvalidConfig(_))));
    }

    // 6. Edge Cases Tests

    #[test]
    fn test_changing_rate_multiple_times() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        orch.configure_port("Ethernet0", config.clone()).unwrap();

        config.rate = NonZeroU32::new(8192);
        orch.configure_port("Ethernet0", config.clone()).unwrap();

        config.rate = NonZeroU32::new(16384);
        orch.configure_port("Ethernet0", config).unwrap();

        assert_eq!(orch.stats().rate_updates, 2);
        assert_eq!(orch.session_count(), 1);

        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 2); // 4096 and 8192 removed
    }

    #[test]
    fn test_session_reuse_optimization() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        let session_id_1 = orch.get_port_info(0x100).unwrap().session_id;

        orch.configure_port("Ethernet4", config).unwrap();
        let session_id_2 = orch.get_port_info(0x104).unwrap().session_id;

        assert_eq!(session_id_1, session_id_2);
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 1); // Only one session created
    }

    #[test]
    fn test_empty_session_cleanup_after_rate_change() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        assert_eq!(orch.session_count(), 1);

        config.rate = NonZeroU32::new(8192);
        orch.configure_port("Ethernet0", config).unwrap();

        assert_eq!(orch.session_count(), 1);
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 1); // Old session removed
    }

    #[test]
    fn test_direction_update_from_both_to_rx() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Both;

        orch.configure_port("Ethernet0", config.clone()).unwrap();

        config.direction = SampleDirection::Rx;
        orch.configure_port("Ethernet0", config).unwrap();

        let ops = callbacks.port_ops.lock().unwrap();
        assert!(ops.iter().any(|s| s.starts_with("disable_ingress:")));
        assert!(ops.iter().any(|s| s.starts_with("disable_egress:")));
    }

    #[test]
    fn test_port_info_retrieval() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.direction = SampleDirection::Both;
        config.admin_state = true;

        orch.configure_port("Ethernet0", config).unwrap();

        let info = orch.get_port_info(0x100).unwrap();
        assert_eq!(info.admin_state, true);
        assert_eq!(info.direction, SampleDirection::Both);
    }

    #[test]
    fn test_session_rate_lookup() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config).unwrap();

        let session_id = orch.get_port_info(0x100).unwrap().session_id;
        let rate = orch.get_session_rate(session_id).unwrap();
        assert_eq!(rate, NonZeroU32::new(4096).unwrap());
    }

    #[test]
    fn test_admin_state_update() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);
        config.admin_state = false;

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        let info = orch.get_port_info(0x100).unwrap();
        assert_eq!(info.admin_state, false);

        config.admin_state = true;
        orch.configure_port("Ethernet0", config).unwrap();
        let info = orch.get_port_info(0x100).unwrap();
        assert_eq!(info.admin_state, true);
    }

    #[test]
    fn test_multiple_rate_changes_with_shared_session() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.set_enabled(true);

        let mut config = SflowConfig::new();
        config.rate = NonZeroU32::new(4096);

        orch.configure_port("Ethernet0", config.clone()).unwrap();
        orch.configure_port("Ethernet4", config.clone()).unwrap();

        // Change rate on one port
        config.rate = NonZeroU32::new(8192);
        orch.configure_port("Ethernet0", config).unwrap();

        // Two sessions should exist
        assert_eq!(orch.session_count(), 2);

        // First session should have ref_count of 1
        let session_4096 = orch.sessions.get(&NonZeroU32::new(4096).unwrap()).unwrap();
        assert_eq!(session_4096.ref_count, 1);

        // Second session should have ref_count of 1
        let session_8192 = orch.sessions.get(&NonZeroU32::new(8192).unwrap()).unwrap();
        assert_eq!(session_8192.ref_count, 1);
    }

    #[test]
    fn test_initialized_state() {
        let mut orch = SflowOrch::new(SflowOrchConfig::default());
        assert!(!orch.is_initialized());

        orch.set_initialized();
        assert!(orch.is_initialized());
    }

    #[test]
    fn test_get_session_rate_for_nonexistent_session() {
        let orch = SflowOrch::new(SflowOrchConfig::default());
        let rate = orch.get_session_rate(0x9999);
        assert!(rate.is_none());
    }
}
