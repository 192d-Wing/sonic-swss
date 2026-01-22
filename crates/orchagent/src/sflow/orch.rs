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
}
