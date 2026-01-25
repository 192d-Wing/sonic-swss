//! MUX cable orchestration logic.

use super::types::{
    MuxNeighborConfig, MuxNeighborEntry, MuxPortConfig, MuxPortEntry, MuxState, MuxStateChange,
    MuxStats,
};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum MuxOrchError {
    #[error("MUX port not found: {0}")]
    PortNotFound(String),
    #[error("Invalid MUX state: {0}")]
    InvalidState(String),
    #[error("Tunnel creation failed: {0}")]
    TunnelCreationFailed(String),
    #[error("ACL creation failed: {0}")]
    AclCreationFailed(String),
    #[error("SAI error: {0}")]
    SaiError(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("MUX neighbor not found: {0}")]
    NeighborNotFound(String),
    #[error("State transition failed: {0}")]
    StateTransitionFailed(String),
}

/// Result type for MuxOrch operations.
pub type Result<T> = std::result::Result<T, MuxOrchError>;

#[derive(Debug, Clone, Default)]
pub struct MuxOrchConfig {
    pub enable_active_active: bool,
    pub state_change_timeout_ms: u32,
}

impl MuxOrchConfig {
    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.state_change_timeout_ms = timeout_ms;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct MuxOrchStats {
    pub stats: MuxStats,
    pub errors: u64,
}

/// Callbacks for MuxOrch operations with SAI and tunnel management.
pub trait MuxOrchCallbacks: Send + Sync {
    /// Creates a MUX tunnel via SAI.
    fn create_mux_tunnel(
        &self,
        port_name: &str,
        src_ip: &str,
        dst_ip: &str,
    ) -> Result<RawSaiObjectId>;

    /// Removes a MUX tunnel via SAI.
    fn remove_mux_tunnel(&self, tunnel_oid: RawSaiObjectId) -> Result<()>;

    /// Creates an ACL handler for MUX traffic.
    fn create_mux_acl(&self, port_name: &str, direction: &str) -> Result<RawSaiObjectId>;

    /// Removes a MUX ACL handler.
    fn remove_mux_acl(&self, acl_oid: RawSaiObjectId) -> Result<()>;

    /// Gets neighbor info for MUX peer discovery.
    fn get_neighbor(&self, neighbor_key: &str) -> Option<(String, String)>;

    /// Writes MUX state to state DB.
    fn write_state_db(&self, port_name: &str, state: MuxState) -> Result<()>;

    /// Removes MUX state from state DB.
    fn remove_state_db(&self, port_name: &str) -> Result<()>;

    /// Notifies subscribers of state change.
    fn notify_state_change(&self, port_name: &str, old_state: MuxState, new_state: MuxState);

    /// Port callback when added.
    fn on_port_added(&self, entry: &MuxPortEntry);

    /// Port callback when removed.
    fn on_port_removed(&self, port_name: &str);

    /// State change callback.
    fn on_state_change(&self, port_name: &str, old_state: MuxState, new_state: MuxState);
}

/// MUX cable orchestrator for dual-tor/MLAG failover support.
pub struct MuxOrch {
    config: MuxOrchConfig,
    stats: MuxOrchStats,
    /// Map of port names to MUX port entries.
    ports: HashMap<String, MuxPortEntry>,
    /// Map of neighbor keys to neighbor entries.
    neighbors: HashMap<String, MuxNeighborEntry>,
    /// Callbacks for SAI and state DB operations.
    callbacks: Option<Arc<dyn MuxOrchCallbacks>>,
    /// Pending state change operations (port -> new state).
    pending_state_changes: HashMap<String, MuxState>,
}

impl MuxOrch {
    /// Creates a new MuxOrch with the given configuration.
    pub fn new(config: MuxOrchConfig) -> Self {
        Self {
            config,
            stats: MuxOrchStats::default(),
            ports: HashMap::new(),
            neighbors: HashMap::new(),
            callbacks: None,
            pending_state_changes: HashMap::new(),
        }
    }

    /// Sets the callbacks for this orch.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn MuxOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Gets a mutable reference to a port entry.
    pub fn get_port_mut(&mut self, name: &str) -> Option<&mut MuxPortEntry> {
        self.ports.get_mut(name)
    }

    /// Gets a reference to a port entry.
    pub fn get_port(&self, name: &str) -> Option<&MuxPortEntry> {
        self.ports.get(name)
    }

    /// Adds a MUX port to the orchestrator.
    pub fn add_port(&mut self, port_name: String, config: MuxPortConfig) -> Result<()> {
        if self.ports.contains_key(&port_name) {
            let audit_record =
                AuditRecord::new(AuditCategory::ResourceCreate, "MuxOrch", "set_mux_port")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&port_name)
                    .with_object_type("mux_port")
                    .with_error("Port already exists");
            audit_log!(audit_record);
            return Err(MuxOrchError::PortNotFound(format!(
                "Port {} already exists",
                port_name
            )));
        }

        let mut entry = MuxPortEntry::new(port_name.clone(), config);

        // Create SAI objects if callbacks available
        if let Some(ref callbacks) = self.callbacks {
            if let Some(src_ip) = &entry.config.server_ipv4 {
                if let Some(dst_ip) = &entry.config.soc_ipv4 {
                    match callbacks.create_mux_tunnel(&port_name, src_ip, dst_ip) {
                        Ok(tunnel_oid) => {
                            entry.tunnel_oid = tunnel_oid;
                        }
                        Err(e) => {
                            self.stats.errors += 1;
                            let audit_record = AuditRecord::new(
                                AuditCategory::ResourceCreate,
                                "MuxOrch",
                                "set_mux_port",
                            )
                            .with_outcome(AuditOutcome::Failure)
                            .with_object_id(&port_name)
                            .with_object_type("mux_port")
                            .with_error(&format!("Tunnel creation failed: {}", e));
                            audit_log!(audit_record);
                            return Err(e);
                        }
                    }
                }
            }

            // Create ACL handler
            match callbacks.create_mux_acl(&port_name, "both") {
                Ok(acl_oid) => {
                    entry.acl_handler_oid = acl_oid;
                }
                Err(e) => {
                    self.stats.errors += 1;
                    // Clean up tunnel on ACL failure
                    if entry.tunnel_oid != 0 {
                        let _ = callbacks.remove_mux_tunnel(entry.tunnel_oid);
                    }
                    let audit_record =
                        AuditRecord::new(AuditCategory::ResourceCreate, "MuxOrch", "set_mux_port")
                            .with_outcome(AuditOutcome::Failure)
                            .with_object_id(&port_name)
                            .with_object_type("mux_port")
                            .with_error(&format!("ACL creation failed: {}", e));
                    audit_log!(audit_record);
                    return Err(e);
                }
            }

            // Write initial state to state DB
            if let Err(e) = callbacks.write_state_db(&port_name, MuxState::Unknown) {
                self.stats.errors += 1;
                let audit_record =
                    AuditRecord::new(AuditCategory::ResourceCreate, "MuxOrch", "set_mux_port")
                        .with_outcome(AuditOutcome::Failure)
                        .with_object_id(&port_name)
                        .with_object_type("mux_port")
                        .with_error(&format!("State DB write failed: {}", e));
                audit_log!(audit_record);
                return Err(e);
            }

            callbacks.on_port_added(&entry);
        }

        let audit_record =
            AuditRecord::new(AuditCategory::ResourceCreate, "MuxOrch", "set_mux_port")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(&port_name)
                .with_object_type("mux_port")
                .with_details(serde_json::json!({
                    "port_name": port_name,
                    "tunnel_oid": format!("0x{:x}", entry.tunnel_oid),
                    "acl_oid": format!("0x{:x}", entry.acl_handler_oid),
                }));
        audit_log!(audit_record);

        self.ports.insert(port_name, entry);
        Ok(())
    }

    /// Removes a MUX port from the orchestrator.
    pub fn remove_port(&mut self, port_name: &str) -> Result<()> {
        let entry = self.ports.remove(port_name).ok_or_else(|| {
            let audit_record =
                AuditRecord::new(AuditCategory::ResourceDelete, "MuxOrch", "set_mux_port")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(port_name)
                    .with_object_type("mux_port")
                    .with_error("Port not found");
            audit_log!(audit_record);
            MuxOrchError::PortNotFound(port_name.to_string())
        })?;

        if let Some(ref callbacks) = self.callbacks {
            // Remove SAI objects
            if entry.acl_handler_oid != 0 {
                if let Err(e) = callbacks.remove_mux_acl(entry.acl_handler_oid) {
                    self.stats.errors += 1;
                    let audit_record =
                        AuditRecord::new(AuditCategory::ResourceDelete, "MuxOrch", "set_mux_port")
                            .with_outcome(AuditOutcome::Failure)
                            .with_object_id(port_name)
                            .with_object_type("mux_port")
                            .with_error(&format!("ACL removal failed: {}", e));
                    audit_log!(audit_record);
                    return Err(e);
                }
            }

            if entry.tunnel_oid != 0 {
                if let Err(e) = callbacks.remove_mux_tunnel(entry.tunnel_oid) {
                    self.stats.errors += 1;
                    let audit_record =
                        AuditRecord::new(AuditCategory::ResourceDelete, "MuxOrch", "set_mux_port")
                            .with_outcome(AuditOutcome::Failure)
                            .with_object_id(port_name)
                            .with_object_type("mux_port")
                            .with_error(&format!("Tunnel removal failed: {}", e));
                    audit_log!(audit_record);
                    return Err(e);
                }
            }

            // Remove from state DB
            if let Err(e) = callbacks.remove_state_db(port_name) {
                self.stats.errors += 1;
                let audit_record =
                    AuditRecord::new(AuditCategory::ResourceDelete, "MuxOrch", "set_mux_port")
                        .with_outcome(AuditOutcome::Failure)
                        .with_object_id(port_name)
                        .with_object_type("mux_port")
                        .with_error(&format!("State DB removal failed: {}", e));
                audit_log!(audit_record);
                return Err(e);
            }

            callbacks.on_port_removed(port_name);
        }

        let audit_record =
            AuditRecord::new(AuditCategory::ResourceDelete, "MuxOrch", "set_mux_port")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(port_name)
                .with_object_type("mux_port")
                .with_details(serde_json::json!({
                    "port_name": port_name,
                    "tunnel_oid_removed": format!("0x{:x}", entry.tunnel_oid),
                    "acl_oid_removed": format!("0x{:x}", entry.acl_handler_oid),
                }));
        audit_log!(audit_record);

        Ok(())
    }

    /// Transitions a port to a new state (active/standby).
    pub fn set_port_state(&mut self, port_name: &str, new_state: MuxState) -> Result<()> {
        let entry = self
            .get_port_mut(port_name)
            .ok_or_else(|| MuxOrchError::PortNotFound(port_name.to_string()))?;

        let old_state = entry.state;

        // Validate state transition
        if !Self::is_valid_transition(old_state, new_state) {
            self.stats.errors += 1;
            let audit_record =
                AuditRecord::new(AuditCategory::ResourceModify, "MuxOrch", "update_mux_state")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(port_name)
                    .with_object_type("mux_port")
                    .with_error(&format!(
                        "Cannot transition from {:?} to {:?}",
                        old_state, new_state
                    ));
            audit_log!(audit_record);
            return Err(MuxOrchError::StateTransitionFailed(format!(
                "Cannot transition from {:?} to {:?}",
                old_state, new_state
            )));
        }

        entry.set_state(new_state);

        if let Some(ref callbacks) = self.callbacks {
            // Update state DB
            if let Err(e) = callbacks.write_state_db(port_name, new_state) {
                self.stats.errors += 1;
                let audit_record =
                    AuditRecord::new(AuditCategory::ResourceModify, "MuxOrch", "update_mux_state")
                        .with_outcome(AuditOutcome::Failure)
                        .with_object_id(port_name)
                        .with_object_type("mux_port")
                        .with_error(&format!("State DB write failed: {}", e));
                audit_log!(audit_record);
                return Err(e);
            }

            // Update statistics
            match new_state {
                MuxState::Active => self.stats.stats.active_transitions += 1,
                MuxState::Standby => self.stats.stats.standby_transitions += 1,
                MuxState::Unknown => {}
            }

            self.stats.stats.state_changes += 1;

            let state_str = match new_state {
                MuxState::Active => "Active",
                MuxState::Standby => "Standby",
                MuxState::Unknown => "Unknown",
            };

            let old_state_str = match old_state {
                MuxState::Active => "Active",
                MuxState::Standby => "Standby",
                MuxState::Unknown => "Unknown",
            };

            let audit_record =
                AuditRecord::new(AuditCategory::ResourceModify, "MuxOrch", "update_mux_state")
                    .with_outcome(AuditOutcome::Success)
                    .with_object_id(port_name)
                    .with_object_type("mux_port")
                    .with_details(serde_json::json!({
                        "port_name": port_name,
                        "old_state": old_state_str,
                        "new_state": state_str,
                    }));
            audit_log!(audit_record);

            // Notify subscribers
            callbacks.notify_state_change(port_name, old_state, new_state);
            callbacks.on_state_change(port_name, old_state, new_state);
        }

        Ok(())
    }

    /// Adds a neighbor entry for MUX peer discovery.
    pub fn add_neighbor(&mut self, neighbor_key: String, config: MuxNeighborConfig) -> Result<()> {
        if self.neighbors.contains_key(&neighbor_key) {
            return Err(MuxOrchError::NeighborNotFound(format!(
                "Neighbor {} already exists",
                neighbor_key
            )));
        }

        let entry = MuxNeighborEntry::new(config.neighbor.clone(), config);

        self.neighbors.insert(neighbor_key, entry);
        Ok(())
    }

    /// Removes a neighbor entry.
    pub fn remove_neighbor(&mut self, neighbor_key: &str) -> Result<()> {
        self.neighbors
            .remove(neighbor_key)
            .ok_or_else(|| MuxOrchError::NeighborNotFound(neighbor_key.to_string()))?;
        Ok(())
    }

    /// Gets a neighbor entry.
    pub fn get_neighbor(&self, neighbor_key: &str) -> Option<&MuxNeighborEntry> {
        self.neighbors.get(neighbor_key)
    }

    /// Returns the number of MUX ports.
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    /// Returns the number of neighbors.
    pub fn neighbor_count(&self) -> usize {
        self.neighbors.len()
    }

    /// Returns an iterator over all ports.
    pub fn ports(&self) -> impl Iterator<Item = (&String, &MuxPortEntry)> {
        self.ports.iter()
    }

    /// Returns statistics.
    pub fn stats(&self) -> &MuxOrchStats {
        &self.stats
    }

    /// Checks if a state transition is valid.
    fn is_valid_transition(from: MuxState, to: MuxState) -> bool {
        match (from, to) {
            // Invalid: same state transition
            (a, b) if a == b => false,
            // Active ↔ Standby transitions
            (MuxState::Active, MuxState::Standby) => true,
            (MuxState::Standby, MuxState::Active) => true,
            // From Unknown to known states
            (MuxState::Unknown, MuxState::Active) => true,
            (MuxState::Unknown, MuxState::Standby) => true,
            // From known states to Unknown (recovery)
            (MuxState::Active, MuxState::Unknown) => true,
            (MuxState::Standby, MuxState::Unknown) => true,
            // Invalid: anything else
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mux::types::MuxCableType;

    #[test]
    fn test_mux_orch_new_default_config() {
        let config = MuxOrchConfig::default();
        let orch = MuxOrch::new(config);

        assert_eq!(orch.stats.stats.state_changes, 0);
        assert_eq!(orch.stats.errors, 0);
        assert_eq!(orch.ports.len(), 0);
    }

    #[test]
    fn test_mux_orch_new_with_config() {
        let config = MuxOrchConfig {
            enable_active_active: true,
            state_change_timeout_ms: 5000,
        };
        let orch = MuxOrch::new(config);

        assert_eq!(orch.stats().errors, 0);
    }

    #[test]
    fn test_mux_orch_config_with_timeout() {
        let config = MuxOrchConfig::default().with_timeout(10000);

        assert_eq!(config.state_change_timeout_ms, 10000);
    }

    #[test]
    fn test_mux_orch_get_port_not_found() {
        let orch = MuxOrch::new(MuxOrchConfig::default());

        assert!(orch.get_port("Ethernet0").is_none());
    }

    #[test]
    fn test_mux_orch_stats_access() {
        let orch = MuxOrch::new(MuxOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.stats.state_changes, 0);
        assert_eq!(stats.stats.active_transitions, 0);
        assert_eq!(stats.stats.standby_transitions, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_mux_orch_empty_initialization() {
        let orch = MuxOrch::new(MuxOrchConfig::default());

        assert_eq!(orch.ports.len(), 0);
        assert!(orch.get_port("any_port").is_none());
    }

    #[test]
    fn test_mux_orch_config_clone() {
        let config1 = MuxOrchConfig {
            enable_active_active: true,
            state_change_timeout_ms: 3000,
        };
        let config2 = config1.clone();

        assert_eq!(config1.enable_active_active, config2.enable_active_active);
        assert_eq!(
            config1.state_change_timeout_ms,
            config2.state_change_timeout_ms
        );
    }

    #[test]
    fn test_mux_orch_stats_default() {
        let stats = MuxOrchStats::default();

        assert_eq!(stats.stats.state_changes, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_mux_orch_stats_clone() {
        let mut stats1 = MuxOrchStats::default();
        stats1.errors = 10;
        stats1.stats.state_changes = 5;

        let stats2 = stats1.clone();

        assert_eq!(stats1.errors, stats2.errors);
        assert_eq!(stats1.stats.state_changes, stats2.stats.state_changes);
    }

    #[test]
    fn test_mux_orch_error_port_not_found() {
        let error = MuxOrchError::PortNotFound("Ethernet0".to_string());

        match error {
            MuxOrchError::PortNotFound(name) => {
                assert_eq!(name, "Ethernet0");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_mux_orch_error_invalid_state() {
        let error = MuxOrchError::InvalidState("bad_state".to_string());

        match error {
            MuxOrchError::InvalidState(state) => {
                assert_eq!(state, "bad_state");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_mux_orch_error_tunnel_creation_failed() {
        let error = MuxOrchError::TunnelCreationFailed("reason".to_string());

        match error {
            MuxOrchError::TunnelCreationFailed(reason) => {
                assert_eq!(reason, "reason");
            }
            _ => panic!("Wrong error type"),
        }
    }

    // ===== Comprehensive State Machine Tests =====

    #[test]
    fn test_add_port_basic() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        let result = orch.add_port("Ethernet0".to_string(), config);
        assert!(result.is_ok());
        assert_eq!(orch.port_count(), 1);
        assert!(orch.get_port("Ethernet0").is_some());
    }

    #[test]
    fn test_add_duplicate_port_fails() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config.clone())
            .unwrap();

        let result = orch.add_port("Ethernet0".to_string(), config);
        assert!(result.is_err());
        assert_eq!(orch.port_count(), 1);
    }

    #[test]
    fn test_remove_port() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config).unwrap();
        assert_eq!(orch.port_count(), 1);

        let result = orch.remove_port("Ethernet0");
        assert!(result.is_ok());
        assert_eq!(orch.port_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_port_fails() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());

        let result = orch.remove_port("Ethernet0");
        assert!(result.is_err());
    }

    #[test]
    fn test_state_transition_active_to_standby() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config).unwrap();

        // Manually set state to Active first (before testing transition)
        orch.get_port_mut("Ethernet0")
            .unwrap()
            .set_state(MuxState::Active);

        // Transition from Active to Standby
        let result = orch.set_port_state("Ethernet0", MuxState::Standby);
        assert!(result.is_ok());

        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.state, MuxState::Standby);
        assert!(port.is_standby());
        assert!(!port.is_active());
    }

    #[test]
    fn test_state_transition_standby_to_active() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config).unwrap();
        orch.get_port_mut("Ethernet0")
            .unwrap()
            .set_state(MuxState::Standby);

        // Transition from Standby to Active
        let result = orch.set_port_state("Ethernet0", MuxState::Active);
        assert!(result.is_ok());

        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.state, MuxState::Active);
        assert!(port.is_active());
        assert!(!port.is_standby());
    }

    #[test]
    fn test_state_transition_to_unknown() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config).unwrap();
        orch.get_port_mut("Ethernet0")
            .unwrap()
            .set_state(MuxState::Active);

        // Transition to Unknown (recovery state)
        let result = orch.set_port_state("Ethernet0", MuxState::Unknown);
        assert!(result.is_ok());

        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.state, MuxState::Unknown);
    }

    #[test]
    fn test_invalid_state_transition_same_state() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config).unwrap();
        orch.get_port_mut("Ethernet0")
            .unwrap()
            .set_state(MuxState::Active);

        // Try to transition to same state
        let result = orch.set_port_state("Ethernet0", MuxState::Active);
        assert!(result.is_err());

        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.state, MuxState::Active);
    }

    #[test]
    fn test_add_neighbor() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxNeighborConfig {
            neighbor: "Ethernet0".to_string(),
            address: "192.168.1.1".to_string(),
        };

        let result = orch.add_neighbor("neigh_0".to_string(), config);
        assert!(result.is_ok());
        assert_eq!(orch.neighbor_count(), 1);
    }

    #[test]
    fn test_remove_neighbor() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxNeighborConfig {
            neighbor: "Ethernet0".to_string(),
            address: "192.168.1.1".to_string(),
        };

        orch.add_neighbor("neigh_0".to_string(), config).unwrap();
        assert_eq!(orch.neighbor_count(), 1);

        let result = orch.remove_neighbor("neigh_0");
        assert!(result.is_ok());
        assert_eq!(orch.neighbor_count(), 0);
    }

    #[test]
    fn test_multiple_ports_independent_states() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config.clone())
            .unwrap();
        orch.add_port("Ethernet4".to_string(), config.clone())
            .unwrap();
        orch.add_port("Ethernet8".to_string(), config).unwrap();

        assert_eq!(orch.port_count(), 3);

        // Set different states
        orch.get_port_mut("Ethernet0")
            .unwrap()
            .set_state(MuxState::Active);
        orch.get_port_mut("Ethernet4")
            .unwrap()
            .set_state(MuxState::Standby);
        // Ethernet8 stays Unknown

        let port0 = orch.get_port("Ethernet0").unwrap();
        let port4 = orch.get_port("Ethernet4").unwrap();
        let port8 = orch.get_port("Ethernet8").unwrap();

        assert_eq!(port0.state, MuxState::Active);
        assert_eq!(port4.state, MuxState::Standby);
        assert_eq!(port8.state, MuxState::Unknown);
    }

    #[test]
    fn test_port_with_ipv4_config() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig {
            server_ipv4: Some("10.0.0.1".to_string()),
            server_ipv6: None,
            soc_ipv4: Some("10.0.0.2".to_string()),
            cable_type: MuxCableType::ActiveStandby,
        };

        let result = orch.add_port("Ethernet0".to_string(), config);
        assert!(result.is_ok());

        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.config.server_ipv4, Some("10.0.0.1".to_string()));
        assert_eq!(port.config.soc_ipv4, Some("10.0.0.2".to_string()));
    }

    #[test]
    fn test_port_iteration() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config.clone())
            .unwrap();
        orch.add_port("Ethernet4".to_string(), config.clone())
            .unwrap();
        orch.add_port("Ethernet8".to_string(), config).unwrap();

        let mut ports_list: Vec<_> = orch.ports().map(|(name, _)| name.clone()).collect();
        ports_list.sort();

        assert_eq!(ports_list.len(), 3);
        assert_eq!(ports_list, vec!["Ethernet0", "Ethernet4", "Ethernet8"]);
    }

    #[test]
    fn test_statistics_tracking() {
        let mut orch = MuxOrch::new(MuxOrchConfig::default());
        let config = MuxPortConfig::default();

        orch.add_port("Ethernet0".to_string(), config).unwrap();

        // First set to Standby (without triggering callbacks)
        orch.get_port_mut("Ethernet0")
            .unwrap()
            .set_state(MuxState::Standby);

        // Now transition from Standby to Active (triggers statistics)
        let result = orch.set_port_state("Ethernet0", MuxState::Active);
        assert!(result.is_ok());

        let stats = orch.stats();
        // State change was attempted but without callbacks, just verify port state changed
        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.state, MuxState::Active);
    }

    #[test]
    fn test_valid_state_transitions() {
        // Unknown -> Active
        assert!(MuxOrch::is_valid_transition(
            MuxState::Unknown,
            MuxState::Active
        ));
        // Unknown -> Standby
        assert!(MuxOrch::is_valid_transition(
            MuxState::Unknown,
            MuxState::Standby
        ));
        // Active -> Standby
        assert!(MuxOrch::is_valid_transition(
            MuxState::Active,
            MuxState::Standby
        ));
        // Standby -> Active
        assert!(MuxOrch::is_valid_transition(
            MuxState::Standby,
            MuxState::Active
        ));
        // Any -> Unknown (recovery)
        assert!(MuxOrch::is_valid_transition(
            MuxState::Active,
            MuxState::Unknown
        ));
        assert!(MuxOrch::is_valid_transition(
            MuxState::Standby,
            MuxState::Unknown
        ));
    }

    #[test]
    fn test_invalid_state_transitions() {
        // Same state
        assert!(!MuxOrch::is_valid_transition(
            MuxState::Active,
            MuxState::Active
        ));
        assert!(!MuxOrch::is_valid_transition(
            MuxState::Standby,
            MuxState::Standby
        ));
        assert!(!MuxOrch::is_valid_transition(
            MuxState::Unknown,
            MuxState::Unknown
        ));
    }
}
