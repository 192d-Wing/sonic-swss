//! DTel (Data Plane Telemetry) orchestration logic.
//!
//! DTel provides in-band telemetry for network visibility, including:
//! - INT (In-band Network Telemetry) sessions for hop-by-hop metadata
//! - Event reporting for flow state, queue events, and drops
//! - Watchlist management for selective telemetry

use super::types::{DtelEventType, IntSessionConfig, IntSessionEntry};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Result type for DtelOrch operations.
pub type Result<T> = std::result::Result<T, DtelOrchError>;

#[derive(Debug, Clone)]
pub enum DtelOrchError {
    SessionExists(String),
    SessionNotFound(String),
    EventNotFound(DtelEventType),
    InvalidConfig(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct DtelOrchConfig {
    /// Enable INT endpoint mode (source/sink).
    pub int_endpoint: bool,
    /// Enable INT transit mode (hop-by-hop).
    pub int_transit: bool,
    /// Enable postcard-based telemetry.
    pub postcard_enable: bool,
    /// Enable drop report generation.
    pub drop_report_enable: bool,
    /// Enable queue report generation.
    pub queue_report_enable: bool,
    /// Sink port list for INT reports.
    pub sink_port_list: Vec<String>,
    /// DSCP value for INT packets.
    pub int_dscp: u8,
}

#[derive(Debug, Clone, Default)]
pub struct DtelOrchStats {
    pub sessions_created: u64,
    pub sessions_removed: u64,
    pub events_enabled: u64,
    pub events_disabled: u64,
    pub watchlist_entries: u64,
    pub errors: u64,
}

/// Callbacks for DTel SAI operations.
pub trait DtelOrchCallbacks: Send + Sync {
    /// Create an INT session in SAI.
    fn create_int_session(
        &self,
        config: &IntSessionConfig,
    ) -> Result<RawSaiObjectId>;

    /// Remove an INT session from SAI.
    fn remove_int_session(&self, session_oid: RawSaiObjectId) -> Result<()>;

    /// Enable a DTel event type.
    fn enable_event(&self, event_type: DtelEventType) -> Result<RawSaiObjectId>;

    /// Disable a DTel event type.
    fn disable_event(&self, event_oid: RawSaiObjectId) -> Result<()>;

    /// Set DTel switch attributes (INT mode, DSCP, etc.).
    fn set_dtel_attribute(&self, attr_name: &str, attr_value: &str) -> Result<()>;

    /// Write session state to STATE_DB.
    fn write_state_db(&self, session_id: &str, state: &str) -> Result<()>;

    /// Remove session state from STATE_DB.
    fn remove_state_db(&self, session_id: &str) -> Result<()>;

    /// Notification callback when session is created.
    fn on_session_created(&self, session_id: &str, session_oid: RawSaiObjectId);

    /// Notification callback when session is removed.
    fn on_session_removed(&self, session_id: &str);

    /// Notification callback when event is enabled/disabled.
    fn on_event_state_changed(&self, event_type: DtelEventType, enabled: bool);
}

/// DTel event entry tracking enabled events.
#[derive(Debug)]
pub struct DtelEventEntry {
    pub event_type: DtelEventType,
    pub event_oid: RawSaiObjectId,
    pub enabled: bool,
}

/// DTel watchlist entry for selective telemetry.
#[derive(Debug, Clone)]
pub struct WatchlistEntry {
    pub acl_table_oid: RawSaiObjectId,
    pub acl_rule_oid: RawSaiObjectId,
}

pub struct DtelOrch<C: DtelOrchCallbacks> {
    config: DtelOrchConfig,
    stats: DtelOrchStats,
    callbacks: Option<Arc<C>>,
    /// INT sessions indexed by session ID.
    sessions: HashMap<String, Arc<IntSessionEntry>>,
    /// Enabled events indexed by event type.
    events: HashMap<DtelEventType, DtelEventEntry>,
    /// DTel watchlist entries.
    watchlist: HashMap<String, WatchlistEntry>,
    /// DTel SAI object (global).
    dtel_oid: Option<RawSaiObjectId>,
}

impl<C: DtelOrchCallbacks> DtelOrch<C> {
    pub fn new(config: DtelOrchConfig) -> Self {
        Self {
            config,
            stats: DtelOrchStats::default(),
            callbacks: None,
            sessions: HashMap::new(),
            events: HashMap::new(),
            watchlist: HashMap::new(),
            dtel_oid: None,
        }
    }

    pub fn with_callbacks(config: DtelOrchConfig, callbacks: Arc<C>) -> Self {
        Self {
            config,
            stats: DtelOrchStats::default(),
            callbacks: Some(callbacks),
            sessions: HashMap::new(),
            events: HashMap::new(),
            watchlist: HashMap::new(),
            dtel_oid: None,
        }
    }

    pub fn stats(&self) -> &DtelOrchStats {
        &self.stats
    }

    pub fn config(&self) -> &DtelOrchConfig {
        &self.config
    }

    /// Add an INT session.
    pub fn add_session(&mut self, config: IntSessionConfig) -> Result<()> {
        let session_id = config.session_id.clone();

        if self.sessions.contains_key(&session_id) {
            return Err(DtelOrchError::SessionExists(session_id));
        }

        let session_oid = if let Some(ref callbacks) = self.callbacks {
            callbacks.create_int_session(&config)?
        } else {
            // No callbacks, use placeholder OID for testing
            0x1000 + self.sessions.len() as u64
        };

        let entry = Arc::new(IntSessionEntry::new(session_oid, config));
        self.sessions.insert(session_id.clone(), entry);
        self.stats.sessions_created += 1;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.on_session_created(&session_id, session_oid);
            let _ = callbacks.write_state_db(&session_id, "active");
        }

        Ok(())
    }

    /// Remove an INT session.
    pub fn remove_session(&mut self, session_id: &str) -> Result<()> {
        let entry = self
            .sessions
            .remove(session_id)
            .ok_or_else(|| DtelOrchError::SessionNotFound(session_id.to_string()))?;

        // Check reference count before removing
        let ref_count = entry.ref_count.load(Ordering::SeqCst);
        if ref_count > 1 {
            // Session still in use, put it back
            self.sessions.insert(session_id.to_string(), entry);
            return Err(DtelOrchError::InvalidConfig(format!(
                "Session {} still has {} references",
                session_id, ref_count
            )));
        }

        if let Some(ref callbacks) = self.callbacks {
            callbacks.remove_int_session(entry.session_oid)?;
            callbacks.on_session_removed(session_id);
            let _ = callbacks.remove_state_db(session_id);
        }

        self.stats.sessions_removed += 1;
        Ok(())
    }

    /// Get an INT session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<Arc<IntSessionEntry>> {
        self.sessions.get(session_id).cloned()
    }

    /// Increment reference count for a session.
    pub fn add_session_ref(&self, session_id: &str) -> Result<()> {
        let entry = self
            .sessions
            .get(session_id)
            .ok_or_else(|| DtelOrchError::SessionNotFound(session_id.to_string()))?;

        entry.ref_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Decrement reference count for a session.
    pub fn release_session_ref(&self, session_id: &str) -> Result<u64> {
        let entry = self
            .sessions
            .get(session_id)
            .ok_or_else(|| DtelOrchError::SessionNotFound(session_id.to_string()))?;

        let prev = entry.ref_count.fetch_sub(1, Ordering::SeqCst);
        Ok(prev - 1)
    }

    /// Enable a DTel event type.
    pub fn enable_event(&mut self, event_type: DtelEventType) -> Result<()> {
        if self.events.contains_key(&event_type) {
            // Already enabled
            return Ok(());
        }

        let event_oid = if let Some(ref callbacks) = self.callbacks {
            callbacks.enable_event(event_type)?
        } else {
            0x2000 + self.events.len() as u64
        };

        let entry = DtelEventEntry {
            event_type,
            event_oid,
            enabled: true,
        };
        self.events.insert(event_type, entry);
        self.stats.events_enabled += 1;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.on_event_state_changed(event_type, true);
        }

        Ok(())
    }

    /// Disable a DTel event type.
    pub fn disable_event(&mut self, event_type: DtelEventType) -> Result<()> {
        let entry = self
            .events
            .remove(&event_type)
            .ok_or(DtelOrchError::EventNotFound(event_type))?;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.disable_event(entry.event_oid)?;
            callbacks.on_event_state_changed(event_type, false);
        }

        self.stats.events_disabled += 1;
        Ok(())
    }

    /// Check if an event type is enabled.
    pub fn is_event_enabled(&self, event_type: DtelEventType) -> bool {
        self.events
            .get(&event_type)
            .map(|e| e.enabled)
            .unwrap_or(false)
    }

    /// Get all enabled event types.
    pub fn get_enabled_events(&self) -> Vec<DtelEventType> {
        self.events
            .values()
            .filter(|e| e.enabled)
            .map(|e| e.event_type)
            .collect()
    }

    /// Add a watchlist entry.
    pub fn add_watchlist_entry(&mut self, key: String, entry: WatchlistEntry) {
        self.watchlist.insert(key, entry);
        self.stats.watchlist_entries = self.watchlist.len() as u64;
    }

    /// Remove a watchlist entry.
    pub fn remove_watchlist_entry(&mut self, key: &str) -> Option<WatchlistEntry> {
        let entry = self.watchlist.remove(key);
        self.stats.watchlist_entries = self.watchlist.len() as u64;
        entry
    }

    /// Get a watchlist entry.
    pub fn get_watchlist_entry(&self, key: &str) -> Option<&WatchlistEntry> {
        self.watchlist.get(key)
    }

    /// Update DTel configuration.
    pub fn update_config(&mut self, new_config: DtelOrchConfig) -> Result<()> {
        if let Some(ref callbacks) = self.callbacks {
            // Update INT endpoint mode
            if new_config.int_endpoint != self.config.int_endpoint {
                callbacks.set_dtel_attribute(
                    "INT_ENDPOINT",
                    if new_config.int_endpoint { "true" } else { "false" },
                )?;
            }

            // Update INT transit mode
            if new_config.int_transit != self.config.int_transit {
                callbacks.set_dtel_attribute(
                    "INT_TRANSIT",
                    if new_config.int_transit { "true" } else { "false" },
                )?;
            }

            // Update drop report
            if new_config.drop_report_enable != self.config.drop_report_enable {
                callbacks.set_dtel_attribute(
                    "DROP_REPORT",
                    if new_config.drop_report_enable {
                        "true"
                    } else {
                        "false"
                    },
                )?;
            }

            // Update queue report
            if new_config.queue_report_enable != self.config.queue_report_enable {
                callbacks.set_dtel_attribute(
                    "QUEUE_REPORT",
                    if new_config.queue_report_enable {
                        "true"
                    } else {
                        "false"
                    },
                )?;
            }

            // Update DSCP
            if new_config.int_dscp != self.config.int_dscp {
                callbacks.set_dtel_attribute("INT_DSCP", &new_config.int_dscp.to_string())?;
            }
        }

        self.config = new_config;
        Ok(())
    }

    /// Get session count.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get enabled event count.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Get watchlist entry count.
    pub fn watchlist_count(&self) -> usize {
        self.watchlist.len()
    }

    /// Set the DTel SAI object ID.
    pub fn set_dtel_oid(&mut self, oid: RawSaiObjectId) {
        self.dtel_oid = Some(oid);
    }

    /// Get the DTel SAI object ID.
    pub fn get_dtel_oid(&self) -> Option<RawSaiObjectId> {
        self.dtel_oid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock callbacks for testing without SAI.
    struct MockDtelCallbacks;

    impl DtelOrchCallbacks for MockDtelCallbacks {
        fn create_int_session(&self, config: &IntSessionConfig) -> Result<RawSaiObjectId> {
            Ok(0x1000)
        }

        fn remove_int_session(&self, _session_oid: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn enable_event(&self, _event_type: DtelEventType) -> Result<RawSaiObjectId> {
            Ok(0x2000)
        }

        fn disable_event(&self, _event_oid: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn set_dtel_attribute(&self, _attr_name: &str, _attr_value: &str) -> Result<()> {
            Ok(())
        }

        fn write_state_db(&self, _session_id: &str, _state: &str) -> Result<()> {
            Ok(())
        }

        fn remove_state_db(&self, _session_id: &str) -> Result<()> {
            Ok(())
        }

        fn on_session_created(&self, _session_id: &str, _session_oid: RawSaiObjectId) {}
        fn on_session_removed(&self, _session_id: &str) {}
        fn on_event_state_changed(&self, _event_type: DtelEventType, _enabled: bool) {}
    }

    #[test]
    fn test_new_dtel_orch_with_default_config() {
        let config = DtelOrchConfig::default();
        let orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(config);

        assert_eq!(orch.stats().sessions_created, 0);
    }

    #[test]
    fn test_dtel_orch_stats_default() {
        let stats = DtelOrchStats::default();

        assert_eq!(stats.sessions_created, 0);
    }

    #[test]
    fn test_dtel_orch_stats_clone() {
        let stats = DtelOrchStats {
            sessions_created: 42,
            sessions_removed: 0,
            events_enabled: 0,
            events_disabled: 0,
            watchlist_entries: 0,
            errors: 0,
        };
        let cloned = stats.clone();

        assert_eq!(cloned.sessions_created, 42);
    }

    #[test]
    fn test_dtel_orch_config_default() {
        let config = DtelOrchConfig::default();

        // Config should have sensible defaults
        assert!(!config.int_endpoint);
        assert!(!config.int_transit);
        let _ = format!("{:?}", config);
    }

    #[test]
    fn test_dtel_orch_config_clone() {
        let config = DtelOrchConfig::default();
        let cloned = config.clone();

        assert_eq!(config.int_endpoint, cloned.int_endpoint);
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn test_dtel_orch_error_session_exists() {
        let err = DtelOrchError::SessionExists("session1".to_string());

        assert!(matches!(err, DtelOrchError::SessionExists(_)));
    }

    #[test]
    fn test_dtel_orch_error_session_not_found() {
        let err = DtelOrchError::SessionNotFound("session2".to_string());

        assert!(matches!(err, DtelOrchError::SessionNotFound(_)));
    }

    #[test]
    fn test_dtel_orch_error_clone() {
        let err = DtelOrchError::SessionExists("test_session".to_string());
        let cloned = err.clone();

        assert!(matches!(cloned, DtelOrchError::SessionExists(_)));
    }

    #[test]
    fn test_multiple_dtel_orch_instances() {
        let config1 = DtelOrchConfig::default();
        let config2 = DtelOrchConfig::default();

        let orch1: DtelOrch<MockDtelCallbacks> = DtelOrch::new(config1);
        let orch2: DtelOrch<MockDtelCallbacks> = DtelOrch::new(config2);

        assert_eq!(orch1.stats().sessions_created, 0);
        assert_eq!(orch2.stats().sessions_created, 0);
    }

    #[test]
    fn test_dtel_orch_stats_access() {
        let config = DtelOrchConfig::default();
        let orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(config);
        let stats = orch.stats();

        assert_eq!(stats.sessions_created, 0);
    }

    // ===== Statistics tracking tests =====

    #[test]
    fn test_dtel_orch_stats_sessions_created_counter() {
        let mut stats = DtelOrchStats::default();

        stats.sessions_created = 15;
        assert_eq!(stats.sessions_created, 15);

        stats.sessions_created += 5;
        assert_eq!(stats.sessions_created, 20);
    }

    #[test]
    fn test_dtel_orch_stats_modification() {
        let orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let stats = orch.stats();
        assert_eq!(stats.sessions_created, 0);
    }

    #[test]
    fn test_dtel_orch_stats_clone_preserves_values() {
        let stats1 = DtelOrchStats {
            sessions_created: 100,
            sessions_removed: 10,
            events_enabled: 5,
            events_disabled: 2,
            watchlist_entries: 3,
            errors: 1,
        };
        let stats2 = stats1.clone();

        assert_eq!(stats1.sessions_created, stats2.sessions_created);
        assert_eq!(stats2.sessions_created, 100);
    }

    // ===== Error handling tests =====

    #[test]
    fn test_dtel_orch_error_session_exists_details() {
        let err = DtelOrchError::SessionExists("int_session_1".to_string());

        match err {
            DtelOrchError::SessionExists(name) => {
                assert_eq!(name, "int_session_1");
            }
            _ => panic!("Expected SessionExists error"),
        }
    }

    #[test]
    fn test_dtel_orch_error_session_not_found_details() {
        let err = DtelOrchError::SessionNotFound("int_session_2".to_string());

        match err {
            DtelOrchError::SessionNotFound(name) => {
                assert_eq!(name, "int_session_2");
            }
            _ => panic!("Expected SessionNotFound error"),
        }
    }

    #[test]
    fn test_dtel_orch_error_debug_format() {
        let err1 = DtelOrchError::SessionExists("test".to_string());
        let err2 = DtelOrchError::SessionNotFound("test2".to_string());

        let debug1 = format!("{:?}", err1);
        let debug2 = format!("{:?}", err2);

        assert!(debug1.contains("SessionExists"));
        assert!(debug2.contains("SessionNotFound"));
    }

    #[test]
    fn test_dtel_orch_error_clone_session_exists() {
        let err1 = DtelOrchError::SessionExists("session_a".to_string());
        let err2 = err1.clone();

        match (err1, err2) {
            (DtelOrchError::SessionExists(n1), DtelOrchError::SessionExists(n2)) => {
                assert_eq!(n1, n2);
            }
            _ => panic!("Error types don't match"),
        }
    }

    #[test]
    fn test_dtel_orch_error_clone_session_not_found() {
        let err1 = DtelOrchError::SessionNotFound("session_b".to_string());
        let err2 = err1.clone();

        match (err1, err2) {
            (DtelOrchError::SessionNotFound(n1), DtelOrchError::SessionNotFound(n2)) => {
                assert_eq!(n1, n2);
            }
            _ => panic!("Error types don't match"),
        }
    }

    #[test]
    fn test_dtel_orch_error_different_variants() {
        let err1 = DtelOrchError::SessionExists("test".to_string());
        let err2 = DtelOrchError::SessionNotFound("test".to_string());

        assert!(matches!(err1, DtelOrchError::SessionExists(_)));
        assert!(matches!(err2, DtelOrchError::SessionNotFound(_)));
    }

    // ===== DtelEventType tests =====

    #[test]
    fn test_dtel_event_type_equality() {
        assert_eq!(DtelEventType::FlowState, DtelEventType::FlowState);
        assert_eq!(DtelEventType::DropReport, DtelEventType::DropReport);
        assert_ne!(DtelEventType::FlowState, DtelEventType::DropReport);
    }

    #[test]
    fn test_dtel_event_type_copy() {
        let event1 = DtelEventType::FlowState;
        let event2 = event1;

        assert_eq!(event1, event2);
    }

    #[test]
    fn test_dtel_event_type_all_variants() {
        let events = vec![
            DtelEventType::FlowState,
            DtelEventType::FlowReportAllPackets,
            DtelEventType::FlowTcpFlag,
            DtelEventType::QueueReportThresholdBreach,
            DtelEventType::QueueReportTailDrop,
            DtelEventType::DropReport,
        ];

        assert_eq!(events.len(), 6);
    }

    #[test]
    fn test_dtel_event_type_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(DtelEventType::FlowState);
        set.insert(DtelEventType::DropReport);
        set.insert(DtelEventType::FlowState); // duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_dtel_event_type_queue_events() {
        let threshold = DtelEventType::QueueReportThresholdBreach;
        let tail_drop = DtelEventType::QueueReportTailDrop;

        assert_ne!(threshold, tail_drop);
        assert!(matches!(threshold, DtelEventType::QueueReportThresholdBreach));
        assert!(matches!(tail_drop, DtelEventType::QueueReportTailDrop));
    }

    #[test]
    fn test_dtel_event_type_flow_events() {
        let flow_state = DtelEventType::FlowState;
        let flow_all = DtelEventType::FlowReportAllPackets;
        let flow_tcp = DtelEventType::FlowTcpFlag;

        assert_ne!(flow_state, flow_all);
        assert_ne!(flow_state, flow_tcp);
        assert_ne!(flow_all, flow_tcp);
    }

    // ===== IntSessionEntry tests =====

    #[test]
    fn test_int_session_entry_new() {
        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let entry = IntSessionEntry::new(0x1234, config);

        assert_eq!(entry.session_oid, 0x1234);
        assert_eq!(entry.config.session_id, "session1");
    }

    #[test]
    fn test_int_session_entry_ref_count_starts_at_one() {
        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let entry = IntSessionEntry::new(0x1234, config);

        assert_eq!(entry.ref_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_int_session_entry_atomic_ref_count() {
        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let entry = IntSessionEntry::new(0x1234, config);

        // Test atomic operations
        entry.ref_count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(entry.ref_count.load(Ordering::SeqCst), 2);

        entry.ref_count.fetch_sub(1, Ordering::SeqCst);
        assert_eq!(entry.ref_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_int_session_config_clone() {
        let config1 = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 16,
        };

        let config2 = config1.clone();

        assert_eq!(config1.session_id, config2.session_id);
        assert_eq!(config1.collect_switch_id, config2.collect_switch_id);
        assert_eq!(config1.max_hop_count, config2.max_hop_count);
    }

    #[test]
    fn test_int_session_config_with_switch_id() {
        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        assert!(config.collect_switch_id);
    }

    #[test]
    fn test_int_session_config_without_switch_id() {
        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: false,
            max_hop_count: 8,
        };

        assert!(!config.collect_switch_id);
    }

    #[test]
    fn test_int_session_config_max_hop_count() {
        let config1 = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 4,
        };

        let config2 = IntSessionConfig {
            session_id: "session2".to_string(),
            collect_switch_id: true,
            max_hop_count: 32,
        };

        assert_eq!(config1.max_hop_count, 4);
        assert_eq!(config2.max_hop_count, 32);
        assert_ne!(config1.max_hop_count, config2.max_hop_count);
    }

    #[test]
    fn test_int_session_entry_with_different_oids() {
        let config1 = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let config2 = IntSessionConfig {
            session_id: "session2".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let entry1 = IntSessionEntry::new(0x1000, config1);
        let entry2 = IntSessionEntry::new(0x2000, config2);

        assert_ne!(entry1.session_oid, entry2.session_oid);
    }

    // ===== Config tests =====

    #[test]
    fn test_dtel_orch_config_equality() {
        let config1 = DtelOrchConfig::default();
        let config2 = DtelOrchConfig::default();

        // Both are default configs
        let _ = format!("{:?}", config1);
        let _ = format!("{:?}", config2);
    }

    #[test]
    fn test_dtel_orch_new_with_custom_config() {
        let config = DtelOrchConfig {
            int_endpoint: true,
            int_transit: false,
            postcard_enable: false,
            drop_report_enable: true,
            queue_report_enable: false,
            sink_port_list: vec!["Ethernet0".to_string()],
            int_dscp: 8,
        };
        let orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(config);

        assert_eq!(orch.stats().sessions_created, 0);
        assert!(orch.config().int_endpoint);
        assert!(orch.config().drop_report_enable);
    }

    // ===== Session management tests =====

    #[test]
    fn test_add_session() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let result = orch.add_session(config);
        assert!(result.is_ok());
        assert_eq!(orch.session_count(), 1);
        assert_eq!(orch.stats().sessions_created, 1);
    }

    #[test]
    fn test_add_session_duplicate() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        orch.add_session(config.clone()).unwrap();
        let result = orch.add_session(config);

        assert!(matches!(result, Err(DtelOrchError::SessionExists(_))));
        assert_eq!(orch.session_count(), 1);
    }

    #[test]
    fn test_remove_session() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        orch.add_session(config).unwrap();
        let result = orch.remove_session("session1");

        assert!(result.is_ok());
        assert_eq!(orch.session_count(), 0);
        assert_eq!(orch.stats().sessions_removed, 1);
    }

    #[test]
    fn test_remove_session_not_found() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let result = orch.remove_session("nonexistent");
        assert!(matches!(result, Err(DtelOrchError::SessionNotFound(_))));
    }

    #[test]
    fn test_get_session() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        orch.add_session(config).unwrap();

        let session = orch.get_session("session1");
        assert!(session.is_some());
        assert_eq!(session.unwrap().config.session_id, "session1");

        let missing = orch.get_session("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_session_ref_counting() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        orch.add_session(config).unwrap();

        // Add reference
        orch.add_session_ref("session1").unwrap();

        // Try to remove - should fail due to ref count
        let result = orch.remove_session("session1");
        assert!(matches!(result, Err(DtelOrchError::InvalidConfig(_))));

        // Release reference
        let remaining = orch.release_session_ref("session1").unwrap();
        assert_eq!(remaining, 1);

        // Now removal should succeed
        let result = orch.remove_session("session1");
        assert!(result.is_ok());
    }

    // ===== Event management tests =====

    #[test]
    fn test_enable_event() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let result = orch.enable_event(DtelEventType::DropReport);
        assert!(result.is_ok());
        assert!(orch.is_event_enabled(DtelEventType::DropReport));
        assert_eq!(orch.event_count(), 1);
        assert_eq!(orch.stats().events_enabled, 1);
    }

    #[test]
    fn test_enable_event_idempotent() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        orch.enable_event(DtelEventType::DropReport).unwrap();
        orch.enable_event(DtelEventType::DropReport).unwrap(); // Should not error

        assert_eq!(orch.event_count(), 1);
        assert_eq!(orch.stats().events_enabled, 1);
    }

    #[test]
    fn test_disable_event() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        orch.enable_event(DtelEventType::DropReport).unwrap();
        let result = orch.disable_event(DtelEventType::DropReport);

        assert!(result.is_ok());
        assert!(!orch.is_event_enabled(DtelEventType::DropReport));
        assert_eq!(orch.event_count(), 0);
        assert_eq!(orch.stats().events_disabled, 1);
    }

    #[test]
    fn test_disable_event_not_found() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let result = orch.disable_event(DtelEventType::DropReport);
        assert!(matches!(result, Err(DtelOrchError::EventNotFound(_))));
    }

    #[test]
    fn test_get_enabled_events() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        orch.enable_event(DtelEventType::DropReport).unwrap();
        orch.enable_event(DtelEventType::FlowState).unwrap();
        orch.enable_event(DtelEventType::QueueReportTailDrop).unwrap();

        let events = orch.get_enabled_events();
        assert_eq!(events.len(), 3);
        assert!(events.contains(&DtelEventType::DropReport));
        assert!(events.contains(&DtelEventType::FlowState));
        assert!(events.contains(&DtelEventType::QueueReportTailDrop));
    }

    // ===== Watchlist tests =====

    #[test]
    fn test_watchlist_management() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        let entry = WatchlistEntry {
            acl_table_oid: 0x3000,
            acl_rule_oid: 0x3001,
        };

        orch.add_watchlist_entry("flow1".to_string(), entry);
        assert_eq!(orch.watchlist_count(), 1);
        assert_eq!(orch.stats().watchlist_entries, 1);

        let found = orch.get_watchlist_entry("flow1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().acl_table_oid, 0x3000);

        let removed = orch.remove_watchlist_entry("flow1");
        assert!(removed.is_some());
        assert_eq!(orch.watchlist_count(), 0);
    }

    // ===== Integration tests =====

    #[test]
    fn test_dtel_orch_multiple_instances_independent() {
        let config1 = DtelOrchConfig::default();
        let config2 = DtelOrchConfig::default();

        let orch1: DtelOrch<MockDtelCallbacks> = DtelOrch::new(config1);
        let orch2: DtelOrch<MockDtelCallbacks> = DtelOrch::new(config2);

        assert_eq!(orch1.stats().sessions_created, 0);
        assert_eq!(orch2.stats().sessions_created, 0);
    }

    #[test]
    fn test_int_session_entry_debug() {
        let config = IntSessionConfig {
            session_id: "debug_session".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let entry = IntSessionEntry::new(0xabcd, config);
        let debug_str = format!("{:?}", entry);

        assert!(debug_str.contains("IntSessionEntry"));
    }

    #[test]
    fn test_int_session_config_debug() {
        let config = IntSessionConfig {
            session_id: "debug_session".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("IntSessionConfig"));
    }

    #[test]
    fn test_int_session_entry_multiple_ref_count_increments() {
        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let entry = IntSessionEntry::new(0x1234, config);

        // Start at 1
        assert_eq!(entry.ref_count.load(Ordering::SeqCst), 1);

        // Increment multiple times
        for i in 2..=10 {
            entry.ref_count.fetch_add(1, Ordering::SeqCst);
            assert_eq!(entry.ref_count.load(Ordering::SeqCst), i);
        }
    }

    #[test]
    fn test_int_session_entry_ref_count_decrement() {
        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        let entry = IntSessionEntry::new(0x1234, config);

        // Start at 1, add 4 more to get to 5
        entry.ref_count.fetch_add(4, Ordering::SeqCst);
        assert_eq!(entry.ref_count.load(Ordering::SeqCst), 5);

        // Decrement to 0
        for i in (0..5).rev() {
            entry.ref_count.fetch_sub(1, Ordering::SeqCst);
            assert_eq!(entry.ref_count.load(Ordering::SeqCst), i);
        }
    }

    #[test]
    fn test_dtel_oid_management() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        assert!(orch.get_dtel_oid().is_none());

        orch.set_dtel_oid(0x5000);
        assert_eq!(orch.get_dtel_oid(), Some(0x5000));
    }

    #[test]
    fn test_multiple_sessions() {
        let mut orch: DtelOrch<MockDtelCallbacks> = DtelOrch::new(DtelOrchConfig::default());

        for i in 0..5 {
            let config = IntSessionConfig {
                session_id: format!("session{}", i),
                collect_switch_id: true,
                max_hop_count: 8,
            };
            orch.add_session(config).unwrap();
        }

        assert_eq!(orch.session_count(), 5);
        assert_eq!(orch.stats().sessions_created, 5);

        // Remove all sessions
        for i in 0..5 {
            orch.remove_session(&format!("session{}", i)).unwrap();
        }

        assert_eq!(orch.session_count(), 0);
        assert_eq!(orch.stats().sessions_removed, 5);
    }

    #[test]
    fn test_with_callbacks() {
        let callbacks = Arc::new(MockDtelCallbacks);
        let mut orch = DtelOrch::with_callbacks(DtelOrchConfig::default(), callbacks);

        let config = IntSessionConfig {
            session_id: "session1".to_string(),
            collect_switch_id: true,
            max_hop_count: 8,
        };

        orch.add_session(config).unwrap();
        assert_eq!(orch.session_count(), 1);
    }
}
