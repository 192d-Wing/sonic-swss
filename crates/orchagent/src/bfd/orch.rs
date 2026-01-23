//! BfdOrch implementation.

use std::collections::HashMap;
use std::sync::Arc;

use sonic_sai::types::RawSaiObjectId;
use thiserror::Error;

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;

use super::types::{
    BfdSessionConfig, BfdSessionInfo, BfdSessionKey, BfdSessionState, BfdSessionType, BfdUpdate,
    BFD_SRCPORT_INIT, BFD_SRCPORT_MAX, NUM_BFD_SRCPORT_RETRIES,
};

/// BFD orchestrator error type.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BfdOrchError {
    /// Session not found.
    #[error("BFD session not found: {0}")]
    SessionNotFound(String),
    /// Session already exists.
    #[error("BFD session already exists: {0}")]
    SessionExists(String),
    /// Invalid configuration.
    #[error("Invalid BFD config: {0}")]
    InvalidConfig(String),
    /// SAI error.
    #[error("SAI error: {0}")]
    SaiError(String),
    /// VRF not found.
    #[error("VRF not found: {0}")]
    VrfNotFound(String),
    /// Port not found.
    #[error("Port not found: {0}")]
    PortNotFound(String),
    /// Source port exhausted.
    #[error("BFD source ports exhausted")]
    SourcePortExhausted,
}

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
            let err = BfdOrchError::SessionExists(key.clone());
            let audit_record = AuditRecord::new(
                AuditCategory::ResourceCreate,
                "BfdOrch",
                "create_session",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&key)
            .with_object_type("bfd_session")
            .with_error("Session already exists");
            audit_log!(audit_record);
            return Err(err);
        }

        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| BfdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        // Check software BFD mode
        if callbacks.is_software_bfd() {
            let state_db_key = config.key.to_state_db_key();
            callbacks.create_software_bfd_session(&state_db_key, &config);

            let audit_record = AuditRecord::new(
                AuditCategory::ResourceCreate,
                "BfdOrch",
                "create_session",
            )
            .with_outcome(AuditOutcome::Success)
            .with_object_id(&key)
            .with_object_type("bfd_session_software")
            .with_details(serde_json::json!({
                "session_key": key,
                "session_type": config.session_type.config_string(),
                "tx_interval": config.tx_interval,
                "rx_interval": config.rx_interval,
                "multiplier": config.multiplier,
                "mode": "software",
            }));
            audit_log!(audit_record);
            return Ok(());
        }

        // Handle TSA - cache and skip if shutdown_bfd_during_tsa is set
        if callbacks.is_tsa_active() && config.shutdown_bfd_during_tsa {
            self.tsa_cache.insert(key.clone(), config.clone());

            let audit_record = AuditRecord::new(
                AuditCategory::ResourceCreate,
                "BfdOrch",
                "create_session_cached_during_tsa",
            )
            .with_outcome(AuditOutcome::Success)
            .with_object_id(&key)
            .with_object_type("bfd_session_cached")
            .with_details(serde_json::json!({
                "session_key": key,
                "session_type": config.session_type.config_string(),
                "tsa_active": true,
                "shutdown_during_tsa": config.shutdown_bfd_during_tsa,
            }));
            audit_log!(audit_record);
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
                    self.sai_to_key.insert(sai_oid, key.clone());
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

                    // Log successful session creation with NIST AU-3 audit details
                    let audit_record = AuditRecord::new(
                        AuditCategory::ResourceCreate,
                        "BfdOrch",
                        "create_hardware_session",
                    )
                    .with_outcome(AuditOutcome::Success)
                    .with_object_id(&key)
                    .with_object_type("bfd_session_hardware")
                    .with_details(serde_json::json!({
                        "session_key": key,
                        "local_discriminator": discriminator,
                        "source_port": src_port,
                        "sai_oid": format!("0x{:x}", sai_oid),
                        "session_type": config.session_type.config_string(),
                        "tx_interval": config.tx_interval,
                        "rx_interval": config.rx_interval,
                        "multiplier": config.multiplier,
                        "tos": config.tos,
                        "initial_state": "Down",
                        "remote_peer": config.key.peer_ip.to_string(),
                        "interface": config.key.interface.as_deref(),
                        "vrf": config.key.vrf.as_str(),
                        "retries_attempted": attempt,
                        "mode": "hardware",
                    }));
                    audit_log!(audit_record);

                    return Ok(());
                }
                Err(e) => {
                    last_error = e;
                    // Continue to retry with different port
                }
            }
        }

        let error = BfdOrchError::SaiError(format!(
            "Failed after {} retries: {}",
            NUM_BFD_SRCPORT_RETRIES, last_error
        ));

        // Log failed session creation
        let audit_record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "BfdOrch",
            "create_hardware_session",
        )
        .with_outcome(AuditOutcome::Failure)
        .with_object_id(&key)
        .with_object_type("bfd_session_hardware")
        .with_error(&format!(
            "Failed after {} retries: {}",
            NUM_BFD_SRCPORT_RETRIES, last_error
        ))
        .with_details(serde_json::json!({
            "session_key": key,
            "session_type": config.session_type.config_string(),
            "remote_peer": config.key.peer_ip.to_string(),
            "retries_attempted": NUM_BFD_SRCPORT_RETRIES,
        }));
        audit_log!(audit_record);

        Err(error)
    }

    /// Removes a BFD session.
    pub fn remove_session(&mut self, key: &str) -> Result<(), BfdOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| BfdOrchError::InvalidConfig("No callbacks set".to_string()))?;

        // Check TSA cache first
        if self.tsa_cache.remove(key).is_some() {
            let audit_record = AuditRecord::new(
                AuditCategory::ResourceDelete,
                "BfdOrch",
                "remove_session_from_tsa_cache",
            )
            .with_outcome(AuditOutcome::Success)
            .with_object_id(key)
            .with_object_type("bfd_session_cached")
            .with_details(serde_json::json!({
                "session_key": key,
                "removal_source": "tsa_cache",
            }));
            audit_log!(audit_record);
            return Ok(());
        }

        // Check software BFD mode
        if callbacks.is_software_bfd() {
            if let Some(session_key) = BfdSessionKey::parse(key) {
                callbacks.remove_software_bfd_session(&session_key.to_state_db_key());

                let audit_record = AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "BfdOrch",
                    "remove_session_software",
                )
                .with_outcome(AuditOutcome::Success)
                .with_object_id(key)
                .with_object_type("bfd_session_software")
                .with_details(serde_json::json!({
                    "session_key": key,
                    "mode": "software",
                }));
                audit_log!(audit_record);
            }
            return Ok(());
        }

        // Get session info and extract all needed data before releasing the borrow
        let (sai_oid, state_db_key, config_copy, local_disc, src_port) = {
            let info = self
                .sessions
                .get(key)
                .ok_or_else(|| {
                    let err = BfdOrchError::SessionNotFound(key.to_string());
                    let audit_record = AuditRecord::new(
                        AuditCategory::ResourceDelete,
                        "BfdOrch",
                        "remove_session",
                    )
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(key)
                    .with_object_type("bfd_session")
                    .with_error("Session not found");
                    audit_log!(audit_record);
                    err
                })?;

            (
                info.sai_oid,
                info.state_db_key.clone(),
                info.config.clone(),
                info.local_discriminator,
                info.src_port,
            )
        };

        // Remove via SAI
        callbacks
            .remove_bfd_session(sai_oid)
            .map_err(|e| {
                let err = BfdOrchError::SaiError(e.clone());
                let audit_record = AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "BfdOrch",
                    "remove_session",
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(key)
                .with_object_type("bfd_session_hardware")
                .with_error(&e);
                audit_log!(audit_record);
                err
            })?;

        // Clean up internal state
        self.sessions.remove(key);
        self.sai_to_key.remove(&sai_oid);
        self.stats.sessions_removed += 1;

        // Remove from state DB
        callbacks.remove_state_db(&state_db_key);

        // Log successful session deletion with NIST AU-3 audit details
        let audit_record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "BfdOrch",
            "remove_session",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(key)
        .with_object_type("bfd_session_hardware")
        .with_details(serde_json::json!({
            "session_key": key,
            "sai_oid": format!("0x{:x}", sai_oid),
            "local_discriminator": local_disc,
            "source_port": src_port,
            "session_type": config_copy.session_type.config_string(),
            "remote_peer": config_copy.key.peer_ip.to_string(),
            "interface": config_copy.key.interface.as_deref(),
            "vrf": config_copy.key.vrf.as_str(),
            "mode": "hardware",
        }));
        audit_log!(audit_record);

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

            let shutdown_count = sessions_to_shutdown.len();
            let mut session_keys = Vec::new();

            for (key, config) in sessions_to_shutdown {
                session_keys.push(key.clone());
                // Remove the session first (ignore errors)
                let _ = self.remove_session(&key);
                // Then cache the config for later restoration
                self.tsa_cache.insert(key, config);
                self.stats.tsa_shutdowns += 1;
            }

            // Log TSA enabled event with SystemLifecycle category per NIST AU-2
            let audit_record = AuditRecord::new(
                AuditCategory::SystemLifecycle,
                "BfdOrch",
                "handle_tsa_enabled",
            )
            .with_outcome(AuditOutcome::Success)
            .with_object_id("TSA")
            .with_object_type("traffic_shift_active")
            .with_details(serde_json::json!({
                "event": "tsa_enabled",
                "sessions_shutdown": shutdown_count,
                "session_keys": session_keys,
                "action": "BFD sessions with shutdown_bfd_during_tsa=true have been shutdown and cached",
            }));
            audit_log!(audit_record);
        } else {
            // TSA disabled - restore cached sessions
            let cached: Vec<_> = self.tsa_cache.drain().collect();
            let restore_count = cached.len();
            let mut session_keys = Vec::new();

            for (key, config) in cached {
                session_keys.push(key.clone());
                // Recreate the session (ignore errors)
                let _ = self.create_session(config);
                self.stats.tsa_restores += 1;
            }

            // Log TSA disabled event with SystemLifecycle category per NIST AU-2
            let audit_record = AuditRecord::new(
                AuditCategory::SystemLifecycle,
                "BfdOrch",
                "handle_tsa_disabled",
            )
            .with_outcome(AuditOutcome::Success)
            .with_object_id("TSA")
            .with_object_type("traffic_shift_active")
            .with_details(serde_json::json!({
                "event": "tsa_disabled",
                "sessions_restored": restore_count,
                "session_keys": session_keys,
                "action": "Cached BFD sessions have been restored following TSA disable",
            }));
            audit_log!(audit_record);
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

    // ========================================================================
    // Additional Comprehensive Tests
    // ========================================================================

    use std::net::Ipv6Addr;

    // ------------------------------------------------------------------------
    // BFD Session Management Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_create_ipv6_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        let result = orch.create_session(config);
        assert!(result.is_ok());
        assert_eq!(orch.session_count(), 1);
        assert!(orch.get_session("default::2001:db8::1").is_some());

        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].0, "default::2001:db8::1");
    }

    #[test]
    fn test_create_single_hop_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            Some("Ethernet0".to_string()),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        );
        let config = BfdSessionConfig::new(key);

        let result = orch.create_session(config);
        assert!(result.is_ok());
        assert_eq!(orch.session_count(), 1);
        assert!(orch.get_session("default:Ethernet0:192.168.1.1").is_some());

        let session = orch.get_session("default:Ethernet0:192.168.1.1").unwrap();
        assert!(!session.config.key.is_multihop());
    }

    #[test]
    fn test_create_multihop_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "Vrf-RED",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 10, 10, 1)),
        );
        let config = BfdSessionConfig::new(key);

        let result = orch.create_session(config);
        assert!(result.is_ok());
        assert_eq!(orch.session_count(), 1);

        let session = orch.get_session("Vrf-RED::10.10.10.1").unwrap();
        assert!(session.config.key.is_multihop());
    }

    #[test]
    fn test_create_session_with_custom_parameters() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key)
            .with_tx_interval(500)
            .with_rx_interval(600)
            .with_multiplier(5)
            .with_tos(128);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.config.tx_interval, 500);
        assert_eq!(session.config.rx_interval, 600);
        assert_eq!(session.config.multiplier, 5);
        assert_eq!(session.config.tos, 128);
    }

    #[test]
    fn test_multiple_sessions_different_peers() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        // Create 3 different sessions
        for i in 1..=3 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let config = BfdSessionConfig::new(key);
            orch.create_session(config).unwrap();
        }

        assert_eq!(orch.session_count(), 3);
        assert!(orch.get_session("default::10.0.0.1").is_some());
        assert!(orch.get_session("default::10.0.0.2").is_some());
        assert!(orch.get_session("default::10.0.0.3").is_some());
    }

    #[test]
    fn test_get_session_by_oid() {
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

        let session_by_oid = orch.get_session_by_oid(sai_oid);
        assert!(session_by_oid.is_some());
        assert_eq!(session_by_oid.unwrap().sai_oid, sai_oid);
    }

    #[test]
    fn test_discriminator_generation() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        // Create multiple sessions and verify unique discriminators
        let mut discriminators = std::collections::HashSet::new();

        for i in 1..=10 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let config = BfdSessionConfig::new(key);
            orch.create_session(config).unwrap();

            let session_key = format!("default::10.0.0.{}", i);
            let session = orch.get_session(&session_key).unwrap();
            discriminators.insert(session.local_discriminator);
        }

        // All discriminators should be unique
        assert_eq!(discriminators.len(), 10);
    }

    #[test]
    fn test_sessions_iterator() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        // Create 3 sessions
        for i in 1..=3 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let config = BfdSessionConfig::new(key);
            orch.create_session(config).unwrap();
        }

        let count = orch.sessions().count();
        assert_eq!(count, 3);

        // Verify all sessions are accessible via iterator
        let session_keys: Vec<_> = orch.sessions().map(|(k, _)| k.clone()).collect();
        assert!(session_keys.contains(&"default::10.0.0.1".to_string()));
        assert!(session_keys.contains(&"default::10.0.0.2".to_string()));
        assert!(session_keys.contains(&"default::10.0.0.3".to_string()));
    }

    // ------------------------------------------------------------------------
    // BFD Parameters Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_session_with_minimum_intervals() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key)
            .with_tx_interval(50)
            .with_rx_interval(50);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.config.tx_interval, 50);
        assert_eq!(session.config.rx_interval, 50);
    }

    #[test]
    fn test_session_with_large_intervals() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key)
            .with_tx_interval(10000)
            .with_rx_interval(15000);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.config.tx_interval, 10000);
        assert_eq!(session.config.rx_interval, 15000);
    }

    #[test]
    fn test_session_with_various_multipliers() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let test_cases = vec![3, 5, 10, 20, 255];

        for (idx, multiplier) in test_cases.into_iter().enumerate() {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, (idx + 1) as u8)),
            );
            let config = BfdSessionConfig::new(key).with_multiplier(multiplier);

            orch.create_session(config).unwrap();

            let session_key = format!("default::10.0.0.{}", idx + 1);
            let session = orch.get_session(&session_key).unwrap();
            assert_eq!(session.config.multiplier, multiplier);
        }
    }

    #[test]
    fn test_session_type_async_active() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key)
            .with_session_type(BfdSessionType::AsyncActive);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.config.session_type, BfdSessionType::AsyncActive);
        assert!(session.config.session_type.is_active());
        assert!(session.config.session_type.is_async());
    }

    #[test]
    fn test_session_type_async_passive() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key)
            .with_session_type(BfdSessionType::AsyncPassive);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.config.session_type, BfdSessionType::AsyncPassive);
        assert!(!session.config.session_type.is_active());
        assert!(session.config.session_type.is_async());
    }

    #[test]
    fn test_session_type_demand_active() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key)
            .with_session_type(BfdSessionType::DemandActive);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.config.session_type, BfdSessionType::DemandActive);
        assert!(session.config.session_type.is_active());
        assert!(!session.config.session_type.is_async());
    }

    #[test]
    fn test_session_with_local_address() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let local_addr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let config = BfdSessionConfig::new(key).with_local_addr(local_addr);

        orch.create_session(config).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.config.local_addr, Some(local_addr));
    }

    // ------------------------------------------------------------------------
    // Session State Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_initial_session_state() {
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

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.state, BfdSessionState::Down);

        // Verify state DB was written with Down state
        let state_updates = callbacks.state_updates.lock().unwrap();
        assert_eq!(state_updates.len(), 1);
        assert_eq!(state_updates[0].1, BfdSessionState::Down);
    }

    #[test]
    fn test_state_transition_down_to_up() {
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

        let session = orch.get_session("default::10.0.0.1").unwrap();
        let sai_oid = session.sai_oid;

        // Transition: Down -> Init -> Up
        orch.handle_state_change(sai_oid, BfdSessionState::Init).unwrap();
        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.state, BfdSessionState::Init);

        orch.handle_state_change(sai_oid, BfdSessionState::Up).unwrap();
        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.state, BfdSessionState::Up);

        assert_eq!(orch.stats().state_changes, 2);
    }

    #[test]
    fn test_state_transition_up_to_down() {
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

        // Transition: Down -> Up -> Down
        orch.handle_state_change(sai_oid, BfdSessionState::Up).unwrap();
        orch.handle_state_change(sai_oid, BfdSessionState::Down).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.state, BfdSessionState::Down);
        assert_eq!(orch.stats().state_changes, 2);
    }

    #[test]
    fn test_state_admin_down() {
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

        orch.handle_state_change(sai_oid, BfdSessionState::AdminDown).unwrap();

        let session = orch.get_session("default::10.0.0.1").unwrap();
        assert_eq!(session.state, BfdSessionState::AdminDown);
    }

    #[test]
    fn test_no_state_change_on_same_state() {
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

        let session = orch.get_session("default::10.0.0.1").unwrap();
        let sai_oid = session.sai_oid;

        // Try to set the same state (Down)
        orch.handle_state_change(sai_oid, BfdSessionState::Down).unwrap();

        // State change counter should not increment
        assert_eq!(orch.stats().state_changes, 0);

        // Notifications should not be sent
        let notifications = callbacks.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 0);
    }

    #[test]
    fn test_state_change_with_notification() {
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

        let session = orch.get_session("default::10.0.0.1").unwrap();
        let sai_oid = session.sai_oid;

        orch.handle_state_change(sai_oid, BfdSessionState::Up).unwrap();

        // Verify notification was sent
        let notifications = callbacks.notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].state, BfdSessionState::Up);
    }

    #[test]
    fn test_state_change_nonexistent_oid() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let fake_oid = 0xDEADBEEF;
        let result = orch.handle_state_change(fake_oid, BfdSessionState::Up);

        assert!(matches!(result, Err(BfdOrchError::SessionNotFound(_))));
    }

    // ------------------------------------------------------------------------
    // TSA (Traffic Shift Algorithm) Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_tsa_cache_session_on_create() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::with_tsa_active());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key).with_shutdown_bfd_during_tsa(true);

        orch.create_session(config).unwrap();

        // Session should be in TSA cache, not active
        assert_eq!(orch.session_count(), 0);
        assert_eq!(orch.tsa_cache.len(), 1);

        // No SAI session should be created
        let created = callbacks.created_sessions.lock().unwrap();
        assert_eq!(created.len(), 0);
    }

    #[test]
    fn test_tsa_no_cache_without_shutdown_flag() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::with_tsa_active());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key).with_shutdown_bfd_during_tsa(false);

        orch.create_session(config).unwrap();

        // Session should be created normally despite TSA being active
        assert_eq!(orch.session_count(), 1);
        assert_eq!(orch.tsa_cache.len(), 0);
    }

    #[test]
    fn test_tsa_remove_cached_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::with_tsa_active());
        orch.set_callbacks(callbacks);

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key).with_shutdown_bfd_during_tsa(true);

        orch.create_session(config).unwrap();
        assert_eq!(orch.tsa_cache.len(), 1);

        // Remove should succeed and remove from cache
        let result = orch.remove_session("default::10.0.0.1");
        assert!(result.is_ok());
        assert_eq!(orch.tsa_cache.len(), 0);
    }

    #[test]
    fn test_tsa_multiple_sessions_selective_shutdown() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        // Create sessions: some with shutdown flag, some without
        for i in 1..=4 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let shutdown_flag = i % 2 == 0; // shutdown for 2 and 4
            let config = BfdSessionConfig::new(key)
                .with_shutdown_bfd_during_tsa(shutdown_flag);

            orch.create_session(config).unwrap();
        }

        assert_eq!(orch.session_count(), 4);

        // Enable TSA
        orch.handle_tsa_state_change(true).unwrap();

        // Only sessions 2 and 4 should be shut down
        assert_eq!(orch.session_count(), 2);
        assert_eq!(orch.tsa_cache.len(), 2);
        assert_eq!(orch.stats().tsa_shutdowns, 2);

        // Sessions 1 and 3 should still be active
        assert!(orch.get_session("default::10.0.0.1").is_some());
        assert!(orch.get_session("default::10.0.0.3").is_some());
    }

    #[test]
    fn test_tsa_restore_all_sessions() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        // Create sessions with shutdown flag
        for i in 1..=3 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let config = BfdSessionConfig::new(key)
                .with_shutdown_bfd_during_tsa(true);

            orch.create_session(config).unwrap();
        }

        // Enable TSA
        orch.handle_tsa_state_change(true).unwrap();
        assert_eq!(orch.session_count(), 0);
        assert_eq!(orch.tsa_cache.len(), 3);

        // Disable TSA - should restore all
        orch.handle_tsa_state_change(false).unwrap();
        assert_eq!(orch.session_count(), 3);
        assert_eq!(orch.tsa_cache.len(), 0);
        assert_eq!(orch.stats().tsa_restores, 3);
    }

    // ------------------------------------------------------------------------
    // Statistics Tracking Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_stats_sessions_created() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        for i in 1..=5 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let config = BfdSessionConfig::new(key);
            orch.create_session(config).unwrap();
        }

        assert_eq!(orch.stats().sessions_created, 5);
    }

    #[test]
    fn test_stats_sessions_removed() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        for i in 1..=5 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let config = BfdSessionConfig::new(key);
            orch.create_session(config).unwrap();
        }

        // Remove 3 sessions
        for i in 1..=3 {
            orch.remove_session(&format!("default::10.0.0.{}", i)).unwrap();
        }

        assert_eq!(orch.stats().sessions_removed, 3);
        assert_eq!(orch.session_count(), 2);
    }

    #[test]
    fn test_stats_state_changes() {
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

        // Multiple state changes
        orch.handle_state_change(sai_oid, BfdSessionState::Init).unwrap();
        orch.handle_state_change(sai_oid, BfdSessionState::Up).unwrap();
        orch.handle_state_change(sai_oid, BfdSessionState::Down).unwrap();
        orch.handle_state_change(sai_oid, BfdSessionState::Up).unwrap();

        assert_eq!(orch.stats().state_changes, 4);
    }

    #[test]
    fn test_stats_tsa_shutdowns_and_restores() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        for i in 1..=3 {
            let key = BfdSessionKey::new(
                "default",
                None,
                IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)),
            );
            let config = BfdSessionConfig::new(key)
                .with_shutdown_bfd_during_tsa(true);

            orch.create_session(config).unwrap();
        }

        // Enable TSA
        orch.handle_tsa_state_change(true).unwrap();
        assert_eq!(orch.stats().tsa_shutdowns, 3);

        // Disable TSA
        orch.handle_tsa_state_change(false).unwrap();
        assert_eq!(orch.stats().tsa_restores, 3);
    }

    // ------------------------------------------------------------------------
    // Software BFD Mode Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_software_bfd_remove_session() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::with_software_bfd());
        orch.set_callbacks(callbacks.clone());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        orch.create_session(config).unwrap();

        // Remove should succeed without SAI interaction
        let result = orch.remove_session("default::10.0.0.1");
        assert!(result.is_ok());

        // No SAI sessions should be removed
        let removed = callbacks.removed_sessions.lock().unwrap();
        assert_eq!(removed.len(), 0);
    }

    // ------------------------------------------------------------------------
    // Initialization and Configuration Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_orch_initialization() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());
        assert!(!orch.is_initialized());

        orch.set_initialized();
        assert!(orch.is_initialized());
    }

    #[test]
    fn test_create_session_without_callbacks() {
        let mut orch = BfdOrch::new(BfdOrchConfig::default());

        let key = BfdSessionKey::new(
            "default",
            None,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let config = BfdSessionConfig::new(key);

        let result = orch.create_session(config);
        assert!(matches!(result, Err(BfdOrchError::InvalidConfig(_))));
    }

    #[test]
    fn test_orch_config() {
        let config = BfdOrchConfig::default();
        let orch = BfdOrch::new(config);

        // Verify config is accessible
        let _cfg = orch.config();
    }
}
