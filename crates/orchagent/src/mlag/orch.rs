//! MlagOrch implementation.

use std::collections::HashSet;
use std::sync::Arc;

use super::types::{MlagIfUpdate, MlagIslUpdate, MlagSubjectType, MlagUpdate};

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use thiserror::Error;

/// MLAG orchestrator error type.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MlagOrchError {
    /// Interface not found.
    #[error("Interface not found: {0}")]
    InterfaceNotFound(String),
    /// Duplicate interface.
    #[error("Duplicate interface: {0}")]
    DuplicateInterface(String),
    /// ISL not set.
    #[error("ISL not set")]
    IslNotSet,
    /// Ports not ready.
    #[error("Ports not ready")]
    PortsNotReady,
}

/// Callbacks for MlagOrch operations.
pub trait MlagOrchCallbacks: Send + Sync {
    /// Notifies observers about an MLAG change.
    fn notify(&self, update: MlagUpdate);

    /// Returns true if all ports are ready.
    fn all_ports_ready(&self) -> bool;
}

/// MLAG orchestrator configuration.
#[derive(Debug, Clone, Default)]
pub struct MlagOrchConfig {
    // Currently no configuration options, but reserved for future use
}

/// MLAG orchestrator statistics.
#[derive(Debug, Clone, Default)]
pub struct MlagOrchStats {
    /// Number of ISL adds.
    pub isl_adds: u64,
    /// Number of ISL deletes.
    pub isl_deletes: u64,
    /// Number of interface adds.
    pub intf_adds: u64,
    /// Number of interface deletes.
    pub intf_deletes: u64,
    /// Number of notifications sent.
    pub notifications: u64,
}

/// MLAG orchestrator for Multi-Chassis Link Aggregation.
pub struct MlagOrch {
    /// Configuration.
    config: MlagOrchConfig,
    /// ISL (peer-link) name.
    isl_name: Option<String>,
    /// Set of MLAG member interfaces.
    mlag_intfs: HashSet<String>,
    /// Callbacks for notifications and port queries.
    callbacks: Option<Arc<dyn MlagOrchCallbacks>>,
    /// Whether the orch is initialized.
    initialized: bool,
    /// Statistics.
    stats: MlagOrchStats,
}

impl std::fmt::Debug for MlagOrch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MlagOrch")
            .field("config", &self.config)
            .field("isl_name", &self.isl_name)
            .field("mlag_intfs_count", &self.mlag_intfs.len())
            .field("initialized", &self.initialized)
            .field("stats", &self.stats)
            .finish()
    }
}

impl MlagOrch {
    /// Creates a new MlagOrch with the given configuration.
    pub fn new(config: MlagOrchConfig) -> Self {
        Self {
            config,
            isl_name: None,
            mlag_intfs: HashSet::new(),
            callbacks: None,
            initialized: false,
            stats: MlagOrchStats::default(),
        }
    }

    /// Sets the callbacks for this orch.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn MlagOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the configuration.
    pub fn config(&self) -> &MlagOrchConfig {
        &self.config
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &MlagOrchStats {
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

    /// Returns the current ISL name, if set.
    pub fn isl_name(&self) -> Option<&str> {
        self.isl_name.as_deref()
    }

    /// Returns true if the given interface is the ISL.
    pub fn is_isl_interface(&self, if_name: &str) -> bool {
        self.isl_name.as_deref() == Some(if_name)
    }

    /// Returns true if the given interface is an MLAG member.
    pub fn is_mlag_interface(&self, if_name: &str) -> bool {
        self.mlag_intfs.contains(if_name)
    }

    /// Returns an iterator over all MLAG interfaces.
    pub fn mlag_interfaces(&self) -> impl Iterator<Item = &String> {
        self.mlag_intfs.iter()
    }

    /// Returns the number of MLAG interfaces.
    pub fn mlag_interface_count(&self) -> usize {
        self.mlag_intfs.len()
    }

    /// Adds or updates the ISL interface.
    ///
    /// Returns Ok(true) if the ISL was changed, Ok(false) if it was already set to this value.
    pub fn add_isl_interface(
        &mut self,
        isl_name: impl Into<String>,
    ) -> Result<bool, MlagOrchError> {
        let isl_name = isl_name.into();

        // Check if already set to this ISL
        if self.isl_name.as_deref() == Some(&isl_name) {
            return Ok(false);
        }

        let audit_record = AuditRecord::new(AuditCategory::NetworkConfig, "MlagOrch", "set_isl")
            .with_outcome(AuditOutcome::Success)
            .with_object_id(&isl_name)
            .with_object_type("mlag_isl")
            .with_details(serde_json::json!({
                "isl_interface": isl_name,
                "action": "isl_created_or_modified",
            }));
        audit_log!(audit_record);

        self.isl_name = Some(isl_name.clone());
        self.stats.isl_adds += 1;

        // Notify observers
        self.notify(MlagUpdate::Isl(MlagIslUpdate::add(isl_name)));

        Ok(true)
    }

    /// Removes the ISL interface.
    ///
    /// Returns Ok(true) if the ISL was removed, Err if it wasn't set.
    pub fn del_isl_interface(&mut self) -> Result<bool, MlagOrchError> {
        if self.isl_name.is_none() {
            let audit_record =
                AuditRecord::new(AuditCategory::NetworkConfig, "MlagOrch", "set_isl")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id("ISL")
                    .with_object_type("mlag_isl")
                    .with_error("ISL not set");
            audit_log!(audit_record);
            return Err(MlagOrchError::IslNotSet);
        }

        let old_isl = self.isl_name.take().unwrap();

        let audit_record = AuditRecord::new(AuditCategory::NetworkConfig, "MlagOrch", "set_isl")
            .with_outcome(AuditOutcome::Success)
            .with_object_id(&old_isl)
            .with_object_type("mlag_isl")
            .with_details(serde_json::json!({
                "isl_interface": old_isl,
                "action": "isl_deleted",
            }));
        audit_log!(audit_record);

        self.stats.isl_deletes += 1;

        // Notify observers
        self.notify(MlagUpdate::Isl(MlagIslUpdate::delete(old_isl)));

        Ok(true)
    }

    /// Adds an MLAG member interface.
    ///
    /// Returns Ok(true) if the interface was added, Err if it was already present.
    pub fn add_mlag_interface(
        &mut self,
        if_name: impl Into<String>,
    ) -> Result<bool, MlagOrchError> {
        let if_name = if_name.into();

        if self.mlag_intfs.contains(&if_name) {
            let audit_record =
                AuditRecord::new(AuditCategory::ResourceCreate, "MlagOrch", "add_mlag_intf")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&if_name)
                    .with_object_type("mlag_interface")
                    .with_error("Interface already a member");
            audit_log!(audit_record);
            return Err(MlagOrchError::DuplicateInterface(if_name));
        }

        let audit_record =
            AuditRecord::new(AuditCategory::ResourceCreate, "MlagOrch", "add_mlag_intf")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(&if_name)
                .with_object_type("mlag_interface")
                .with_details(serde_json::json!({
                    "interface_name": if_name,
                    "total_mlag_members": self.mlag_intfs.len() + 1,
                }));
        audit_log!(audit_record);

        self.mlag_intfs.insert(if_name.clone());
        self.stats.intf_adds += 1;

        // Notify observers
        self.notify(MlagUpdate::Intf(MlagIfUpdate::add(if_name)));

        Ok(true)
    }

    /// Removes an MLAG member interface.
    ///
    /// Returns Ok(true) if the interface was removed, Err if it wasn't found.
    pub fn del_mlag_interface(&mut self, if_name: &str) -> Result<bool, MlagOrchError> {
        if !self.mlag_intfs.remove(if_name) {
            let audit_record = AuditRecord::new(
                AuditCategory::ResourceDelete,
                "MlagOrch",
                "remove_mlag_intf",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(if_name)
            .with_object_type("mlag_interface")
            .with_error("Interface not found");
            audit_log!(audit_record);
            return Err(MlagOrchError::InterfaceNotFound(if_name.to_string()));
        }

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "MlagOrch",
            "remove_mlag_intf",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(if_name)
        .with_object_type("mlag_interface")
        .with_details(serde_json::json!({
            "interface_name": if_name,
            "remaining_mlag_members": self.mlag_intfs.len(),
        }));
        audit_log!(audit_record);

        self.stats.intf_deletes += 1;

        // Notify observers
        self.notify(MlagUpdate::Intf(MlagIfUpdate::delete(if_name)));

        Ok(true)
    }

    /// Parses an MLAG interface key to extract the interface name.
    ///
    /// Key format: `MCLAG_INTF_TABLE|mclag<id>|ifname` or just `mclag<id>|ifname`
    pub fn parse_mlag_interface_key(key: &str) -> Option<String> {
        // Split by '|' and take the last part
        let parts: Vec<&str> = key.split('|').collect();
        if parts.len() >= 2 {
            Some(parts.last()?.to_string())
        } else {
            None
        }
    }

    /// Sends a notification to observers.
    fn notify(&mut self, update: MlagUpdate) {
        self.stats.notifications += 1;
        if let Some(callbacks) = &self.callbacks {
            callbacks.notify(update);
        }
    }

    /// Returns true if all ports are ready.
    pub fn all_ports_ready(&self) -> bool {
        self.callbacks
            .as_ref()
            .map(|cb| cb.all_ports_ready())
            .unwrap_or(true) // Default to true if no callbacks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestCallbacks {
        updates: Mutex<Vec<MlagUpdate>>,
        ports_ready: bool,
    }

    impl TestCallbacks {
        fn new() -> Self {
            Self {
                updates: Mutex::new(Vec::new()),
                ports_ready: true,
            }
        }

        fn with_ports_ready(ports_ready: bool) -> Self {
            Self {
                updates: Mutex::new(Vec::new()),
                ports_ready,
            }
        }
    }

    impl MlagOrchCallbacks for TestCallbacks {
        fn notify(&self, update: MlagUpdate) {
            self.updates.lock().unwrap().push(update);
        }

        fn all_ports_ready(&self) -> bool {
            self.ports_ready
        }
    }

    #[test]
    fn test_mlag_orch_new() {
        let orch = MlagOrch::new(MlagOrchConfig::default());
        assert!(!orch.is_initialized());
        assert!(orch.isl_name().is_none());
        assert_eq!(orch.mlag_interface_count(), 0);
    }

    #[test]
    fn test_add_isl_interface() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Add ISL
        let result = orch.add_isl_interface("PortChannel100");
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(orch.isl_name(), Some("PortChannel100"));
        assert!(orch.is_isl_interface("PortChannel100"));

        // Adding same ISL again should return Ok(false)
        let result = orch.add_isl_interface("PortChannel100");
        assert!(result.is_ok());
        assert!(!result.unwrap());

        // Check notification
        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 1);
        assert!(
            matches!(&updates[0], MlagUpdate::Isl(u) if u.is_add && u.isl_name == "PortChannel100")
        );
    }

    #[test]
    fn test_del_isl_interface() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Add then delete ISL
        orch.add_isl_interface("PortChannel100").unwrap();
        let result = orch.del_isl_interface();
        assert!(result.is_ok());
        assert!(orch.isl_name().is_none());

        // Delete without ISL should error
        let result = orch.del_isl_interface();
        assert!(result.is_err());

        // Check notifications
        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 2);
        assert!(matches!(&updates[1], MlagUpdate::Isl(u) if !u.is_add));
    }

    #[test]
    fn test_add_mlag_interface() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Add interface
        let result = orch.add_mlag_interface("Ethernet0");
        assert!(result.is_ok());
        assert!(orch.is_mlag_interface("Ethernet0"));
        assert_eq!(orch.mlag_interface_count(), 1);

        // Add another interface
        orch.add_mlag_interface("Ethernet4").unwrap();
        assert_eq!(orch.mlag_interface_count(), 2);

        // Adding duplicate should error
        let result = orch.add_mlag_interface("Ethernet0");
        assert!(matches!(result, Err(MlagOrchError::DuplicateInterface(_))));

        // Check notifications (2 successful adds)
        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 2);
    }

    #[test]
    fn test_del_mlag_interface() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Add then delete
        orch.add_mlag_interface("Ethernet0").unwrap();
        let result = orch.del_mlag_interface("Ethernet0");
        assert!(result.is_ok());
        assert!(!orch.is_mlag_interface("Ethernet0"));
        assert_eq!(orch.mlag_interface_count(), 0);

        // Delete unknown should error
        let result = orch.del_mlag_interface("Ethernet99");
        assert!(matches!(result, Err(MlagOrchError::InterfaceNotFound(_))));

        // Check notifications
        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 2); // add + delete
        assert!(matches!(&updates[1], MlagUpdate::Intf(u) if !u.is_add));
    }

    #[test]
    fn test_parse_mlag_interface_key() {
        // Full key format
        let key = "MCLAG_INTF_TABLE|mclag1|Ethernet0";
        assert_eq!(
            MlagOrch::parse_mlag_interface_key(key),
            Some("Ethernet0".to_string())
        );

        // Shorter key format
        let key = "mclag1|Ethernet4";
        assert_eq!(
            MlagOrch::parse_mlag_interface_key(key),
            Some("Ethernet4".to_string())
        );

        // Invalid key
        let key = "no_delimiter";
        assert!(MlagOrch::parse_mlag_interface_key(key).is_none());
    }

    #[test]
    fn test_mlag_interfaces_iterator() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.add_mlag_interface("Ethernet4").unwrap();
        orch.add_mlag_interface("Ethernet8").unwrap();

        let intfs: Vec<&String> = orch.mlag_interfaces().collect();
        assert_eq!(intfs.len(), 3);
        assert!(intfs.iter().any(|i| *i == "Ethernet0"));
        assert!(intfs.iter().any(|i| *i == "Ethernet4"));
        assert!(intfs.iter().any(|i| *i == "Ethernet8"));
    }

    #[test]
    fn test_statistics() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        orch.add_isl_interface("PortChannel100").unwrap();
        orch.del_isl_interface().unwrap();
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.add_mlag_interface("Ethernet4").unwrap();
        orch.del_mlag_interface("Ethernet0").unwrap();

        let stats = orch.stats();
        assert_eq!(stats.isl_adds, 1);
        assert_eq!(stats.isl_deletes, 1);
        assert_eq!(stats.intf_adds, 2);
        assert_eq!(stats.intf_deletes, 1);
        assert_eq!(stats.notifications, 5);
    }

    #[test]
    fn test_all_ports_ready() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Without callbacks, default to true
        assert!(orch.all_ports_ready());

        // With callbacks returning false
        let callbacks = Arc::new(TestCallbacks::with_ports_ready(false));
        orch.set_callbacks(callbacks);
        assert!(!orch.all_ports_ready());
    }

    // ========================================================================
    // MLAG Domain Management Tests
    // ========================================================================

    #[test]
    fn test_isl_update_and_reconfiguration() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Set initial ISL
        orch.add_isl_interface("PortChannel100").unwrap();
        assert_eq!(orch.isl_name(), Some("PortChannel100"));

        // Update ISL to a different interface
        let result = orch.add_isl_interface("PortChannel200");
        assert!(result.is_ok());
        assert!(result.unwrap());
        assert_eq!(orch.isl_name(), Some("PortChannel200"));
        assert!(!orch.is_isl_interface("PortChannel100"));
        assert!(orch.is_isl_interface("PortChannel200"));

        // Verify notifications for both ISL changes
        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 2);
    }

    #[test]
    fn test_isl_without_callbacks() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Should work without callbacks
        let result = orch.add_isl_interface("PortChannel100");
        assert!(result.is_ok());
        assert_eq!(orch.isl_name(), Some("PortChannel100"));

        let result = orch.del_isl_interface();
        assert!(result.is_ok());
        assert!(orch.isl_name().is_none());
    }

    #[test]
    fn test_isl_with_special_names() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Test with various interface naming formats
        orch.add_isl_interface("PortChannel1").unwrap();
        assert_eq!(orch.isl_name(), Some("PortChannel1"));

        orch.add_isl_interface("PortChannel999").unwrap();
        assert_eq!(orch.isl_name(), Some("PortChannel999"));

        orch.add_isl_interface("peer-link").unwrap();
        assert_eq!(orch.isl_name(), Some("peer-link"));
    }

    #[test]
    fn test_multiple_isl_deletes() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // First delete without ISL should fail
        assert!(matches!(
            orch.del_isl_interface(),
            Err(MlagOrchError::IslNotSet)
        ));

        // Add ISL and delete
        orch.add_isl_interface("PortChannel100").unwrap();
        assert!(orch.del_isl_interface().is_ok());

        // Second delete should fail
        assert!(matches!(
            orch.del_isl_interface(),
            Err(MlagOrchError::IslNotSet)
        ));
    }

    // ========================================================================
    // MLAG Interface Management Tests
    // ========================================================================

    #[test]
    fn test_multiple_mlag_interfaces() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Add multiple interfaces
        let interfaces = vec![
            "Ethernet0",
            "Ethernet4",
            "Ethernet8",
            "PortChannel10",
            "PortChannel20",
        ];

        for intf in &interfaces {
            let result = orch.add_mlag_interface(*intf);
            assert!(result.is_ok());
        }

        assert_eq!(orch.mlag_interface_count(), 5);

        // Verify all interfaces are tracked
        for intf in &interfaces {
            assert!(orch.is_mlag_interface(intf));
        }

        // Verify notifications
        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 5);
    }

    #[test]
    fn test_mlag_interface_lifecycle() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Add interface
        orch.add_mlag_interface("Ethernet0").unwrap();
        assert_eq!(orch.mlag_interface_count(), 1);
        assert!(orch.is_mlag_interface("Ethernet0"));

        // Remove interface
        orch.del_mlag_interface("Ethernet0").unwrap();
        assert_eq!(orch.mlag_interface_count(), 0);
        assert!(!orch.is_mlag_interface("Ethernet0"));

        // Re-add same interface
        orch.add_mlag_interface("Ethernet0").unwrap();
        assert_eq!(orch.mlag_interface_count(), 1);
        assert!(orch.is_mlag_interface("Ethernet0"));
    }

    #[test]
    fn test_mlag_interface_with_port_channels() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Add various PortChannel interfaces
        orch.add_mlag_interface("PortChannel1").unwrap();
        orch.add_mlag_interface("PortChannel10").unwrap();
        orch.add_mlag_interface("PortChannel100").unwrap();

        assert_eq!(orch.mlag_interface_count(), 3);
        assert!(orch.is_mlag_interface("PortChannel1"));
        assert!(orch.is_mlag_interface("PortChannel10"));
        assert!(orch.is_mlag_interface("PortChannel100"));
    }

    #[test]
    fn test_remove_all_mlag_interfaces() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Add multiple interfaces
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.add_mlag_interface("Ethernet4").unwrap();
        orch.add_mlag_interface("Ethernet8").unwrap();
        assert_eq!(orch.mlag_interface_count(), 3);

        // Remove all
        orch.del_mlag_interface("Ethernet0").unwrap();
        orch.del_mlag_interface("Ethernet4").unwrap();
        orch.del_mlag_interface("Ethernet8").unwrap();

        assert_eq!(orch.mlag_interface_count(), 0);
    }

    #[test]
    fn test_mlag_interface_partial_removal() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Add 5 interfaces
        for i in 0..5 {
            orch.add_mlag_interface(format!("Ethernet{}", i * 4))
                .unwrap();
        }
        assert_eq!(orch.mlag_interface_count(), 5);

        // Remove 3 interfaces
        orch.del_mlag_interface("Ethernet0").unwrap();
        orch.del_mlag_interface("Ethernet8").unwrap();
        orch.del_mlag_interface("Ethernet16").unwrap();

        assert_eq!(orch.mlag_interface_count(), 2);
        assert!(orch.is_mlag_interface("Ethernet4"));
        assert!(orch.is_mlag_interface("Ethernet12"));
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[test]
    fn test_duplicate_interface_error() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        orch.add_mlag_interface("Ethernet0").unwrap();

        let result = orch.add_mlag_interface("Ethernet0");
        assert!(matches!(result, Err(MlagOrchError::DuplicateInterface(_))));

        if let Err(MlagOrchError::DuplicateInterface(name)) = result {
            assert_eq!(name, "Ethernet0");
        }
    }

    #[test]
    fn test_interface_not_found_error() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        let result = orch.del_mlag_interface("NonExistent");
        assert!(matches!(result, Err(MlagOrchError::InterfaceNotFound(_))));

        if let Err(MlagOrchError::InterfaceNotFound(name)) = result {
            assert_eq!(name, "NonExistent");
        }
    }

    #[test]
    fn test_isl_not_set_error() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        let result = orch.del_isl_interface();
        assert!(matches!(result, Err(MlagOrchError::IslNotSet)));
    }

    #[test]
    fn test_error_display_messages() {
        let err1 = MlagOrchError::InterfaceNotFound("Ethernet0".to_string());
        assert_eq!(format!("{}", err1), "Interface not found: Ethernet0");

        let err2 = MlagOrchError::DuplicateInterface("Ethernet4".to_string());
        assert_eq!(format!("{}", err2), "Duplicate interface: Ethernet4");

        let err3 = MlagOrchError::IslNotSet;
        assert_eq!(format!("{}", err3), "ISL not set");

        let err4 = MlagOrchError::PortsNotReady;
        assert_eq!(format!("{}", err4), "Ports not ready");
    }

    // ========================================================================
    // Statistics Tracking Tests
    // ========================================================================

    #[test]
    fn test_statistics_initialization() {
        let orch = MlagOrch::new(MlagOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.isl_adds, 0);
        assert_eq!(stats.isl_deletes, 0);
        assert_eq!(stats.intf_adds, 0);
        assert_eq!(stats.intf_deletes, 0);
        assert_eq!(stats.notifications, 0);
    }

    #[test]
    fn test_statistics_isl_operations() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Multiple ISL add/delete cycles
        for i in 0..3 {
            orch.add_isl_interface(format!("PortChannel{}", i * 100))
                .unwrap();
            orch.del_isl_interface().unwrap();
        }

        let stats = orch.stats();
        assert_eq!(stats.isl_adds, 3);
        assert_eq!(stats.isl_deletes, 3);
    }

    #[test]
    fn test_statistics_interface_operations() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Add 10 interfaces
        for i in 0..10 {
            orch.add_mlag_interface(format!("Ethernet{}", i * 4))
                .unwrap();
        }

        // Delete 5 interfaces
        for i in 0..5 {
            orch.del_mlag_interface(&format!("Ethernet{}", i * 4))
                .unwrap();
        }

        let stats = orch.stats();
        assert_eq!(stats.intf_adds, 10);
        assert_eq!(stats.intf_deletes, 5);
    }

    #[test]
    fn test_statistics_notifications_count() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Perform various operations
        orch.add_isl_interface("PortChannel100").unwrap();
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.add_mlag_interface("Ethernet4").unwrap();
        orch.del_mlag_interface("Ethernet0").unwrap();
        orch.del_isl_interface().unwrap();

        let stats = orch.stats();
        assert_eq!(stats.notifications, 5);

        // Verify callbacks received all notifications
        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 5);
    }

    #[test]
    fn test_statistics_after_errors() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Successful operations
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.add_mlag_interface("Ethernet4").unwrap();

        // Failed operations (should not increment stats)
        let _ = orch.add_mlag_interface("Ethernet0"); // Duplicate
        let _ = orch.del_mlag_interface("NonExistent"); // Not found

        let stats = orch.stats();
        assert_eq!(stats.intf_adds, 2);
        assert_eq!(stats.intf_deletes, 0);
    }

    // ========================================================================
    // Initialization and Configuration Tests
    // ========================================================================

    #[test]
    fn test_initialization_state() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        assert!(!orch.is_initialized());

        orch.set_initialized();
        assert!(orch.is_initialized());
    }

    #[test]
    fn test_configuration_access() {
        let config = MlagOrchConfig::default();
        let orch = MlagOrch::new(config);

        let _retrieved_config = orch.config();
        // Config access should not panic
    }

    #[test]
    fn test_callbacks_without_initial_setup() {
        let orch = MlagOrch::new(MlagOrchConfig::default());

        // Should default to true when no callbacks are set
        assert!(orch.all_ports_ready());
    }

    #[test]
    fn test_callbacks_setup_and_query() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        let callbacks_ready = Arc::new(TestCallbacks::with_ports_ready(true));
        orch.set_callbacks(callbacks_ready);
        assert!(orch.all_ports_ready());

        let callbacks_not_ready = Arc::new(TestCallbacks::with_ports_ready(false));
        orch.set_callbacks(callbacks_not_ready);
        assert!(!orch.all_ports_ready());
    }

    // ========================================================================
    // Key Parsing Tests
    // ========================================================================

    #[test]
    fn test_parse_mlag_interface_key_variations() {
        // Standard format with table prefix
        let key1 = "MCLAG_INTF_TABLE|mclag1|Ethernet0";
        assert_eq!(
            MlagOrch::parse_mlag_interface_key(key1),
            Some("Ethernet0".to_string())
        );

        // Format without table prefix
        let key2 = "mclag10|PortChannel5";
        assert_eq!(
            MlagOrch::parse_mlag_interface_key(key2),
            Some("PortChannel5".to_string())
        );

        // Multiple delimiters
        let key3 = "MCLAG_INTF_TABLE|mclag1|PortChannel10";
        assert_eq!(
            MlagOrch::parse_mlag_interface_key(key3),
            Some("PortChannel10".to_string())
        );
    }

    #[test]
    fn test_parse_mlag_interface_key_edge_cases() {
        // Empty string
        assert!(MlagOrch::parse_mlag_interface_key("").is_none());

        // Single word (no delimiter)
        assert!(MlagOrch::parse_mlag_interface_key("Ethernet0").is_none());

        // Just delimiter
        assert_eq!(
            MlagOrch::parse_mlag_interface_key("|"),
            Some("".to_string())
        );

        // Multiple consecutive delimiters
        let key = "MCLAG_INTF_TABLE||mclag1||Ethernet0";
        assert_eq!(
            MlagOrch::parse_mlag_interface_key(key),
            Some("Ethernet0".to_string())
        );
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_full_mlag_setup_workflow() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Initialize
        orch.set_initialized();
        assert!(orch.is_initialized());

        // Set ISL
        orch.add_isl_interface("PortChannel100").unwrap();
        assert!(orch.is_isl_interface("PortChannel100"));

        // Add MLAG interfaces
        orch.add_mlag_interface("PortChannel1").unwrap();
        orch.add_mlag_interface("PortChannel2").unwrap();
        orch.add_mlag_interface("PortChannel3").unwrap();

        assert_eq!(orch.mlag_interface_count(), 3);

        // Verify state
        assert!(orch.all_ports_ready());

        // Verify statistics
        let stats = orch.stats();
        assert_eq!(stats.isl_adds, 1);
        assert_eq!(stats.intf_adds, 3);
        assert_eq!(stats.notifications, 4);
    }

    #[test]
    fn test_mlag_teardown_workflow() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Setup
        orch.add_isl_interface("PortChannel100").unwrap();
        orch.add_mlag_interface("PortChannel1").unwrap();
        orch.add_mlag_interface("PortChannel2").unwrap();

        // Teardown - remove interfaces first
        orch.del_mlag_interface("PortChannel1").unwrap();
        orch.del_mlag_interface("PortChannel2").unwrap();
        assert_eq!(orch.mlag_interface_count(), 0);

        // Then remove ISL
        orch.del_isl_interface().unwrap();
        assert!(orch.isl_name().is_none());

        // Verify clean state
        let stats = orch.stats();
        assert_eq!(stats.isl_adds, 1);
        assert_eq!(stats.isl_deletes, 1);
        assert_eq!(stats.intf_adds, 2);
        assert_eq!(stats.intf_deletes, 2);
    }

    #[test]
    fn test_mlag_interface_iterator_empty() {
        let orch = MlagOrch::new(MlagOrchConfig::default());
        let intfs: Vec<&String> = orch.mlag_interfaces().collect();
        assert_eq!(intfs.len(), 0);
    }

    #[test]
    fn test_mlag_interface_iterator_after_operations() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Add interfaces
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.add_mlag_interface("Ethernet4").unwrap();
        orch.add_mlag_interface("Ethernet8").unwrap();

        // Remove one
        orch.del_mlag_interface("Ethernet4").unwrap();

        // Verify iterator
        let intfs: Vec<&String> = orch.mlag_interfaces().collect();
        assert_eq!(intfs.len(), 2);
        assert!(intfs.iter().any(|i| *i == "Ethernet0"));
        assert!(intfs.iter().any(|i| *i == "Ethernet8"));
        assert!(!intfs.iter().any(|i| *i == "Ethernet4"));
    }

    #[test]
    fn test_isl_and_mlag_interface_separation() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());

        // Set ISL
        orch.add_isl_interface("PortChannel100").unwrap();

        // Add MLAG interface with different name
        orch.add_mlag_interface("PortChannel1").unwrap();

        // Verify they are tracked separately
        assert!(orch.is_isl_interface("PortChannel100"));
        assert!(!orch.is_mlag_interface("PortChannel100"));
        assert!(orch.is_mlag_interface("PortChannel1"));
        assert!(!orch.is_isl_interface("PortChannel1"));
    }

    #[test]
    fn test_notifications_order() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Perform operations in sequence
        orch.add_isl_interface("PortChannel100").unwrap();
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.del_mlag_interface("Ethernet0").unwrap();
        orch.del_isl_interface().unwrap();

        let updates = callbacks.updates.lock().unwrap();
        assert_eq!(updates.len(), 4);

        // Verify order
        assert!(matches!(&updates[0], MlagUpdate::Isl(u) if u.is_add));
        assert!(matches!(&updates[1], MlagUpdate::Intf(u) if u.is_add));
        assert!(matches!(&updates[2], MlagUpdate::Intf(u) if !u.is_add));
        assert!(matches!(&updates[3], MlagUpdate::Isl(u) if !u.is_add));
    }

    #[test]
    fn test_debug_formatting() {
        let mut orch = MlagOrch::new(MlagOrchConfig::default());
        orch.add_isl_interface("PortChannel100").unwrap();
        orch.add_mlag_interface("Ethernet0").unwrap();
        orch.set_initialized();

        let debug_str = format!("{:?}", orch);
        assert!(debug_str.contains("MlagOrch"));
        assert!(debug_str.contains("initialized"));
    }

    #[test]
    fn test_error_trait_implementation() {
        use std::error::Error;

        let err = MlagOrchError::InterfaceNotFound("test".to_string());
        let _err_trait: &dyn Error = &err;
        assert!(format!("{}", err).contains("test"));
    }
}
