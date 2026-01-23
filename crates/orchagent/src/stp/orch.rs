//! STP orchestration logic.

use super::types::{SaiStpPortState, StpInstanceEntry, StpPortIds, StpState};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};

/// STP orchestrator error types.
#[derive(Debug, Clone, thiserror::Error)]
pub enum StpOrchError {
    #[error("Invalid instance: {0}")]
    InvalidInstance(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Invalid port: {0}")]
    InvalidPort(String),
    #[error("Port not ready")]
    PortNotReady,
    #[error("VLAN not found: {0}")]
    VlanNotFound(String),
    #[error("SAI error: {0}")]
    SaiError(String),
    #[error("Instance not found: {0}")]
    InstanceNotFound(u16),
    #[error("STP port not found: {0}")]
    StpPortNotFound(u16),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// STP orchestrator configuration.
#[derive(Debug, Clone, Default)]
pub struct StpOrchConfig {
    pub enable_state_db: bool,
}

/// STP orchestrator statistics.
#[derive(Debug, Clone, Default)]
pub struct StpOrchStats {
    pub instances_created: u64,
    pub instances_removed: u64,
    pub ports_created: u64,
    pub ports_removed: u64,
    pub state_updates: u64,
    pub fdb_flushes: u64,
}

/// Callbacks for STP operations.
pub trait StpOrchCallbacks: Send + Sync {
    fn all_ports_ready(&self) -> bool;
    fn get_port_bridge_port_id(&self, alias: &str) -> Option<RawSaiObjectId>;
    fn create_stp_instance(&self) -> Result<RawSaiObjectId, String>;
    fn remove_stp_instance(&self, oid: RawSaiObjectId) -> Result<(), String>;
    fn set_vlan_stp_instance(&self, vlan_alias: &str, stp_inst_oid: RawSaiObjectId) -> Result<(), String>;
    fn create_stp_port(&self, bridge_port_id: RawSaiObjectId, stp_inst_oid: RawSaiObjectId, state: SaiStpPortState) -> Result<RawSaiObjectId, String>;
    fn remove_stp_port(&self, stp_port_oid: RawSaiObjectId) -> Result<(), String>;
    fn set_stp_port_state(&self, stp_port_oid: RawSaiObjectId, state: SaiStpPortState) -> Result<(), String>;
    fn flush_fdb_by_vlan(&self, vlan_alias: &str) -> Result<(), String>;
    fn ensure_bridge_port(&self, port_alias: &str) -> Result<RawSaiObjectId, String>;
}

/// STP orchestrator.
pub struct StpOrch {
    config: StpOrchConfig,
    stats: StpOrchStats,
    callbacks: Option<Arc<dyn StpOrchCallbacks>>,

    /// Map: instance ID → SAI STP OID
    stp_inst_to_oid: HashMap<u16, RawSaiObjectId>,
    /// Map: instance ID → StpInstanceEntry
    vlan_to_instance_map: HashMap<u16, StpInstanceEntry>,
    /// Default STP instance OID
    default_stp_id: RawSaiObjectId,
    /// Maximum STP instances supported
    max_stp_instance: u16,
}

impl StpOrch {
    /// Creates a new STP orchestrator.
    pub fn new(config: StpOrchConfig) -> Self {
        Self {
            config,
            stats: StpOrchStats::default(),
            callbacks: None,
            stp_inst_to_oid: HashMap::new(),
            vlan_to_instance_map: HashMap::new(),
            default_stp_id: 0,
            max_stp_instance: 0,
        }
    }

    /// Sets callbacks.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn StpOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Initializes with default STP instance and max instances.
    pub fn initialize(&mut self, default_stp_id: RawSaiObjectId, max_stp_instance: u16) {
        self.default_stp_id = default_stp_id;
        self.max_stp_instance = max_stp_instance;
        self.stp_inst_to_oid.insert(0, default_stp_id);
    }

    /// Gets STP instance OID.
    pub fn get_instance_oid(&self, instance: u16) -> Option<RawSaiObjectId> {
        self.stp_inst_to_oid.get(&instance).copied()
    }

    /// Adds STP instance.
    pub fn add_instance(&mut self, instance: u16) -> Result<RawSaiObjectId, StpOrchError> {
        if instance >= self.max_stp_instance {
            let error = StpOrchError::InvalidInstance(format!(
                "Instance {} exceeds max {}",
                instance, self.max_stp_instance
            ));
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "StpOrch",
                "create_stp_instance"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(format!("stp_{}", instance))
            .with_object_type("stp_instance")
            .with_error(error.to_string()));
            return Err(error);
        }

        if let Some(oid) = self.stp_inst_to_oid.get(&instance) {
            return Ok(*oid);
        }

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        let stp_oid = callbacks.create_stp_instance()
            .map_err(StpOrchError::SaiError)?;

        self.stp_inst_to_oid.insert(instance, stp_oid);
        self.stats.instances_created += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "StpOrch",
            "create_stp_instance"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("stp_{}", instance))
        .with_object_type("stp_instance")
        .with_details(serde_json::json!({
            "instance": instance,
            "oid": stp_oid,
            "stats": {
                "instances_created": self.stats.instances_created
            }
        })));

        Ok(stp_oid)
    }

    /// Removes STP instance.
    pub fn remove_instance(&mut self, instance: u16) -> Result<(), StpOrchError> {
        let stp_oid = self.stp_inst_to_oid.get(&instance)
            .copied()
            .ok_or(StpOrchError::InstanceNotFound(instance))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        callbacks.remove_stp_instance(stp_oid)
            .map_err(StpOrchError::SaiError)?;

        self.stp_inst_to_oid.remove(&instance);
        self.vlan_to_instance_map.remove(&instance);
        self.stats.instances_removed += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "StpOrch",
            "remove_stp_instance"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("stp_{}", instance))
        .with_object_type("stp_instance")
        .with_details(serde_json::json!({
            "instance": instance,
            "oid": stp_oid,
            "stats": {
                "instances_removed": self.stats.instances_removed
            }
        })));

        Ok(())
    }

    /// Adds VLAN to STP instance.
    pub fn add_vlan_to_instance(&mut self, vlan_alias: &str, instance: u16) -> Result<(), StpOrchError> {
        // Lazy-create instance if needed
        let stp_inst_oid = if let Some(oid) = self.get_instance_oid(instance) {
            oid
        } else {
            self.add_instance(instance)?
        };

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        // Set VLAN attribute
        callbacks.set_vlan_stp_instance(vlan_alias, stp_inst_oid)
            .map_err(StpOrchError::SaiError)?;

        // Track VLAN in instance
        self.vlan_to_instance_map
            .entry(instance)
            .or_insert_with(|| StpInstanceEntry::new(stp_inst_oid))
            .add_vlan(vlan_alias.to_string());

        Ok(())
    }

    /// Removes VLAN from STP instance.
    pub fn remove_vlan_from_instance(&mut self, vlan_alias: &str, instance: u16) -> Result<(), StpOrchError> {
        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        // Reset to default instance
        callbacks.set_vlan_stp_instance(vlan_alias, self.default_stp_id)
            .map_err(StpOrchError::SaiError)?;

        // Remove from tracking
        if let Some(entry) = self.vlan_to_instance_map.get_mut(&instance) {
            entry.remove_vlan(vlan_alias);
        }

        Ok(())
    }

    /// Adds STP port.
    pub fn add_stp_port(
        &mut self,
        port_alias: &str,
        instance: u16,
        stp_port_ids: &mut StpPortIds,
    ) -> Result<RawSaiObjectId, StpOrchError> {
        // Check if already exists
        if let Some(existing) = stp_port_ids.get(&instance) {
            return Ok(*existing);
        }

        let stp_inst_oid = self.get_instance_oid(instance)
            .ok_or(StpOrchError::InstanceNotFound(instance))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        // Ensure bridge port exists
        let bridge_port_id = callbacks.ensure_bridge_port(port_alias)
            .map_err(StpOrchError::SaiError)?;

        // Create STP port with blocking state
        let stp_port_oid = callbacks.create_stp_port(
            bridge_port_id,
            stp_inst_oid,
            SaiStpPortState::Blocking,
        ).map_err(StpOrchError::SaiError)?;

        stp_port_ids.insert(instance, stp_port_oid);
        self.stats.ports_created += 1;

        Ok(stp_port_oid)
    }

    /// Removes STP port.
    pub fn remove_stp_port(
        &mut self,
        instance: u16,
        stp_port_ids: &mut StpPortIds,
    ) -> Result<(), StpOrchError> {
        let stp_port_oid = stp_port_ids.get(&instance)
            .copied()
            .ok_or(StpOrchError::StpPortNotFound(instance))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        callbacks.remove_stp_port(stp_port_oid)
            .map_err(StpOrchError::SaiError)?;

        stp_port_ids.remove(&instance);
        self.stats.ports_removed += 1;

        Ok(())
    }

    /// Updates STP port state.
    pub fn update_port_state(
        &mut self,
        port_alias: &str,
        instance: u16,
        state: StpState,
        stp_port_ids: &mut StpPortIds,
    ) -> Result<(), StpOrchError> {
        // Lazy-create STP port if needed
        let stp_port_oid = if let Some(oid) = stp_port_ids.get(&instance) {
            *oid
        } else {
            self.add_stp_port(port_alias, instance, stp_port_ids)?
        };

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        let sai_state = state.to_sai_state();
        callbacks.set_stp_port_state(stp_port_oid, sai_state)
            .map_err(StpOrchError::SaiError)?;

        self.stats.state_updates += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceModify,
            "StpOrch",
            "update_port_state"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(port_alias.to_string())
        .with_object_type("stp_port")
        .with_details(serde_json::json!({
            "port": port_alias,
            "instance": instance,
            "state": format!("{:?}", state),
            "oid": stp_port_oid,
            "stats": {
                "state_updates": self.stats.state_updates
            }
        })));

        Ok(())
    }

    /// Flushes FDB for a VLAN.
    pub fn flush_fdb_vlan(&mut self, vlan_alias: &str) -> Result<(), StpOrchError> {
        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| StpOrchError::SaiError("No callbacks set".to_string()))?;

        callbacks.flush_fdb_by_vlan(vlan_alias)
            .map_err(StpOrchError::SaiError)?;

        self.stats.fdb_flushes += 1;

        Ok(())
    }

    /// Gets statistics.
    pub fn stats(&self) -> &StpOrchStats {
        &self.stats
    }

    /// Gets number of STP instances.
    pub fn instance_count(&self) -> usize {
        self.stp_inst_to_oid.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestCallbacks {
        ports_ready: bool,
        created_instances: Mutex<Vec<RawSaiObjectId>>,
        created_ports: Mutex<Vec<(RawSaiObjectId, RawSaiObjectId, SaiStpPortState)>>,
        next_oid: Mutex<RawSaiObjectId>,
    }

    impl TestCallbacks {
        fn new() -> Self {
            Self {
                ports_ready: true,
                created_instances: Mutex::new(Vec::new()),
                created_ports: Mutex::new(Vec::new()),
                next_oid: Mutex::new(0x1000),
            }
        }

        fn next_id(&self) -> RawSaiObjectId {
            let mut oid = self.next_oid.lock().unwrap();
            *oid += 1;
            *oid
        }
    }

    impl StpOrchCallbacks for TestCallbacks {
        fn all_ports_ready(&self) -> bool {
            self.ports_ready
        }

        fn get_port_bridge_port_id(&self, _alias: &str) -> Option<RawSaiObjectId> {
            Some(0x2000)
        }

        fn create_stp_instance(&self) -> Result<RawSaiObjectId, String> {
            let oid = self.next_id();
            self.created_instances.lock().unwrap().push(oid);
            Ok(oid)
        }

        fn remove_stp_instance(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn set_vlan_stp_instance(&self, _vlan_alias: &str, _stp_inst_oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn create_stp_port(&self, bridge_port_id: RawSaiObjectId, stp_inst_oid: RawSaiObjectId, state: SaiStpPortState) -> Result<RawSaiObjectId, String> {
            let oid = self.next_id();
            self.created_ports.lock().unwrap().push((bridge_port_id, stp_inst_oid, state));
            Ok(oid)
        }

        fn remove_stp_port(&self, _stp_port_oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn set_stp_port_state(&self, _stp_port_oid: RawSaiObjectId, _state: SaiStpPortState) -> Result<(), String> {
            Ok(())
        }

        fn flush_fdb_by_vlan(&self, _vlan_alias: &str) -> Result<(), String> {
            Ok(())
        }

        fn ensure_bridge_port(&self, _port_alias: &str) -> Result<RawSaiObjectId, String> {
            Ok(0x2000)
        }
    }

    #[test]
    fn test_create_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        let oid = orch.add_instance(1).unwrap();
        assert_eq!(orch.get_instance_oid(1), Some(oid));
        assert_eq!(orch.instance_count(), 2); // Default + 1

        let created = callbacks.created_instances.lock().unwrap();
        assert_eq!(created.len(), 1);
    }

    #[test]
    fn test_add_vlan_to_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_vlan_to_instance("Vlan100", 1).unwrap();

        let entry = orch.vlan_to_instance_map.get(&1).unwrap();
        assert_eq!(entry.vlan_count(), 1);
    }

    #[test]
    fn test_stp_port_lifecycle() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        let port_oid = orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids).unwrap();

        assert_eq!(stp_port_ids.get(&1), Some(&port_oid));

        let created = callbacks.created_ports.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].2, SaiStpPortState::Blocking); // Initial state

        orch.remove_stp_port(1, &mut stp_port_ids).unwrap();
        assert_eq!(stp_port_ids.get(&1), None);
    }

    #[test]
    fn test_update_port_state() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        orch.update_port_state("Ethernet0", 1, StpState::Forwarding, &mut stp_port_ids).unwrap();

        assert!(stp_port_ids.contains_key(&1));
        assert_eq!(orch.stats().state_updates, 1);
    }

    // STP Instance Management Tests

    #[test]
    fn test_multiple_stp_instances() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        // Create multiple instances
        let oid1 = orch.add_instance(1).unwrap();
        let oid2 = orch.add_instance(2).unwrap();
        let oid3 = orch.add_instance(3).unwrap();

        assert_ne!(oid1, oid2);
        assert_ne!(oid2, oid3);
        assert_eq!(orch.instance_count(), 4); // Default + 3 instances

        // Verify each instance has unique OID
        assert_eq!(orch.get_instance_oid(1), Some(oid1));
        assert_eq!(orch.get_instance_oid(2), Some(oid2));
        assert_eq!(orch.get_instance_oid(3), Some(oid3));

        assert_eq!(callbacks.created_instances.lock().unwrap().len(), 3);
    }

    #[test]
    fn test_default_stp_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let default_oid = 0x100;
        orch.initialize(default_oid, 256);

        // Default instance (instance 0) should exist
        assert_eq!(orch.get_instance_oid(0), Some(default_oid));
        assert_eq!(orch.instance_count(), 1);
    }

    #[test]
    fn test_remove_stp_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        let oid = orch.add_instance(1).unwrap();
        assert_eq!(orch.instance_count(), 2);

        // Remove instance
        orch.remove_instance(1).unwrap();
        assert_eq!(orch.get_instance_oid(1), None);
        assert_eq!(orch.instance_count(), 1);
        assert_eq!(orch.stats().instances_removed, 1);
    }

    #[test]
    fn test_create_duplicate_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        let oid1 = orch.add_instance(1).unwrap();
        let oid2 = orch.add_instance(1).unwrap(); // Duplicate

        // Should return same OID
        assert_eq!(oid1, oid2);
        assert_eq!(orch.instance_count(), 2); // Default + 1
        assert_eq!(callbacks.created_instances.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_invalid_instance_exceeds_max() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 10); // Max 10 instances

        let result = orch.add_instance(10);
        assert!(matches!(result, Err(StpOrchError::InvalidInstance(_))));
    }

    #[test]
    fn test_remove_nonexistent_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        let result = orch.remove_instance(99);
        assert!(matches!(result, Err(StpOrchError::InstanceNotFound(99))));
    }

    // Port State Management Tests

    #[test]
    fn test_port_state_disabled() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        orch.update_port_state("Ethernet0", 1, StpState::Disabled, &mut stp_port_ids).unwrap();

        assert!(stp_port_ids.contains_key(&1));
        assert_eq!(orch.stats().state_updates, 1);
    }

    #[test]
    fn test_port_state_blocking() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        orch.update_port_state("Ethernet0", 1, StpState::Blocking, &mut stp_port_ids).unwrap();

        assert!(stp_port_ids.contains_key(&1));
    }

    #[test]
    fn test_port_state_learning() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        orch.update_port_state("Ethernet0", 1, StpState::Learning, &mut stp_port_ids).unwrap();

        assert!(stp_port_ids.contains_key(&1));
    }

    #[test]
    fn test_port_state_transitions() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();

        // Transition: Blocking -> Learning -> Forwarding
        orch.update_port_state("Ethernet0", 1, StpState::Blocking, &mut stp_port_ids).unwrap();
        assert_eq!(orch.stats().state_updates, 1);

        orch.update_port_state("Ethernet0", 1, StpState::Learning, &mut stp_port_ids).unwrap();
        assert_eq!(orch.stats().state_updates, 2);

        orch.update_port_state("Ethernet0", 1, StpState::Forwarding, &mut stp_port_ids).unwrap();
        assert_eq!(orch.stats().state_updates, 3);
    }

    #[test]
    fn test_per_vlan_per_port_state() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        // Create two instances
        orch.add_instance(1).unwrap();
        orch.add_instance(2).unwrap();

        let mut stp_port_ids = HashMap::new();

        // Same port, different VLANs/instances, different states
        orch.update_port_state("Ethernet0", 1, StpState::Forwarding, &mut stp_port_ids).unwrap();
        orch.update_port_state("Ethernet0", 2, StpState::Blocking, &mut stp_port_ids).unwrap();

        // Should have two STP port entries
        assert_eq!(stp_port_ids.len(), 2);
        assert!(stp_port_ids.contains_key(&1));
        assert!(stp_port_ids.contains_key(&2));
    }

    #[test]
    fn test_multiple_ports_same_vlan() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids_eth0 = HashMap::new();
        let mut stp_port_ids_eth1 = HashMap::new();
        let mut stp_port_ids_eth2 = HashMap::new();

        // Add multiple ports to same instance
        orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids_eth0).unwrap();
        orch.add_stp_port("Ethernet1", 1, &mut stp_port_ids_eth1).unwrap();
        orch.add_stp_port("Ethernet2", 1, &mut stp_port_ids_eth2).unwrap();

        // All ports created with blocking state
        let created = callbacks.created_ports.lock().unwrap();
        assert_eq!(created.len(), 3);
        assert!(created.iter().all(|(_, _, state)| *state == SaiStpPortState::Blocking));
    }

    // VLAN Configuration Tests

    #[test]
    fn test_add_multiple_vlans_to_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_vlan_to_instance("Vlan100", 1).unwrap();
        orch.add_vlan_to_instance("Vlan200", 1).unwrap();
        orch.add_vlan_to_instance("Vlan300", 1).unwrap();

        let entry = orch.vlan_to_instance_map.get(&1).unwrap();
        assert_eq!(entry.vlan_count(), 3);
    }

    #[test]
    fn test_remove_vlan_from_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_vlan_to_instance("Vlan100", 1).unwrap();
        orch.add_vlan_to_instance("Vlan200", 1).unwrap();

        let entry = orch.vlan_to_instance_map.get(&1).unwrap();
        assert_eq!(entry.vlan_count(), 2);

        // Remove one VLAN
        orch.remove_vlan_from_instance("Vlan100", 1).unwrap();

        let entry = orch.vlan_to_instance_map.get(&1).unwrap();
        assert_eq!(entry.vlan_count(), 1);
    }

    #[test]
    fn test_vlan_membership_tracking() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        // Add VLANs to different instances
        orch.add_vlan_to_instance("Vlan100", 1).unwrap();
        orch.add_vlan_to_instance("Vlan200", 1).unwrap();
        orch.add_vlan_to_instance("Vlan300", 2).unwrap();

        let entry1 = orch.vlan_to_instance_map.get(&1).unwrap();
        let entry2 = orch.vlan_to_instance_map.get(&2).unwrap();

        assert_eq!(entry1.vlan_count(), 2);
        assert_eq!(entry2.vlan_count(), 1);
    }

    #[test]
    fn test_lazy_create_instance_on_vlan_add() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        // Add VLAN without creating instance first (lazy creation)
        assert_eq!(orch.instance_count(), 1); // Only default

        orch.add_vlan_to_instance("Vlan100", 5).unwrap();

        // Instance should be created automatically
        assert_eq!(orch.instance_count(), 2);
        assert!(orch.get_instance_oid(5).is_some());
    }

    // Port Operations Tests

    #[test]
    fn test_add_port_creates_with_blocking_state() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids).unwrap();

        let created = callbacks.created_ports.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].2, SaiStpPortState::Blocking);
    }

    #[test]
    fn test_add_duplicate_port() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        let oid1 = orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids).unwrap();
        let oid2 = orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids).unwrap();

        // Should return existing OID
        assert_eq!(oid1, oid2);
        assert_eq!(callbacks.created_ports.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_remove_port_from_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();
        orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids).unwrap();
        assert_eq!(orch.stats().ports_created, 1);

        orch.remove_stp_port(1, &mut stp_port_ids).unwrap();
        assert!(!stp_port_ids.contains_key(&1));
        assert_eq!(orch.stats().ports_removed, 1);
    }

    #[test]
    fn test_remove_nonexistent_port() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        let mut stp_port_ids = HashMap::new();
        let result = orch.remove_stp_port(99, &mut stp_port_ids);

        assert!(matches!(result, Err(StpOrchError::StpPortNotFound(99))));
    }

    #[test]
    fn test_port_spanning_multiple_vlans() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();
        orch.add_instance(2).unwrap();
        orch.add_instance(3).unwrap();

        let mut stp_port_ids = HashMap::new();

        // Same port in multiple instances
        orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids).unwrap();
        orch.add_stp_port("Ethernet0", 2, &mut stp_port_ids).unwrap();
        orch.add_stp_port("Ethernet0", 3, &mut stp_port_ids).unwrap();

        assert_eq!(stp_port_ids.len(), 3);
        assert!(stp_port_ids.contains_key(&1));
        assert!(stp_port_ids.contains_key(&2));
        assert!(stp_port_ids.contains_key(&3));
    }

    // Statistics Tests

    #[test]
    fn test_statistics_tracking() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        // Create instances
        orch.add_instance(1).unwrap();
        orch.add_instance(2).unwrap();
        assert_eq!(orch.stats().instances_created, 2);

        // Create ports
        let mut stp_port_ids1 = HashMap::new();
        let mut stp_port_ids2 = HashMap::new();
        orch.add_stp_port("Ethernet0", 1, &mut stp_port_ids1).unwrap();
        orch.add_stp_port("Ethernet1", 2, &mut stp_port_ids2).unwrap();
        assert_eq!(orch.stats().ports_created, 2);

        // Update states
        orch.update_port_state("Ethernet0", 1, StpState::Forwarding, &mut stp_port_ids1).unwrap();
        orch.update_port_state("Ethernet1", 2, StpState::Learning, &mut stp_port_ids2).unwrap();
        assert_eq!(orch.stats().state_updates, 2);

        // Remove ports
        orch.remove_stp_port(1, &mut stp_port_ids1).unwrap();
        assert_eq!(orch.stats().ports_removed, 1);

        // Remove instances
        orch.remove_instance(1).unwrap();
        assert_eq!(orch.stats().instances_removed, 1);
    }

    #[test]
    fn test_fdb_flush_statistics() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.flush_fdb_vlan("Vlan100").unwrap();
        orch.flush_fdb_vlan("Vlan200").unwrap();
        orch.flush_fdb_vlan("Vlan300").unwrap();

        assert_eq!(orch.stats().fdb_flushes, 3);
    }

    // Error Handling Tests

    #[test]
    fn test_add_port_without_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        let mut stp_port_ids = HashMap::new();
        let result = orch.add_stp_port("Ethernet0", 99, &mut stp_port_ids);

        assert!(matches!(result, Err(StpOrchError::InstanceNotFound(99))));
    }

    #[test]
    fn test_operations_without_callbacks() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        orch.initialize(0x100, 256);

        // Try to add instance without callbacks
        let result = orch.add_instance(1);
        assert!(matches!(result, Err(StpOrchError::SaiError(_))));

        // Try to remove instance without callbacks
        orch.stp_inst_to_oid.insert(1, 0x1234);
        let result = orch.remove_instance(1);
        assert!(matches!(result, Err(StpOrchError::SaiError(_))));
    }

    // Edge Cases Tests

    #[test]
    fn test_empty_stp_instance() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        // Create instance without any VLANs or ports
        let oid = orch.add_instance(1).unwrap();
        assert!(oid > 0);
        assert_eq!(orch.instance_count(), 2);

        // Instance should not have any VLANs
        assert!(orch.vlan_to_instance_map.get(&1).is_none());
    }

    #[test]
    fn test_rapid_state_changes() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();

        // Rapid state changes
        for _ in 0..10 {
            orch.update_port_state("Ethernet0", 1, StpState::Blocking, &mut stp_port_ids).unwrap();
            orch.update_port_state("Ethernet0", 1, StpState::Learning, &mut stp_port_ids).unwrap();
            orch.update_port_state("Ethernet0", 1, StpState::Forwarding, &mut stp_port_ids).unwrap();
        }

        assert_eq!(orch.stats().state_updates, 30);
    }

    #[test]
    fn test_remove_instance_clears_vlan_mapping() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        orch.add_vlan_to_instance("Vlan100", 1).unwrap();
        orch.add_vlan_to_instance("Vlan200", 1).unwrap();

        assert!(orch.vlan_to_instance_map.contains_key(&1));

        // Remove instance should clear VLAN mapping
        orch.remove_instance(1).unwrap();
        assert!(!orch.vlan_to_instance_map.contains_key(&1));
    }

    #[test]
    fn test_lazy_create_port_on_state_update() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());
        orch.initialize(0x100, 256);

        orch.add_instance(1).unwrap();

        let mut stp_port_ids = HashMap::new();

        // Update state without explicitly creating port (lazy creation)
        orch.update_port_state("Ethernet0", 1, StpState::Forwarding, &mut stp_port_ids).unwrap();

        // Port should be created automatically
        assert!(stp_port_ids.contains_key(&1));
        assert_eq!(callbacks.created_ports.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_instance_count_tracking() {
        let mut orch = StpOrch::new(StpOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);
        orch.initialize(0x100, 256);

        assert_eq!(orch.instance_count(), 1); // Default

        orch.add_instance(1).unwrap();
        assert_eq!(orch.instance_count(), 2);

        orch.add_instance(2).unwrap();
        orch.add_instance(3).unwrap();
        assert_eq!(orch.instance_count(), 4);

        orch.remove_instance(2).unwrap();
        assert_eq!(orch.instance_count(), 3);

        orch.remove_instance(1).unwrap();
        orch.remove_instance(3).unwrap();
        assert_eq!(orch.instance_count(), 1); // Back to default only
    }
}
