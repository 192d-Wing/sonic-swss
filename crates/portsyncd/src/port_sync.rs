//! Port status synchronization via netlink events
//!
//! Handles RTM_NEWLINK and RTM_DELLINK netlink events and updates STATE_DB
//! with the current operational status of ports.

use crate::error::Result;
use std::collections::HashSet;

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

/// Port synchronization daemon state
/// (Stub - will be fully implemented in Day 3)
pub struct LinkSync {
    /// Uninitialized ports awaiting their first netlink event
    uninitialized_ports: HashSet<String>,
    /// Flag: have we sent PortInitDone yet?
    port_init_done: bool,
}

impl LinkSync {
    /// Create new LinkSync daemon
    pub fn new() -> Result<Self> {
        Ok(Self {
            uninitialized_ports: HashSet::new(),
            port_init_done: false,
        })
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
        assert!(fields.iter().any(|(k, v)| k == "netdev_oper_status" && v == "up"));
        assert!(fields.iter().any(|(k, v)| k == "admin_status" && v == "up"));
        assert!(fields.iter().any(|(k, v)| k == "mtu" && v == "9100"));
    }

    #[test]
    fn test_port_is_front_panel_ethernet() {
        let state = PortLinkState::new("Ethernet0".to_string(), LinkStatus::Up, LinkStatus::Up, 9100);
        assert!(state.is_front_panel());
    }

    #[test]
    fn test_port_is_front_panel_port_channel() {
        let state = PortLinkState::new("PortChannel001".to_string(), LinkStatus::Up, LinkStatus::Up, 9100);
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
        assert!(fields
            .iter()
            .find(|(k, v)| k == "netdev_oper_status" && v == "down")
            .is_some());
    }
}
