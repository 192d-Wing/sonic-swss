//! BfdOrch implementation.

use std::collections::HashMap;
use std::sync::Arc;

use sonic_sai::types::RawSaiObjectId;

use super::types::{
    BfdSessionConfig, BfdSessionInfo, BfdSessionKey, BfdSessionState, BfdSessionType, BfdUpdate,
    BFD_SRCPORT_INIT, BFD_SRCPORT_MAX, NUM_BFD_SRCPORT_RETRIES,
};

/// BFD orchestrator error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BfdOrchError {
    /// Session not found.
    SessionNotFound(String),
    /// Session already exists.
    SessionExists(String),
    /// Invalid configuration.
    InvalidConfig(String),
    /// SAI error.
    SaiError(String),
    /// VRF not found.
    VrfNotFound(String),
    /// Port not found.
    PortNotFound(String),
    /// Source port exhausted.
    SourcePortExhausted,
}

impl std::fmt::Display for BfdOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionNotFound(key) => write!(f, "BFD session not found: {}", key),
            Self::SessionExists(key) => write!(f, "BFD session already exists: {}", key),
            Self::InvalidConfig(msg) => write!(f, "Invalid BFD config: {}", msg),
            Self::SaiError(msg) => write!(f, "SAI error: {}", msg),
            Self::VrfNotFound(name) => write!(f, "VRF not found: {}", name),
            Self::PortNotFound(name) => write!(f, "Port not found: {}", name),
            Self::SourcePortExhausted => write!(f, "BFD source ports exhausted"),
        }
    }
}

impl std::error::Error for BfdOrchError {}

/// Callbacks for BfdOrch operations.
pub trait BfdOrchCallbacks: Send + Sync {
    /// Creates a BFD session via SAI.
    fn create_bfd_session(&self, config: &BfdSessionConfig, discriminator: u32, src_port: u16) -> Result<RawSaiObjectId, String>;

    /// Removes a BFD session via SAI.
    fn remove_bfd_session(&self, sai_oid: RawSaiObjectId) -> Result<(), String>;

    /// Gets VRF SAI object ID by name.
    fn get_vrf_id(&self, vrf_name: &str) -> Option<RawSaiObjectId>;

    /// Gets port SAI object ID by name.
    fn get_port_id(&self, port_name: &str) -> Option<RawSaiObjectId>;

    /// Writes session state to state DB.
    fn write_state_db(&self, key: &str, state: BfdSessionState, session_type: BfdSessionType);

    /// Removes session from state DB.
    fn remove_state_db(&self, key: &str);

    /// Notifies observers about a BFD state change.
    fn notify(&self, update: BfdUpdate);

    /// Returns true if software BFD mode is enabled.
    fn is_software_bfd(&self) -> bool;

    /// Returns true if TSA (Traffic Shift Algorithm) is active.
    fn is_tsa_active(&self) -> bool;

    /// Creates a software BFD session (for DPU passive sessions).
    fn create_software_bfd_session(&self, key: &str, config: &BfdSessionConfig);

    /// Removes a software BFD session.
    fn remove_software_bfd_session(&self, key: &str);
}

/// BFD orchestrator configuration.
#[derive(Debug, Clone, Default)]
pub struct BfdOrchConfig {
    // Currently no configuration options, but reserved for future use
}

/// BFD orchestrator statistics.
#[derive(Debug, Clone, Default)]
pub struct BfdOrchStats {
    /// Number of sessions created.
    pub sessions_created: u64,
    /// Number of sessions removed.
    pub sessions_removed: u64,
    /// Number of state change notifications.
    pub state_changes: u64,
    /// Number of creation retries.
    pub creation_retries: u64,
    /// Number of TSA shutdowns.
    pub tsa_shutdowns: u64,
    /// Number of TSA restores.
    pub tsa_restores: u64,
}

/// BFD orchestrator for Bidirectional Forwarding Detection.
pub struct BfdOrch {
    /// Configuration.
    config: BfdOrchConfig,
    /// Map from config key to session info.
    sessions: HashMap<String, BfdSessionInfo>,
    /// Reverse map from SAI OID to config key.
    sai_to_key: HashMap<RawSaiObjectId, String>,
    /// Cached sessions for TSA (sessions removed during TSA).
    tsa_cache: HashMap<String, BfdSessionConfig>,
    /// Callbacks for SAI and DB operations.
    callbacks: Option<Arc<dyn BfdOrchCallbacks>>,
    /// Whether the orch is initialized.
    initialized: bool,
    /// Statistics.
    stats: BfdOrchStats,
    /// Next local discriminator.
    next_discriminator: u32,
    /// Next source port.
    next_src_port: u16,
    /// Whether notification handler is registered.
    notification_registered: bool,
}

impl std::fmt::Debug for BfdOrch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BfdOrch")
            .field("config", &self.config)
            .field("sessions_count", &self.sessions.len())
            .field("tsa_cache_count", &self.tsa_cache.len())
            .field("initialized", &self.initialized)
            .field("stats", &self.stats)
            .finish()
    }
}

impl BfdOrch {
    /// Creates a new BfdOrch with the given configuration.
    pub fn new(config: BfdOrchConfig) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
            sai_to_key: HashMap::new(),
            tsa_cache: HashMap::new(),
            callbacks: None,
            initialized: false,
            stats: BfdOrchStats::default(),
            next_discriminator: 1,
            next_src_port: BFD_SRCPORT_INIT,
            notification_registered: false,
        }
    }

    /// Sets the callbacks for this orch.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn BfdOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the configuration.
    pub fn config(&self) -> &BfdOrchConfig {
        &self.config
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &BfdOrchStats {
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

    /// Returns the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns an iterator over all sessions.
    pub fn sessions(&self) -> impl Iterator<Item = (&String, &BfdSessionInfo)> {
        self.sessions.iter()
    }

    /// Gets a session by config key.
    pub fn get_session(&self, key: &str) -> Option<&BfdSessionInfo> {
        self.sessions.get(key)
    }

    /// Gets a session by SAI OID.
    pub fn get_session_by_oid(&self, oid: RawSaiObjectId) -> Option<&BfdSessionInfo> {
        self.sai_to_key
            .get(&oid)
            .and_then(|key| self.sessions.get(key))
    }

    /// Generates the next local discriminator.
    fn gen_discriminator(&mut self) -> u32 {
        let disc = self.next_discriminator;
        self.next_discriminator = self.next_discriminator.wrapping_add(1);
        if self.next_discriminator == 0 {
            self.next_discriminator = 1; // Skip 0
        }
        disc
    }

    /// Generates the next source port.
    fn gen_src_port(&mut self) -> u16 {
        let port = self.next_src_port;
        self.next_src_port += 1;
        if self.next_src_port > BFD_SRCPORT_MAX {
            self.next_src_port = BFD_SRCPORT_INIT;
        }
        port
    }

    /// Creates a BFD session.
    pub fn create_session(&mut self, config: BfdSessionConfig) -> Result<(), BfdOrchError> {
        let key = config.key.to_config_key();

        // Check if session already exists
        if self.sessions.contains_key(&key) {
            return Err(BfdOrchError::SessionExists(key));
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| BfdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        // Check software BFD mode
        if callbacks.is_software_bfd() {
            let state_db_key = config.key.to_state_db_key();
            callbacks.create_software_bfd_session(&state_db_key, &config);
            return Ok(());
        }

        // Handle TSA - cache and skip if shutdown_bfd_during_tsa is set
        if callbacks.is_tsa_active() && config.shutdown_bfd_during_tsa {
            self.tsa_cache.insert(key, config);
            return Ok(());
        }

        // Create hardware BFD session
        self.create_hardware_session(config)
    }

    /// Creates a hardware BFD session via SAI.
    fn create_hardware_session(&mut self, config: BfdSessionConfig) -> Result<(), BfdOrchError> {
        let key = config.key.to_config_key();
        let state_db_key = config.key.to_state_db_key();

        // Clone the Arc to avoid borrow conflicts when calling mutable methods
        let callbacks = Arc::clone(
            self.callbacks
                .as_ref()
                .ok_or_else(|| BfdOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let discriminator = self.gen_discriminator();

        // Try to create session, retrying with different source ports if needed
        let mut last_error = String::new();
        for attempt in 0..NUM_BFD_SRCPORT_RETRIES {
            let src_port = self.gen_src_port();

            match callbacks.create_bfd_session(&config, discriminator, src_port) {
                Ok(sai_oid) => {
                    // Success - store session info
                    let info = BfdSessionInfo::new(
                        sai_oid,
                        state_db_key.clone(),
                        config.clone(),
                        discriminator,
                        src_port,
                    );

                    self.sessions.insert(key.clone(), info);
                    self.sai_to_key.insert(sai_oid, key);
                    self.stats.sessions_created += 1;

                    if attempt > 0 {
                        self.stats.creation_retries += attempt as u64;
                    }

                    // Write initial state to state DB
                    callbacks.write_state_db(
                        &state_db_key,
                        BfdSessionState::Down,
                        config.session_type,
                    );

                    return Ok(());
                }
                Err(e) => {
                    last_error = e;
                    // Continue to retry with different port
                }
            }
        }

        Err(BfdOrchError::SaiError(format!(
            "Failed after {} retries: {}",
            NUM_BFD_SRCPORT_RETRIES, last_error
        )))
    }

    /// Removes a BFD session.
    pub fn remove_session(&mut self, key: &str) -> Result<(), BfdOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| BfdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        // Check TSA cache first
        if self.tsa_cache.remove(key).is_some() {
            return Ok(());
        }

        // Check software BFD mode
        if callbacks.is_software_bfd() {
            if let Some(session_key) = BfdSessionKey::parse(key) {
                callbacks.remove_software_bfd_session(&session_key.to_state_db_key());
            }
            return Ok(());
        }

        // Get session info
        let info = self
            .sessions
            .get(key)
            .ok_or_else(|| BfdOrchError::SessionNotFound(key.to_string()))?;

        let sai_oid = info.sai_oid;
        let state_db_key = info.state_db_key.clone();

        // Remove via SAI
        callbacks
            .remove_bfd_session(sai_oid)
            .map_err(BfdOrchError::SaiError)?;

        // Clean up internal state
        self.sessions.remove(key);
        self.sai_to_key.remove(&sai_oid);
        self.stats.sessions_removed += 1;

        // Remove from state DB
        callbacks.remove_state_db(&state_db_key);

        Ok(())
    }

    /// Handles a BFD session state change notification from SAI.
    pub fn handle_state_change(
        &mut self,
        sai_oid: RawSaiObjectId,
        new_state: BfdSessionState,
    ) -> Result<(), BfdOrchError> {
        let key = self
            .sai_to_key
            .get(&sai_oid)
            .ok_or_else(|| BfdOrchError::SessionNotFound(format!("OID 0x{:x}", sai_oid)))?
            .clone();

        let info = self
            .sessions
            .get_mut(&key)
            .ok_or_else(|| BfdOrchError::SessionNotFound(key.clone()))?;

        let old_state = info.state;

        // Only process if state actually changed
        if old_state == new_state {
            return Ok(());
        }

        info.set_state(new_state);
        self.stats.state_changes += 1;

        if let Some(callbacks) = &self.callbacks {
            // Update state DB
            callbacks.write_state_db(
                &info.state_db_key,
                new_state,
                info.config.session_type,
            );

            // Notify observers
            callbacks.notify(BfdUpdate::new(&info.state_db_key, new_state));
        }

        Ok(())
    }

    /// Handles TSA state change.
    pub fn handle_tsa_state_change(&mut self, tsa_enabled: bool) -> Result<(), BfdOrchError> {
        if tsa_enabled {
            // TSA enabled - shutdown sessions with shutdown_bfd_during_tsa=true
            let sessions_to_shutdown: Vec<_> = self
                .sessions
                .iter()
                .filter(|(_, info)| info.config.shutdown_bfd_during_tsa)
                .map(|(k, info)| (k.clone(), info.config.clone()))
                .collect();

            for (key, config) in sessions_to_shutdown {
                // Remove the session first (ignore errors)
                let _ = self.remove_session(&key);
                // Then cache the config for later restoration
                self.tsa_cache.insert(key, config);
                self.stats.tsa_shutdowns += 1;
            }
        } else {
            // TSA disabled - restore cached sessions
            let cached: Vec<_> = self.tsa_cache.drain().collect();
            for (_, config) in cached {
                // Recreate the session (ignore errors)
                let _ = self.create_session(config);
                self.stats.tsa_restores += 1;
            }
        }

        Ok(())
    }

    /// Creates all software BFD sessions (for transitioning to software mode).
    pub fn create_all_software_sessions(&self) {
        if let Some(callbacks) = &self.callbacks {
            for (_, info) in &self.sessions {
                callbacks.create_software_bfd_session(&info.state_db_key, &info.config);
            }
        }
    }

    /// Removes all software BFD sessions.
    pub fn remove_all_software_sessions(&self) {
        if let Some(callbacks) = &self.callbacks {
            for (_, info) in &self.sessions {
                callbacks.remove_software_bfd_session(&info.state_db_key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Mutex;

    struct TestCallbacks {
        created_sessions: Mutex<Vec<(String, u32, u16)>>,
        removed_sessions: Mutex<Vec<RawSaiObjectId>>,
        state_updates: Mutex<Vec<(String, BfdSessionState)>>,
        notifications: Mutex<Vec<BfdUpdate>>,
        software_bfd: bool,
        tsa_active: bool,
        fail_create: bool,
    }

    impl TestCallbacks {
        fn new() -> Self {
            Self {
                created_sessions: Mutex::new(Vec::new()),
                removed_sessions: Mutex::new(Vec::new()),
                state_updates: Mutex::new(Vec::new()),
                notifications: Mutex::new(Vec::new()),
                software_bfd: false,
                tsa_active: false,
                fail_create: false,
            }
        }

        fn with_software_bfd() -> Self {
            Self {
                software_bfd: true,
                ..Self::new()
            }
        }

        fn with_tsa_active() -> Self {
            Self {
                tsa_active: true,
                ..Self::new()
            }
        }
    }

    impl BfdOrchCallbacks for TestCallbacks {
        fn create_bfd_session(
            &self,
            config: &BfdSessionConfig,
            discriminator: u32,
            src_port: u16,
        ) -> Result<RawSaiObjectId, String> {
            if self.fail_create {
                return Err("Creation failed".to_string());
            }
            let oid = (discriminator as u64) << 16 | (src_port as u64);
            self.created_sessions
                .lock()
                .unwrap()
                .push((config.key.to_config_key(), discriminator, src_port));
            Ok(oid)
        }

        fn remove_bfd_session(&self, sai_oid: RawSaiObjectId) -> Result<(), String> {
            self.removed_sessions.lock().unwrap().push(sai_oid);
            Ok(())
        }

        fn get_vrf_id(&self, _vrf_name: &str) -> Option<RawSaiObjectId> {
            Some(0x1000)
        }

        fn get_port_id(&self, _port_name: &str) -> Option<RawSaiObjectId> {
            Some(0x2000)
        }

        fn write_state_db(&self, key: &str, state: BfdSessionState, _session_type: BfdSessionType) {
            self.state_updates
                .lock()
                .unwrap()
                .push((key.to_string(), state));
        }

        fn remove_state_db(&self, _key: &str) {}

        fn notify(&self, update: BfdUpdate) {
            self.notifications.lock().unwrap().push(update);
        }

        fn is_software_bfd(&self) -> bool {
            self.software_bfd
        }

        fn is_tsa_active(&self) -> bool {
            self.tsa_active
        }

        fn create_software_bfd_session(&self, _key: &str, _config: &BfdSessionConfig) {}

        fn remove_software_bfd_session(&self, _key: &str) {}
    }

    #[test]
    fn test_bfd_orch_new() {
        let orch = BfdOrch::new(BfdOrchConfig::default());
        assert!(!orch.is_initialized());
        assert_eq!(orch.session_count(), 0);
    }

    #[test]
    fn test_create_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        let result = orch.create_session(config);
        assert!(result.is_ok());
        assert_eq!(orch.session_count(), 1);
        assert!(orch.get_session("default::10.0.0.1").is_some());

        // Check callbacks
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].0, "default::10.0.0.1");
    }

    #[test]
    fn test_create_duplicate_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key.clone());

        orch.create_session(config.clone()).unwrap();

        // Try to create duplicate
        let result = orch.create_session(config);
        assert!(matches!(result, Err(BfdOrchError::SessionExists(_))));
    }

    #[test]
    fn test_remove_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        orch.create_session(config).unwrap();
        assert_eq!(orch.session_count(), 1);

        let result = orch.remove_session("default::10.0.0.1");
        assert!(result.is_ok());
        assert_eq!(orch.session_count(), 0);

        // Check SAI removal
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_remove_nonexistent_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let result = orch.remove_session("default::10.0.0.1");
        assert!(matches!(result, Err(BfdOrchError::SessionNotFound(_))));
    }

    #[test]
    fn test_state_change() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        orch.create_session(config).unwrap();

        // Get SAI OID
        let session = orch.get_session("default::10.0.0.1").unwrap();
        let sai_oid = session.sai_oid;

        // Simulate state change
        orch.handle_state_change(sai_oid, BfdSessionState::Up)
            .unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.state, BfdSessionState::Up);

        // Check notification
        let notifications = callbacks.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].state, BfdSessionState::Up);
    }

    #[test]
    fn test_tsa_shutdown() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        // Create session with shutdown_bfd_during_tsa=true
        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key).with_shutdown_bfd_during_tsa(true);

        orch.create_session(config).unwrap();
        assert_eq!(orch.session_count(), 1);

        // Enable TSA
        orch.handle_tsa_state_change(true).unwrap();
        assert_eq!(orch.session_count(), 0);
        assert_eq!(orch.tsa_cache.len(), 1);

        // Disable TSA - should restore
        orch.handle_tsa_state_change(false).unwrap();
        assert_eq!(orch.session_count(), 1);
        assert_eq!(orch.tsa_cache.len(), 0);
    }

    #[test]
    fn test_software_bfd_mode() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::with_software_bfd());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        // Should succeed without creating SAI session
        let result = orch.create_session(config);
        assert!(result.is_ok());

        // No SAI sessions created
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 0);
    }

    #[test]
    fn test_statistics() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        let sai_oid = session.sai_oid;

        orch.handle_state_change(sai_oid, BfdSessionState::Up)
            .unwrap();
        orch.remove_session("default::10.0.0.1").unwrap();

        let stats = orch.stats();
        assert_eq!(stats.sessions_created, 1);
        assert_eq!(stats.sessions_removed, 1);
        assert_eq!(stats.state_changes, 1);
    }
}
