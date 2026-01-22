//! MlagOrch implementation.

use std::collections::HashSet;
use std::sync::Arc;

use super::types::{MlagIfUpdate, MlagIslUpdate, MlagSubjectType, MlagUpdate};

/// MLAG orchestrator error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MlagOrchError {
    /// Interface not found.
    InterfaceNotFound(String),
    /// Duplicate interface.
    DuplicateInterface(String),
    /// ISL not set.
    IslNotSet,
    /// Ports not ready.
    PortsNotReady,
}

impl std::fmt::Display for MlagOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InterfaceNotFound(name) => write!(f, "Interface not found: {}", name),
            Self::DuplicateInterface(name) => write!(f, "Duplicate interface: {}", name),
            Self::IslNotSet => write!(f, "ISL not set"),
            Self::PortsNotReady => write!(f, "Ports not ready"),
        }
    }
}

impl std::error::Error for MlagOrchError {}

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
    pub fn add_isl_interface(&mut self, isl_name: impl Into<String>) -> Result<bool, MlagOrchError> {
        let isl_name = isl_name.into();

        // Check if already set to this ISL
        if self.isl_name.as_deref() == Some(&isl_name) {
            return Ok(false);
        }

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
            return Err(MlagOrchError::IslNotSet);
        }

        let old_isl = self.isl_name.take().unwrap();
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
            return Err(MlagOrchError::DuplicateInterface(if_name));
        }

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
            return Err(MlagOrchError::InterfaceNotFound(if_name.to_string()));
        }

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
        assert!(matches!(&updates[0], MlagUpdate::Isl(u) if u.is_add && u.isl_name == "PortChannel100"));
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
}
