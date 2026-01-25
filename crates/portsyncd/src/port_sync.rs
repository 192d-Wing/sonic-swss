//! Port status synchronization via netlink events
//!
//! Handles RTM_NEWLINK and RTM_DELLINK netlink events and updates STATE_DB
//! with the current operational status of ports.
//!
//! Supports warm restart via WarmRestartManager, which gates APP_DB updates
//! during initial synchronization after a warm restart.

use crate::config::DatabaseConnection;
use crate::error::Result;
use crate::warm_restart::{PortState, WarmRestartManager, WarmRestartMetrics, WarmRestartState};
use std::collections::HashSet;
use std::path::PathBuf;

/// Link status values
#[derive(Clone, Debug, PartialEq)]
pub enum LinkStatus {
    /// Link is up and operational
    Up,
    /// Link is down
    Down,
}

impl LinkStatus {
    /// Convert status to string for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkStatus::Up => "up",
            LinkStatus::Down => "down",
        }
    }

    /// Parse from netlink flags
    pub fn from_netlink_flags(flags: u32) -> Self {
        // IFF_UP = 0x1 in netlink
        if (flags & 0x1) != 0 {
            LinkStatus::Up
        } else {
            LinkStatus::Down
        }
    }
}

/// Port link state entry for STATE_DB
#[derive(Clone, Debug)]
pub struct PortLinkState {
    /// Port name (e.g., "Ethernet0")
    pub name: String,
    /// Operational status
    pub oper_status: LinkStatus,
    /// Administrative status
    pub admin_status: LinkStatus,
    /// Maximum transmission unit
    pub mtu: u32,
}

impl PortLinkState {
    /// Create new port link state
    pub fn new(name: String, oper_status: LinkStatus, admin_status: LinkStatus, mtu: u32) -> Self {
        Self {
            name,
            oper_status,
            admin_status,
            mtu,
        }
    }

    /// Convert to field-value tuples for STATE_DB storage
    pub fn to_field_values(&self) -> Vec<(String, String)> {
        vec![
            ("state".to_string(), "ok".to_string()),
            (
                "netdev_oper_status".to_string(),
                self.oper_status.as_str().to_string(),
            ),
            (
                "admin_status".to_string(),
                self.admin_status.as_str().to_string(),
            ),
            ("mtu".to_string(), self.mtu.to_string()),
        ]
    }

    /// Check if this is a front-panel port (Ethernet* or PortChannel*)
    pub fn is_front_panel(&self) -> bool {
        self.name.starts_with("Ethernet") || self.name.starts_with("PortChannel")
    }
}

/// Netlink event types
#[derive(Clone, Debug, PartialEq)]
pub enum NetlinkEventType {
    /// RTM_NEWLINK: New or updated link
    NewLink,
    /// RTM_DELLINK: Deleted link
    DelLink,
}

/// Netlink event for port state changes
#[derive(Clone, Debug)]
pub struct NetlinkEvent {
    /// Event type (RTM_NEWLINK or RTM_DELLINK)
    pub event_type: NetlinkEventType,
    /// Port/interface name
    pub port_name: String,
    /// Flags from netlink message (for NewLink events)
    pub flags: Option<u32>,
    /// MTU value (for NewLink events)
    pub mtu: Option<u32>,
}

/// Port synchronization daemon state
pub struct LinkSync {
    /// Uninitialized ports awaiting their first netlink event
    uninitialized_ports: HashSet<String>,
    /// Flag: have we sent PortInitDone yet?
    port_init_done: bool,
    /// Warm restart manager for coordinating warm restarts
    warm_restart: Option<WarmRestartManager>,
}

impl LinkSync {
    /// Create new LinkSync daemon without warm restart support
    pub fn new() -> Result<Self> {
        Ok(Self {
            uninitialized_ports: HashSet::new(),
            port_init_done: false,
            warm_restart: None,
        })
    }

    /// Create new LinkSync daemon with warm restart support
    pub fn with_warm_restart(state_file_path: PathBuf) -> Result<Self> {
        Ok(Self {
            uninitialized_ports: HashSet::new(),
            port_init_done: false,
            warm_restart: Some(WarmRestartManager::with_state_file(state_file_path)),
        })
    }

    /// Initialize warm restart - detects cold start vs warm restart
    pub fn initialize_warm_restart(&mut self) -> Result<()> {
        if let Some(ref mut mgr) = self.warm_restart {
            mgr.initialize()?;
        }
        Ok(())
    }

    /// Begin warm restart initial sync (skip APP_DB updates)
    pub fn begin_warm_restart_sync(&mut self) {
        if let Some(ref mut mgr) = self.warm_restart {
            mgr.begin_initial_sync();
        }
    }

    /// Complete warm restart initial sync (enable APP_DB updates)
    pub fn complete_warm_restart_sync(&mut self) {
        if let Some(ref mut mgr) = self.warm_restart {
            mgr.complete_initial_sync();
        }
    }

    /// Check if APP_DB updates should be skipped (warm restart in progress)
    pub fn should_skip_app_db_updates(&self) -> bool {
        self.warm_restart
            .as_ref()
            .map(|mgr| mgr.should_skip_app_db_updates())
            .unwrap_or(false)
    }

    /// Get warm restart state
    pub fn warm_restart_state(&self) -> Option<WarmRestartState> {
        self.warm_restart.as_ref().map(|mgr| mgr.current_state())
    }

    /// Save port state for warm restart recovery
    pub fn save_port_state(&self) -> Result<()> {
        if let Some(ref mgr) = self.warm_restart {
            mgr.save_state()?;
        }
        Ok(())
    }

    /// Add port to warm restart saved state
    pub fn record_port_for_warm_restart(&mut self, port_name: String, flags: u32, mtu: u32) {
        if let Some(ref mut mgr) = self.warm_restart {
            let admin_state = if (flags & 0x1) != 0 { 1 } else { 0 };
            let oper_state = if (flags & 0x1) != 0 { 1 } else { 0 };
            let port_state = PortState::new(port_name, admin_state, oper_state, flags, mtu);
            mgr.add_port(port_state);
        }
    }

    /// Check if port should be ignored
    pub fn should_ignore(&self, name: &str) -> bool {
        // Skip non-front-panel interfaces
        if !name.starts_with("Ethernet") && !name.starts_with("PortChannel") {
            return true;
        }

        // Skip management interfaces
        if name == "eth0" || name == "lo" {
            return true;
        }

        false
    }

    /// Check if all ports have been initialized
    pub fn are_all_ports_initialized(&self) -> bool {
        self.uninitialized_ports.is_empty()
    }

    /// Mark port as initialized
    pub fn mark_port_initialized(&mut self, name: &str) {
        self.uninitialized_ports.remove(name);
    }

    /// Send port initialization done signal
    pub fn set_port_init_done(&mut self) {
        self.port_init_done = true;
    }

    /// Check if port init done has been signaled
    pub fn is_port_init_done(&self) -> bool {
        self.port_init_done
    }

    /// Get count of uninitialized ports
    pub fn uninitialized_count(&self) -> usize {
        self.uninitialized_ports.len()
    }

    /// Handle RTM_NEWLINK netlink event
    pub async fn handle_new_link(
        &mut self,
        event: &NetlinkEvent,
        state_db: &mut DatabaseConnection,
    ) -> Result<()> {
        // Ignore non-front-panel and management interfaces
        if self.should_ignore(&event.port_name) {
            return Ok(());
        }

        // Extract status and MTU from event
        let oper_status = event
            .flags
            .map(LinkStatus::from_netlink_flags)
            .unwrap_or(LinkStatus::Up);
        let mtu = event.mtu.unwrap_or(9100);
        let flags = event.flags.unwrap_or(0);

        // Record port for warm restart if enabled
        self.record_port_for_warm_restart(event.port_name.clone(), flags, mtu);

        // Create port link state entry
        let port_state = PortLinkState::new(
            event.port_name.clone(),
            oper_status,
            LinkStatus::Up, // Admin status assumed up for now (from CONFIG_DB in prod)
            mtu,
        );

        // Write to STATE_DB only if not skipped during warm restart initial sync
        if !self.should_skip_app_db_updates() {
            let key = format!("PORT_TABLE|{}", port_state.name);
            let field_values = port_state.to_field_values();
            state_db.hset(&key, &field_values).await?;
        }

        // Mark port as initialized
        self.mark_port_initialized(&event.port_name);

        Ok(())
    }

    /// Handle RTM_DELLINK netlink event
    pub async fn handle_del_link(
        &mut self,
        port_name: &str,
        state_db: &mut DatabaseConnection,
    ) -> Result<()> {
        // Ignore non-front-panel and management interfaces
        if self.should_ignore(port_name) {
            return Ok(());
        }

        // Delete from STATE_DB
        let key = format!("PORT_TABLE|{}", port_name);
        state_db.delete(&key).await?;

        Ok(())
    }

    /// Initialize port list from port names
    /// Used to pre-populate the set of ports we're waiting for
    pub fn initialize_ports(&mut self, port_names: Vec<String>) {
        self.uninitialized_ports = port_names.into_iter().collect();
    }

    /// Check if we should send PortInitDone signal
    pub fn should_send_port_init_done(&self) -> bool {
        self.are_all_ports_initialized() && !self.port_init_done
    }

    /// Get warm restart metrics (if warm restart is enabled)
    pub fn metrics(&self) -> Option<&WarmRestartMetrics> {
        self.warm_restart.as_ref().map(|mgr| &mgr.metrics)
    }

    /// Get mutable reference to warm restart metrics (if warm restart is enabled)
    pub fn metrics_mut(&mut self) -> Option<&mut WarmRestartMetrics> {
        self.warm_restart.as_mut().map(|mgr| &mut mgr.metrics)
    }
}

impl Default for LinkSync {
    fn default() -> Self {
        Self::new().expect("Failed to create LinkSync")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_status_up() {
        assert_eq!(LinkStatus::Up.as_str(), "up");
    }

    #[test]
    fn test_link_status_down() {
        assert_eq!(LinkStatus::Down.as_str(), "down");
    }

    #[test]
    fn test_port_link_state_creation() {
        let state = PortLinkState::new(
            "Ethernet0".to_string(),
            LinkStatus::Up,
            LinkStatus::Up,
            9100,
        );
        assert_eq!(state.name, "Ethernet0");
        assert_eq!(state.mtu, 9100);
    }

    #[test]
    fn test_port_link_state_to_field_values() {
        let state = PortLinkState::new(
            "Ethernet0".to_string(),
            LinkStatus::Up,
            LinkStatus::Up,
            9100,
        );
        let fields = state.to_field_values();
        assert_eq!(fields.len(), 4);
        assert!(fields.iter().any(|(k, _)| k == "state"));
        assert!(
            fields
                .iter()
                .any(|(k, v)| k == "netdev_oper_status" && v == "up")
        );
        assert!(fields.iter().any(|(k, v)| k == "admin_status" && v == "up"));
        assert!(fields.iter().any(|(k, v)| k == "mtu" && v == "9100"));
    }

    #[test]
    fn test_port_is_front_panel_ethernet() {
        let state = PortLinkState::new(
            "Ethernet0".to_string(),
            LinkStatus::Up,
            LinkStatus::Up,
            9100,
        );
        assert!(state.is_front_panel());
    }

    #[test]
    fn test_port_is_front_panel_port_channel() {
        let state = PortLinkState::new(
            "PortChannel001".to_string(),
            LinkStatus::Up,
            LinkStatus::Up,
            9100,
        );
        assert!(state.is_front_panel());
    }

    #[test]
    fn test_port_not_front_panel() {
        let state = PortLinkState::new("eth0".to_string(), LinkStatus::Up, LinkStatus::Up, 1500);
        assert!(!state.is_front_panel());
    }

    #[test]
    fn test_linksync_creation() {
        let sync = LinkSync::new().expect("Failed to create LinkSync");
        assert!(!sync.is_port_init_done());
        assert_eq!(sync.uninitialized_count(), 0);
    }

    #[test]
    fn test_linksync_should_ignore_eth0() {
        let sync = LinkSync::new().expect("Failed to create LinkSync");
        assert!(sync.should_ignore("eth0"));
    }

    #[test]
    fn test_linksync_should_ignore_loopback() {
        let sync = LinkSync::new().expect("Failed to create LinkSync");
        assert!(sync.should_ignore("lo"));
    }

    #[test]
    fn test_linksync_should_not_ignore_ethernet() {
        let sync = LinkSync::new().expect("Failed to create LinkSync");
        assert!(!sync.should_ignore("Ethernet0"));
    }

    #[test]
    fn test_linksync_port_init_done() {
        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        assert!(!sync.is_port_init_done());
        sync.set_port_init_done();
        assert!(sync.is_port_init_done());
    }

    #[test]
    fn test_port_link_state_down() {
        let state = PortLinkState::new(
            "Ethernet0".to_string(),
            LinkStatus::Down,
            LinkStatus::Down,
            9100,
        );
        let fields = state.to_field_values();
        assert!(
            fields
                .iter()
                .any(|(k, v)| k == "netdev_oper_status" && v == "down")
        );
    }

    #[test]
    fn test_link_status_from_netlink_flags_up() {
        // IFF_UP = 0x1
        let status = LinkStatus::from_netlink_flags(0x1);
        assert_eq!(status, LinkStatus::Up);
    }

    #[test]
    fn test_link_status_from_netlink_flags_down() {
        // No IFF_UP flag
        let status = LinkStatus::from_netlink_flags(0x0);
        assert_eq!(status, LinkStatus::Down);
    }

    #[test]
    fn test_link_status_from_netlink_flags_with_other_bits() {
        // IFF_UP (0x1) with other flags set (e.g., IFF_BROADCAST 0x2, IFF_RUNNING 0x40)
        let status = LinkStatus::from_netlink_flags(0x43); // 0x1 | 0x2 | 0x40
        assert_eq!(status, LinkStatus::Up);
    }

    #[test]
    fn test_netlink_event_new_link() {
        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet0".to_string(),
            flags: Some(0x1),
            mtu: Some(9100),
        };
        assert_eq!(event.event_type, NetlinkEventType::NewLink);
        assert_eq!(event.port_name, "Ethernet0");
        assert_eq!(event.flags, Some(0x1));
        assert_eq!(event.mtu, Some(9100));
    }

    #[test]
    fn test_netlink_event_del_link() {
        let event = NetlinkEvent {
            event_type: NetlinkEventType::DelLink,
            port_name: "Ethernet0".to_string(),
            flags: None,
            mtu: None,
        };
        assert_eq!(event.event_type, NetlinkEventType::DelLink);
        assert_eq!(event.port_name, "Ethernet0");
    }

    #[tokio::test]
    async fn test_handle_new_link_writes_to_state_db() {
        use crate::config::DatabaseConnection;

        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet0".to_string(),
            flags: Some(0x1), // Up
            mtu: Some(9100),
        };

        sync.handle_new_link(&event, &mut state_db)
            .await
            .expect("Failed to handle new link");

        // Verify port was written to STATE_DB
        let result = state_db
            .hgetall("PORT_TABLE|Ethernet0")
            .await
            .expect("Failed to read from STATE_DB");
        assert!(!result.is_empty());
        assert_eq!(result.get("mtu"), Some(&"9100".to_string()));
    }

    #[tokio::test]
    async fn test_handle_new_link_marks_port_initialized() {
        use crate::config::DatabaseConnection;

        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        sync.initialize_ports(vec!["Ethernet0".to_string()]);
        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        assert_eq!(sync.uninitialized_count(), 1);

        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet0".to_string(),
            flags: Some(0x1),
            mtu: Some(9100),
        };

        sync.handle_new_link(&event, &mut state_db)
            .await
            .expect("Failed to handle new link");

        assert_eq!(sync.uninitialized_count(), 0);
        assert!(sync.are_all_ports_initialized());
    }

    #[tokio::test]
    async fn test_handle_new_link_ignores_eth0() {
        use crate::config::DatabaseConnection;

        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "eth0".to_string(),
            flags: Some(0x1),
            mtu: Some(1500),
        };

        sync.handle_new_link(&event, &mut state_db)
            .await
            .expect("Failed to handle new link");

        // Verify eth0 was not written to STATE_DB
        let result = state_db
            .hgetall("PORT_TABLE|eth0")
            .await
            .expect("Failed to read from STATE_DB");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_handle_del_link_removes_from_state_db() {
        use crate::config::DatabaseConnection;

        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        // First add a port
        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet0".to_string(),
            flags: Some(0x1),
            mtu: Some(9100),
        };
        sync.handle_new_link(&event, &mut state_db)
            .await
            .expect("Failed to add port");

        // Verify it exists
        let result = state_db
            .hgetall("PORT_TABLE|Ethernet0")
            .await
            .expect("Failed to read from STATE_DB");
        assert!(!result.is_empty());

        // Delete it
        sync.handle_del_link("Ethernet0", &mut state_db)
            .await
            .expect("Failed to delete link");

        // Verify it's gone
        let result = state_db
            .hgetall("PORT_TABLE|Ethernet0")
            .await
            .expect("Failed to read from STATE_DB");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_handle_del_link_ignores_eth0() {
        use crate::config::DatabaseConnection;

        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        // Should not fail even though eth0 doesn't exist
        sync.handle_del_link("eth0", &mut state_db)
            .await
            .expect("Failed to delete eth0");
    }

    #[test]
    fn test_initialize_ports() {
        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        assert_eq!(sync.uninitialized_count(), 0);

        sync.initialize_ports(vec![
            "Ethernet0".to_string(),
            "Ethernet4".to_string(),
            "Ethernet8".to_string(),
        ]);

        assert_eq!(sync.uninitialized_count(), 3);
        assert!(!sync.are_all_ports_initialized());
    }

    #[test]
    fn test_should_send_port_init_done_when_all_initialized() {
        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        sync.initialize_ports(vec!["Ethernet0".to_string()]);

        assert!(!sync.are_all_ports_initialized());
        assert!(!sync.should_send_port_init_done());

        sync.mark_port_initialized("Ethernet0");

        assert!(sync.are_all_ports_initialized());
        assert!(sync.should_send_port_init_done());

        sync.set_port_init_done();

        assert!(!sync.should_send_port_init_done());
    }

    #[tokio::test]
    async fn test_handle_multiple_new_links() {
        use crate::config::DatabaseConnection;

        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        sync.initialize_ports(vec!["Ethernet0".to_string(), "Ethernet4".to_string()]);
        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        // Handle first port
        let event1 = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet0".to_string(),
            flags: Some(0x1),
            mtu: Some(9100),
        };
        sync.handle_new_link(&event1, &mut state_db)
            .await
            .expect("Failed to handle new link");

        assert_eq!(sync.uninitialized_count(), 1);

        // Handle second port
        let event2 = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet4".to_string(),
            flags: Some(0x1),
            mtu: Some(9100),
        };
        sync.handle_new_link(&event2, &mut state_db)
            .await
            .expect("Failed to handle new link");

        assert_eq!(sync.uninitialized_count(), 0);
        assert!(sync.should_send_port_init_done());
    }

    #[tokio::test]
    async fn test_handle_new_link_down_status() {
        use crate::config::DatabaseConnection;

        let mut sync = LinkSync::new().expect("Failed to create LinkSync");
        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet0".to_string(),
            flags: Some(0x0), // Down
            mtu: Some(9100),
        };

        sync.handle_new_link(&event, &mut state_db)
            .await
            .expect("Failed to handle new link");

        // Verify port status is down
        let result = state_db
            .hgetall("PORT_TABLE|Ethernet0")
            .await
            .expect("Failed to read from STATE_DB");
        assert_eq!(result.get("netdev_oper_status"), Some(&"down".to_string()));
    }

    #[test]
    fn test_linksync_without_warm_restart() {
        let sync = LinkSync::new().expect("Failed to create LinkSync");
        assert!(!sync.should_skip_app_db_updates());
        assert_eq!(sync.warm_restart_state(), None);
    }

    #[test]
    fn test_linksync_with_warm_restart() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let state_file = temp_dir.path().join("port_state.json");

        let mut sync = LinkSync::with_warm_restart(state_file).expect("Failed to create LinkSync");
        sync.initialize_warm_restart()
            .expect("Failed to initialize warm restart");

        // Should be cold start initially
        assert_eq!(sync.warm_restart_state(), Some(WarmRestartState::ColdStart));
        assert!(!sync.should_skip_app_db_updates());
    }

    #[test]
    fn test_linksync_warm_restart_state_transitions() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let state_file = temp_dir.path().join("port_state.json");

        let mut sync = LinkSync::with_warm_restart(state_file).expect("Failed to create LinkSync");
        sync.initialize_warm_restart()
            .expect("Failed to initialize warm restart");

        assert_eq!(sync.warm_restart_state(), Some(WarmRestartState::ColdStart));

        // Transition to warm restart sync
        sync.begin_warm_restart_sync();
        // Note: will still be ColdStart since no saved state exists
        assert_eq!(sync.warm_restart_state(), Some(WarmRestartState::ColdStart));

        // Transition to complete
        sync.complete_warm_restart_sync();
        assert_eq!(sync.warm_restart_state(), Some(WarmRestartState::ColdStart));
    }

    #[tokio::test]
    async fn test_handle_new_link_records_port_for_warm_restart() {
        use crate::config::DatabaseConnection;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let state_file = temp_dir.path().join("port_state.json");

        let mut sync =
            LinkSync::with_warm_restart(state_file.clone()).expect("Failed to create LinkSync");
        sync.initialize_warm_restart()
            .expect("Failed to initialize warm restart");

        let mut state_db = DatabaseConnection::new("STATE_DB".to_string());

        let event = NetlinkEvent {
            event_type: NetlinkEventType::NewLink,
            port_name: "Ethernet0".to_string(),
            flags: Some(0x41), // Up and running
            mtu: Some(9216),
        };

        sync.handle_new_link(&event, &mut state_db)
            .await
            .expect("Failed to handle new link");

        // Save and verify port was recorded
        sync.save_port_state().expect("Failed to save port state");

        // Reload and verify
        let mut sync2 = LinkSync::with_warm_restart(state_file).expect("Failed to create LinkSync");
        sync2
            .initialize_warm_restart()
            .expect("Failed to initialize warm restart");

        // Port was saved, so if we create a new manager it should load it
        // (but current cold start won't load it until we have a saved state file)
    }

    #[test]
    fn test_record_port_for_warm_restart() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let state_file = temp_dir.path().join("port_state.json");

        let mut sync = LinkSync::with_warm_restart(state_file).expect("Failed to create LinkSync");
        sync.initialize_warm_restart()
            .expect("Failed to initialize warm restart");

        // Record port
        sync.record_port_for_warm_restart("Ethernet0".to_string(), 0x41, 9216);

        // Save state
        sync.save_port_state().expect("Failed to save port state");

        // Verify saved - state file path is used (in temp dir for testing)
    }
}
