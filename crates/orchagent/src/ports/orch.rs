//! PortsOrch - Main orchestrator for port management.
//!
//! This is the Rust implementation of the C++ PortsOrch class. It handles:
//! - Port creation and initialization from hardware discovery
//! - Port configuration from CONFIG_DB
//! - LAG creation and member management
//! - VLAN creation and member management
//! - Queue and scheduler group management
//! - Port state machine (CONFIG_MISSING → CONFIG_RECEIVED → CONFIG_DONE)
//!
//! # Safety Improvements
//!
//! - Uses `SyncMap` instead of `std::map` to prevent auto-vivification
//! - Returns `Result` instead of throwing exceptions
//! - Uses owned data instead of raw pointers
//! - Type-safe port types via enums

use std::collections::HashMap;
use std::sync::Arc;

use sonic_orch_common::{SyncMap, TaskStatus};
use sonic_sai::types::RawSaiObjectId;

use super::config::{PortConfig, PortConfigError};
use super::port::{Port, PortAdminState, PortOperState, PortType};
use super::queue::{PriorityGroupInfo, QueueInfo, SchedulerGroupInfo};
use super::types::{
    GearboxPortTable, LagInfo, LagTable, PortInitState, PortSupportedSpeeds,
    PortTable, PortsOrchStats, SystemPortTable, VlanInfo, VlanMemberInfo,
    VlanTable, VlanTaggingMode,
};

/// Error type for PortsOrch operations.
#[derive(Debug, Clone)]
pub enum PortsOrchError {
    /// Port not found.
    PortNotFound(String),
    /// LAG not found.
    LagNotFound(String),
    /// VLAN not found.
    VlanNotFound(String),
    /// Port already exists.
    PortAlreadyExists(String),
    /// Invalid configuration.
    InvalidConfig(String),
    /// SAI error.
    SaiError(String),
    /// Port is in wrong state for operation.
    InvalidState(String),
    /// Resource exhausted.
    ResourceExhausted(String),
    /// Configuration parsing error.
    ConfigError(PortConfigError),
}

impl std::fmt::Display for PortsOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PortNotFound(alias) => write!(f, "Port not found: {}", alias),
            Self::LagNotFound(alias) => write!(f, "LAG not found: {}", alias),
            Self::VlanNotFound(alias) => write!(f, "VLAN not found: {}", alias),
            Self::PortAlreadyExists(alias) => write!(f, "Port already exists: {}", alias),
            Self::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            Self::SaiError(msg) => write!(f, "SAI error: {}", msg),
            Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            Self::ResourceExhausted(msg) => write!(f, "Resource exhausted: {}", msg),
            Self::ConfigError(e) => write!(f, "Config error: {}", e),
        }
    }
}

impl std::error::Error for PortsOrchError {}

impl From<PortConfigError> for PortsOrchError {
    fn from(e: PortConfigError) -> Self {
        Self::ConfigError(e)
    }
}

/// Result type alias for PortsOrch operations.
pub type Result<T> = std::result::Result<T, PortsOrchError>;

/// Callbacks for PortsOrch to notify other orchs of port events.
///
/// This replaces the C++ pattern of direct cross-orch calls with a callback interface,
/// allowing for better decoupling and testability.
#[derive(Clone)]
pub struct PortsOrchCallbacks {
    /// Called when a port's operational state changes.
    pub on_port_state_change: Option<Arc<dyn Fn(&str, PortOperState) + Send + Sync>>,
    /// Called when a new port is created.
    pub on_port_created: Option<Arc<dyn Fn(&Port) + Send + Sync>>,
    /// Called when a port is deleted.
    pub on_port_deleted: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// Called when a LAG is created.
    pub on_lag_created: Option<Arc<dyn Fn(&LagInfo) + Send + Sync>>,
    /// Called when a LAG member is added.
    pub on_lag_member_added: Option<Arc<dyn Fn(&str, &str) + Send + Sync>>,
    /// Called when a VLAN is created.
    pub on_vlan_created: Option<Arc<dyn Fn(&VlanInfo) + Send + Sync>>,
}

impl Default for PortsOrchCallbacks {
    fn default() -> Self {
        Self {
            on_port_state_change: None,
            on_port_created: None,
            on_port_deleted: None,
            on_lag_created: None,
            on_lag_member_added: None,
            on_vlan_created: None,
        }
    }
}

impl std::fmt::Debug for PortsOrchCallbacks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortsOrchCallbacks")
            .field("on_port_state_change", &self.on_port_state_change.is_some())
            .field("on_port_created", &self.on_port_created.is_some())
            .field("on_port_deleted", &self.on_port_deleted.is_some())
            .field("on_lag_created", &self.on_lag_created.is_some())
            .field("on_lag_member_added", &self.on_lag_member_added.is_some())
            .field("on_vlan_created", &self.on_vlan_created.is_some())
            .finish()
    }
}

/// Configuration for PortsOrch.
#[derive(Debug, Clone)]
pub struct PortsOrchConfig {
    /// Maximum number of ports supported.
    pub max_ports: usize,
    /// Maximum number of LAGs supported.
    pub max_lags: usize,
    /// Maximum number of VLANs supported.
    pub max_vlans: usize,
    /// Default MTU for new ports.
    pub default_mtu: u32,
    /// Default admin state for new ports.
    pub default_admin_state: PortAdminState,
    /// Whether to enable port state change logging.
    pub log_state_changes: bool,
    /// Whether to support gearbox (external PHY).
    pub gearbox_enabled: bool,
    /// Whether to support system ports (VOQ).
    pub system_port_enabled: bool,
}

impl Default for PortsOrchConfig {
    fn default() -> Self {
        Self {
            max_ports: 512,
            max_lags: 256,
            max_vlans: 4094,
            default_mtu: 9100,
            default_admin_state: PortAdminState::Down,
            log_state_changes: true,
            gearbox_enabled: false,
            system_port_enabled: false,
        }
    }
}

/// PortsOrch - The main port orchestration struct.
///
/// This manages all port-related state and operations in SONiC.
#[derive(Debug)]
pub struct PortsOrch {
    /// Configuration.
    config: PortsOrchConfig,

    /// Callbacks for notifying other orchs.
    callbacks: Option<Arc<PortsOrchCallbacks>>,

    // ============ Port Tables ============
    /// All ports indexed by alias.
    ports: PortTable,

    /// Ports indexed by SAI object ID (for reverse lookup).
    port_oid_to_alias: HashMap<RawSaiObjectId, String>,

    /// Ports indexed by lane (for hardware mapping).
    lane_to_port: HashMap<u32, String>,

    /// Port initialization state.
    port_init_states: HashMap<String, PortInitState>,

    /// Pending port configurations (waiting for hardware).
    pending_port_configs: HashMap<String, PortConfig>,

    // ============ LAG Tables ============
    /// LAGs indexed by alias.
    lags: LagTable,

    /// LAG member mapping: member alias → LAG alias.
    lag_member_to_lag: HashMap<String, String>,

    // ============ VLAN Tables ============
    /// VLANs indexed by alias.
    vlans: VlanTable,

    /// VLAN ID to alias mapping.
    vlan_id_to_alias: HashMap<u16, String>,

    // ============ Gearbox Tables ============
    /// Gearbox ports (if gearbox enabled).
    gearbox_ports: GearboxPortTable,

    // ============ System Port Tables (VOQ) ============
    /// System ports (if VOQ enabled).
    system_ports: SystemPortTable,

    // ============ QoS State ============
    /// Queue info per port: port alias → queues.
    port_queues: HashMap<String, Vec<QueueInfo>>,

    /// Priority group info per port.
    port_priority_groups: HashMap<String, Vec<PriorityGroupInfo>>,

    /// Scheduler group info per port.
    port_scheduler_groups: HashMap<String, Vec<SchedulerGroupInfo>>,

    // ============ Capability Caches ============
    /// Supported speeds per port.
    port_supported_speeds: HashMap<String, PortSupportedSpeeds>,

    // ============ State ============
    /// Whether initial port discovery is complete.
    initialized: bool,

    /// Number of ports expected (from platform config).
    expected_port_count: usize,

    /// Statistics.
    stats: PortsOrchStats,

    // ============ CPU Port ============
    /// CPU port object ID.
    cpu_port_id: RawSaiObjectId,

    /// Default VLAN ID.
    default_vlan_id: u16,
}

impl PortsOrch {
    /// Creates a new PortsOrch with the given configuration.
    pub fn new(config: PortsOrchConfig) -> Self {
        Self {
            config,
            callbacks: None,
            ports: SyncMap::new(),
            port_oid_to_alias: HashMap::new(),
            lane_to_port: HashMap::new(),
            port_init_states: HashMap::new(),
            pending_port_configs: HashMap::new(),
            lags: SyncMap::new(),
            lag_member_to_lag: HashMap::new(),
            vlans: SyncMap::new(),
            vlan_id_to_alias: HashMap::new(),
            gearbox_ports: SyncMap::new(),
            system_ports: SyncMap::new(),
            port_queues: HashMap::new(),
            port_priority_groups: HashMap::new(),
            port_scheduler_groups: HashMap::new(),
            port_supported_speeds: HashMap::new(),
            initialized: false,
            expected_port_count: 0,
            stats: PortsOrchStats::default(),
            cpu_port_id: 0,
            default_vlan_id: 1,
        }
    }

    /// Sets the callbacks for port events.
    pub fn set_callbacks(&mut self, callbacks: PortsOrchCallbacks) {
        self.callbacks = Some(Arc::new(callbacks));
    }

    // ============ Port Operations ============

    /// Returns true if a port exists with the given alias.
    pub fn has_port(&self, alias: &str) -> bool {
        self.ports.contains_key(&alias.to_string())
    }

    /// Gets a port by alias.
    pub fn get_port(&self, alias: &str) -> Option<Port> {
        self.ports.get(&alias.to_string()).map(|p| p.clone())
    }

    /// Gets a port by SAI object ID.
    pub fn get_port_by_oid(&self, oid: RawSaiObjectId) -> Option<Port> {
        self.port_oid_to_alias
            .get(&oid)
            .and_then(|alias| self.ports.get(alias))
            .map(|p| p.clone())
    }

    /// Gets a mutable reference to a port.
    ///
    /// Returns `Err(PortNotFound)` if the port doesn't exist - this is safer
    /// than the C++ version which would create an empty entry via auto-vivification.
    pub fn get_port_mut(&mut self, alias: &str) -> Result<&mut Port> {
        self.ports
            .get_mut(&alias.to_string())
            .ok_or_else(|| PortsOrchError::PortNotFound(alias.to_string()))
    }

    /// Returns the number of ports.
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    /// Returns all port aliases.
    pub fn port_aliases(&self) -> Vec<String> {
        self.ports.keys().cloned().collect()
    }

    /// Adds a port from hardware discovery.
    ///
    /// This is called during initialization when SAI reports the hardware ports.
    pub fn add_port_from_hardware(
        &mut self,
        alias: String,
        port_id: RawSaiObjectId,
        lanes: Vec<u32>,
    ) -> Result<()> {
        if self.ports.contains_key(&alias) {
            return Err(PortsOrchError::PortAlreadyExists(alias));
        }

        let mut port = Port::physical(&alias, lanes.clone());
        port.port_id = port_id;

        // Register lane mappings
        for lane in &lanes {
            self.lane_to_port.insert(*lane, alias.clone());
        }

        // Register OID mapping
        self.port_oid_to_alias.insert(port_id, alias.clone());

        // Set initial state
        self.port_init_states
            .insert(alias.clone(), PortInitState::ConfigMissing);

        // Check if we have pending config for this port
        if let Some(config) = self.pending_port_configs.remove(&alias) {
            config.apply_to(&mut port);
            self.port_init_states
                .insert(alias.clone(), PortInitState::ConfigReceived);
        }

        self.ports.insert(alias.clone(), port);
        self.stats.ports_created += 1;

        // Notify callbacks
        if let Some(callbacks) = &self.callbacks {
            if let Some(ref on_created) = callbacks.on_port_created {
                if let Some(port) = self.ports.get(&alias) {
                    on_created(port);
                }
            }
        }

        Ok(())
    }

    /// Configures a port from CONFIG_DB.
    ///
    /// If the port already exists (from hardware), applies the config.
    /// If not, stores the config as pending until hardware reports the port.
    pub fn configure_port(&mut self, config: PortConfig) -> Result<TaskStatus> {
        let alias = config
            .alias
            .as_ref()
            .ok_or_else(|| PortsOrchError::InvalidConfig("Missing alias".to_string()))?
            .clone();

        // Validate config
        config.validate()?;

        // If port exists, apply config
        if let Some(port) = self.ports.get_mut(&alias) {
            config.apply_to(port);
            self.port_init_states
                .insert(alias.clone(), PortInitState::ConfigReceived);
            self.stats.port_config_changes += 1;

            // If port is fully initialized, mark as done
            if port.initialized {
                self.port_init_states
                    .insert(alias.clone(), PortInitState::ConfigDone);
            }

            Ok(TaskStatus::Success)
        } else {
            // Port doesn't exist yet, store as pending
            self.pending_port_configs.insert(alias, config);
            Ok(TaskStatus::NeedRetry)
        }
    }

    /// Removes a port.
    pub fn remove_port(&mut self, alias: &str) -> Result<()> {
        let port = self
            .ports
            .remove(&alias.to_string())
            .ok_or_else(|| PortsOrchError::PortNotFound(alias.to_string()))?;

        // Clean up mappings
        self.port_oid_to_alias.remove(&port.port_id);
        for lane in &port.lanes {
            self.lane_to_port.remove(lane);
        }
        self.port_init_states.remove(alias);
        self.port_queues.remove(alias);
        self.port_priority_groups.remove(alias);
        self.port_scheduler_groups.remove(alias);
        self.port_supported_speeds.remove(alias);

        self.stats.ports_deleted += 1;

        // Notify callbacks
        if let Some(callbacks) = &self.callbacks {
            if let Some(ref on_deleted) = callbacks.on_port_deleted {
                on_deleted(alias);
            }
        }

        Ok(())
    }

    /// Sets the admin state of a port.
    pub fn set_port_admin_state(&mut self, alias: &str, state: PortAdminState) -> Result<()> {
        let port = self.get_port_mut(alias)?;
        port.set_admin_state(state);
        Ok(())
    }

    /// Sets the operational state of a port.
    ///
    /// This is typically called from port state change notifications.
    pub fn set_port_oper_state(&mut self, alias: &str, state: PortOperState) -> Result<()> {
        let port = self.get_port_mut(alias)?;
        let old_state = port.oper_state;
        port.oper_state = state;

        if self.config.log_state_changes && old_state != state {
            // Log state change (in production, use tracing/slog)
        }

        // Notify callbacks
        if old_state != state {
            // Clone the Arc before the mutable borrow ends
            let callbacks = self.callbacks.clone();
            if let Some(callbacks) = callbacks {
                if let Some(ref on_state_change) = callbacks.on_port_state_change {
                    on_state_change(alias, state);
                }
            }
        }

        Ok(())
    }

    /// Gets the initialization state of a port.
    pub fn get_port_init_state(&self, alias: &str) -> Option<PortInitState> {
        self.port_init_states.get(alias).copied()
    }

    /// Returns true if all expected ports are configured.
    pub fn all_ports_configured(&self) -> bool {
        if self.expected_port_count == 0 {
            return false;
        }
        self.port_init_states
            .values()
            .filter(|s| **s == PortInitState::ConfigDone)
            .count()
            >= self.expected_port_count
    }

    /// Sets the expected port count (from platform config).
    pub fn set_expected_port_count(&mut self, count: usize) {
        self.expected_port_count = count;
    }

    // ============ LAG Operations ============

    /// Returns true if a LAG exists with the given alias.
    pub fn has_lag(&self, alias: &str) -> bool {
        self.lags.contains_key(&alias.to_string())
    }

    /// Gets a LAG by alias.
    pub fn get_lag(&self, alias: &str) -> Option<LagInfo> {
        self.lags.get(&alias.to_string()).map(|l| l.clone())
    }

    /// Returns the number of LAGs.
    pub fn lag_count(&self) -> usize {
        self.lags.len()
    }

    /// Creates a new LAG.
    pub fn create_lag(&mut self, alias: &str, lag_id: RawSaiObjectId) -> Result<()> {
        if self.lags.contains_key(&alias.to_string()) {
            return Err(PortsOrchError::PortAlreadyExists(alias.to_string()));
        }

        if self.lags.len() >= self.config.max_lags {
            return Err(PortsOrchError::ResourceExhausted("Max LAGs reached".to_string()));
        }

        let lag = LagInfo::new(lag_id, alias);
        self.lags.insert(alias.to_string(), lag.clone());

        // Also create a Port entry for the LAG
        let mut port = Port::lag(alias);
        port.port_id = lag_id;
        self.ports.insert(alias.to_string(), port);
        self.port_oid_to_alias.insert(lag_id, alias.to_string());

        self.stats.lags_created += 1;

        // Notify callbacks
        if let Some(callbacks) = &self.callbacks {
            if let Some(ref on_lag_created) = callbacks.on_lag_created {
                on_lag_created(&lag);
            }
        }

        Ok(())
    }

    /// Removes a LAG.
    pub fn remove_lag(&mut self, alias: &str) -> Result<()> {
        let lag = self
            .lags
            .remove(&alias.to_string())
            .ok_or_else(|| PortsOrchError::LagNotFound(alias.to_string()))?;

        // Remove member mappings
        for member in &lag.members {
            self.lag_member_to_lag.remove(member);
        }

        // Remove Port entry
        self.ports.remove(&alias.to_string());
        self.port_oid_to_alias.remove(&lag.lag_id);

        self.stats.lags_deleted += 1;

        Ok(())
    }

    /// Adds a member to a LAG.
    pub fn add_lag_member(&mut self, lag_alias: &str, member_alias: &str) -> Result<()> {
        // Verify member port exists
        if !self.ports.contains_key(&member_alias.to_string()) {
            return Err(PortsOrchError::PortNotFound(member_alias.to_string()));
        }

        // Get LAG and add member
        let lag = self
            .lags
            .get_mut(&lag_alias.to_string())
            .ok_or_else(|| PortsOrchError::LagNotFound(lag_alias.to_string()))?;
        lag.add_member(member_alias);

        // Update member port
        let lag_id = lag.lag_id;
        let port = self
            .ports
            .get_mut(&member_alias.to_string())
            .ok_or_else(|| PortsOrchError::PortNotFound(member_alias.to_string()))?;
        port.lag_id = Some(lag_id);

        // Update mapping
        self.lag_member_to_lag
            .insert(member_alias.to_string(), lag_alias.to_string());

        // Notify callbacks
        let callbacks = self.callbacks.clone();
        if let Some(callbacks) = callbacks {
            if let Some(ref on_member_added) = callbacks.on_lag_member_added {
                on_member_added(lag_alias, member_alias);
            }
        }

        Ok(())
    }

    /// Removes a member from a LAG.
    pub fn remove_lag_member(&mut self, lag_alias: &str, member_alias: &str) -> Result<()> {
        // Get LAG and remove member
        let lag = self
            .lags
            .get_mut(&lag_alias.to_string())
            .ok_or_else(|| PortsOrchError::LagNotFound(lag_alias.to_string()))?;
        lag.remove_member(member_alias);

        // Update member port
        let port = self
            .ports
            .get_mut(&member_alias.to_string())
            .ok_or_else(|| PortsOrchError::PortNotFound(member_alias.to_string()))?;
        port.lag_id = None;
        port.lag_member_id = None;

        // Remove mapping
        self.lag_member_to_lag.remove(member_alias);

        Ok(())
    }

    /// Gets the LAG alias for a member port.
    pub fn get_lag_for_member(&self, member_alias: &str) -> Option<String> {
        self.lag_member_to_lag.get(member_alias).cloned()
    }

    // ============ VLAN Operations ============

    /// Returns true if a VLAN exists with the given alias.
    pub fn has_vlan(&self, alias: &str) -> bool {
        self.vlans.contains_key(&alias.to_string())
    }

    /// Gets a VLAN by alias.
    pub fn get_vlan(&self, alias: &str) -> Option<VlanInfo> {
        self.vlans.get(&alias.to_string()).map(|v| v.clone())
    }

    /// Gets a VLAN by VLAN ID.
    pub fn get_vlan_by_id(&self, vlan_id: u16) -> Option<VlanInfo> {
        self.vlan_id_to_alias
            .get(&vlan_id)
            .and_then(|alias| self.vlans.get(alias))
            .map(|v| v.clone())
    }

    /// Returns the number of VLANs.
    pub fn vlan_count(&self) -> usize {
        self.vlans.len()
    }

    /// Creates a new VLAN.
    pub fn create_vlan(
        &mut self,
        alias: &str,
        vlan_id: u16,
        sai_vlan_id: RawSaiObjectId,
    ) -> Result<()> {
        if self.vlans.contains_key(&alias.to_string()) {
            return Err(PortsOrchError::PortAlreadyExists(alias.to_string()));
        }

        if self.vlans.len() >= self.config.max_vlans {
            return Err(PortsOrchError::ResourceExhausted("Max VLANs reached".to_string()));
        }

        if vlan_id == 0 || vlan_id > 4094 {
            return Err(PortsOrchError::InvalidConfig(format!(
                "Invalid VLAN ID: {}",
                vlan_id
            )));
        }

        let vlan = VlanInfo::new(sai_vlan_id, vlan_id, alias);
        self.vlans.insert(alias.to_string(), vlan.clone());
        self.vlan_id_to_alias.insert(vlan_id, alias.to_string());

        // Also create a Port entry for the VLAN SVI
        let mut port = Port::vlan(alias, vlan_id);
        port.port_id = sai_vlan_id;
        self.ports.insert(alias.to_string(), port);

        self.stats.vlans_created += 1;

        // Notify callbacks
        if let Some(callbacks) = &self.callbacks {
            if let Some(ref on_vlan_created) = callbacks.on_vlan_created {
                on_vlan_created(&vlan);
            }
        }

        Ok(())
    }

    /// Removes a VLAN.
    pub fn remove_vlan(&mut self, alias: &str) -> Result<()> {
        let vlan = self
            .vlans
            .remove(&alias.to_string())
            .ok_or_else(|| PortsOrchError::VlanNotFound(alias.to_string()))?;

        self.vlan_id_to_alias.remove(&vlan.vlan_number);
        self.ports.remove(&alias.to_string());

        self.stats.vlans_deleted += 1;

        Ok(())
    }

    /// Adds a member to a VLAN.
    pub fn add_vlan_member(
        &mut self,
        vlan_alias: &str,
        port_alias: &str,
        tagging_mode: VlanTaggingMode,
        vlan_member_id: RawSaiObjectId,
        bridge_port_id: RawSaiObjectId,
    ) -> Result<()> {
        // Verify port exists
        if !self.ports.contains_key(&port_alias.to_string()) {
            return Err(PortsOrchError::PortNotFound(port_alias.to_string()));
        }

        // Get VLAN and add member
        let vlan = self
            .vlans
            .get_mut(&vlan_alias.to_string())
            .ok_or_else(|| PortsOrchError::VlanNotFound(vlan_alias.to_string()))?;

        let member_info = VlanMemberInfo {
            vlan_member_id,
            bridge_port_id,
            tagging_mode,
        };
        vlan.add_member(port_alias, member_info);
        let vlan_id = vlan.vlan_number;

        // Update port's VLAN membership
        let port = self
            .ports
            .get_mut(&port_alias.to_string())
            .ok_or_else(|| PortsOrchError::PortNotFound(port_alias.to_string()))?;
        port.add_vlan_member(vlan_id);

        Ok(())
    }

    /// Removes a member from a VLAN.
    pub fn remove_vlan_member(&mut self, vlan_alias: &str, port_alias: &str) -> Result<()> {
        let vlan = self
            .vlans
            .get_mut(&vlan_alias.to_string())
            .ok_or_else(|| PortsOrchError::VlanNotFound(vlan_alias.to_string()))?;

        vlan.remove_member(port_alias);
        let vlan_id = vlan.vlan_number;

        let port = self
            .ports
            .get_mut(&port_alias.to_string())
            .ok_or_else(|| PortsOrchError::PortNotFound(port_alias.to_string()))?;
        port.remove_vlan_member(vlan_id);

        Ok(())
    }

    // ============ Queue Operations ============

    /// Sets the queues for a port.
    pub fn set_port_queues(&mut self, alias: &str, queues: Vec<QueueInfo>) {
        self.port_queues.insert(alias.to_string(), queues);
    }

    /// Gets the queues for a port.
    pub fn get_port_queues(&self, alias: &str) -> Option<&Vec<QueueInfo>> {
        self.port_queues.get(alias)
    }

    /// Sets the priority groups for a port.
    pub fn set_port_priority_groups(&mut self, alias: &str, pgs: Vec<PriorityGroupInfo>) {
        self.port_priority_groups.insert(alias.to_string(), pgs);
    }

    /// Gets the priority groups for a port.
    pub fn get_port_priority_groups(&self, alias: &str) -> Option<&Vec<PriorityGroupInfo>> {
        self.port_priority_groups.get(alias)
    }

    // ============ State and Statistics ============

    /// Returns true if PortsOrch is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Sets the initialized flag.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.initialized = initialized;
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &PortsOrchStats {
        &self.stats
    }

    /// Sets the CPU port ID.
    pub fn set_cpu_port_id(&mut self, cpu_port_id: RawSaiObjectId) {
        self.cpu_port_id = cpu_port_id;
    }

    /// Gets the CPU port ID.
    pub fn cpu_port_id(&self) -> RawSaiObjectId {
        self.cpu_port_id
    }

    /// Sets the default VLAN ID.
    pub fn set_default_vlan_id(&mut self, vlan_id: u16) {
        self.default_vlan_id = vlan_id;
    }

    /// Gets the default VLAN ID.
    pub fn default_vlan_id(&self) -> u16 {
        self.default_vlan_id
    }

    // ============ Bulk Operations ============

    /// Gets all physical ports.
    pub fn get_physical_ports(&self) -> Vec<Port> {
        self.ports
            .values()
            .filter(|p| p.port_type == PortType::Phy)
            .cloned()
            .collect()
    }

    /// Gets all LAG ports.
    pub fn get_lag_ports(&self) -> Vec<Port> {
        self.ports
            .values()
            .filter(|p| p.port_type == PortType::Lag)
            .cloned()
            .collect()
    }

    /// Gets all VLAN ports.
    pub fn get_vlan_ports(&self) -> Vec<Port> {
        self.ports
            .values()
            .filter(|p| p.port_type == PortType::Vlan)
            .cloned()
            .collect()
    }

    /// Gets all ports that are operationally up.
    pub fn get_up_ports(&self) -> Vec<Port> {
        self.ports
            .values()
            .filter(|p| p.oper_state == PortOperState::Up)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ports_orch_new() {
        let orch = PortsOrch::new(PortsOrchConfig::default());
        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.lag_count(), 0);
        assert_eq!(orch.vlan_count(), 0);
        assert!(!orch.is_initialized());
    }

    #[test]
    fn test_add_port_from_hardware() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0, 1, 2, 3])
            .unwrap();

        assert!(orch.has_port("Ethernet0"));
        assert_eq!(orch.port_count(), 1);

        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.alias, "Ethernet0");
        assert_eq!(port.port_id, 0x1234);
        assert_eq!(port.lanes, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_port_init_state() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0])
            .unwrap();

        assert_eq!(
            orch.get_port_init_state("Ethernet0"),
            Some(PortInitState::ConfigMissing)
        );

        let mut config = PortConfig::new();
        config.alias = Some("Ethernet0".to_string());
        config.speed = Some(100000);
        orch.configure_port(config).unwrap();

        assert_eq!(
            orch.get_port_init_state("Ethernet0"),
            Some(PortInitState::ConfigReceived)
        );
    }

    #[test]
    fn test_pending_config() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        // Configure port before hardware reports it
        let mut config = PortConfig::new();
        config.alias = Some("Ethernet0".to_string());
        config.lanes = Some(vec![0, 1, 2, 3]);
        config.speed = Some(100000);

        let status = orch.configure_port(config).unwrap();
        assert_eq!(status, TaskStatus::NeedRetry); // Port doesn't exist yet

        // Now hardware reports the port
        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0, 1, 2, 3])
            .unwrap();

        // Config should have been applied
        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.speed, 100000);
        assert_eq!(
            orch.get_port_init_state("Ethernet0"),
            Some(PortInitState::ConfigReceived)
        );
    }

    #[test]
    fn test_duplicate_port() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0])
            .unwrap();

        let result =
            orch.add_port_from_hardware("Ethernet0".to_string(), 0x5678, vec![1]);
        assert!(matches!(result, Err(PortsOrchError::PortAlreadyExists(_))));
    }

    #[test]
    fn test_lag_operations() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        // Create ports first
        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1000, vec![0])
            .unwrap();
        orch.add_port_from_hardware("Ethernet4".to_string(), 0x1001, vec![1])
            .unwrap();

        // Create LAG
        orch.create_lag("PortChannel0001", 0x2000).unwrap();
        assert!(orch.has_lag("PortChannel0001"));
        assert!(orch.has_port("PortChannel0001")); // LAG also appears as a port

        // Add members
        orch.add_lag_member("PortChannel0001", "Ethernet0").unwrap();
        orch.add_lag_member("PortChannel0001", "Ethernet4").unwrap();

        let lag = orch.get_lag("PortChannel0001").unwrap();
        assert_eq!(lag.member_count(), 2);

        // Check member mappings
        assert_eq!(
            orch.get_lag_for_member("Ethernet0"),
            Some("PortChannel0001".to_string())
        );

        // Remove member
        orch.remove_lag_member("PortChannel0001", "Ethernet0")
            .unwrap();
        let lag = orch.get_lag("PortChannel0001").unwrap();
        assert_eq!(lag.member_count(), 1);

        // Remove LAG
        orch.remove_lag("PortChannel0001").unwrap();
        assert!(!orch.has_lag("PortChannel0001"));
    }

    #[test]
    fn test_vlan_operations() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        // Create port first
        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1000, vec![0])
            .unwrap();

        // Create VLAN
        orch.create_vlan("Vlan100", 100, 0x3000).unwrap();
        assert!(orch.has_vlan("Vlan100"));
        assert!(orch.has_port("Vlan100")); // VLAN SVI also appears as a port

        // Add member
        orch.add_vlan_member(
            "Vlan100",
            "Ethernet0",
            VlanTaggingMode::Tagged,
            0x4000,
            0x5000,
        )
        .unwrap();

        let vlan = orch.get_vlan("Vlan100").unwrap();
        assert_eq!(vlan.member_count(), 1);

        // Check port's VLAN membership
        let port = orch.get_port("Ethernet0").unwrap();
        assert!(port.vlan_members.contains(&100));

        // Remove member
        orch.remove_vlan_member("Vlan100", "Ethernet0").unwrap();
        let vlan = orch.get_vlan("Vlan100").unwrap();
        assert_eq!(vlan.member_count(), 0);

        // Remove VLAN
        orch.remove_vlan("Vlan100").unwrap();
        assert!(!orch.has_vlan("Vlan100"));
    }

    #[test]
    fn test_get_port_by_oid() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0])
            .unwrap();

        let port = orch.get_port_by_oid(0x1234).unwrap();
        assert_eq!(port.alias, "Ethernet0");

        assert!(orch.get_port_by_oid(0x9999).is_none());
    }

    #[test]
    fn test_port_state_changes() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0])
            .unwrap();

        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.admin_state, PortAdminState::Down);
        assert_eq!(port.oper_state, PortOperState::Down);

        orch.set_port_admin_state("Ethernet0", PortAdminState::Up)
            .unwrap();
        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.admin_state, PortAdminState::Up);

        orch.set_port_oper_state("Ethernet0", PortOperState::Up)
            .unwrap();
        let port = orch.get_port("Ethernet0").unwrap();
        assert_eq!(port.oper_state, PortOperState::Up);
    }

    #[test]
    fn test_statistics() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0])
            .unwrap();
        orch.create_lag("PortChannel0001", 0x2000).unwrap();
        orch.create_vlan("Vlan100", 100, 0x3000).unwrap();

        let stats = orch.stats();
        assert_eq!(stats.ports_created, 1);
        assert_eq!(stats.lags_created, 1);
        assert_eq!(stats.vlans_created, 1);

        orch.remove_port("Ethernet0").unwrap();
        orch.remove_lag("PortChannel0001").unwrap();
        orch.remove_vlan("Vlan100").unwrap();

        let stats = orch.stats();
        assert_eq!(stats.ports_deleted, 1);
        assert_eq!(stats.lags_deleted, 1);
        assert_eq!(stats.vlans_deleted, 1);
    }

    #[test]
    fn test_port_not_found_error() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        let result = orch.get_port_mut("NonExistent");
        assert!(matches!(result, Err(PortsOrchError::PortNotFound(_))));
    }

    #[test]
    fn test_get_ports_by_type() {
        let mut orch = PortsOrch::new(PortsOrchConfig::default());

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1000, vec![0])
            .unwrap();
        orch.add_port_from_hardware("Ethernet4".to_string(), 0x1001, vec![1])
            .unwrap();
        orch.create_lag("PortChannel0001", 0x2000).unwrap();
        orch.create_vlan("Vlan100", 100, 0x3000).unwrap();

        assert_eq!(orch.get_physical_ports().len(), 2);
        assert_eq!(orch.get_lag_ports().len(), 1);
        assert_eq!(orch.get_vlan_ports().len(), 1);
    }

    #[test]
    fn test_callbacks() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let port_created_count = Arc::new(AtomicUsize::new(0));
        let count_clone = port_created_count.clone();

        let mut orch = PortsOrch::new(PortsOrchConfig::default());
        orch.set_callbacks(PortsOrchCallbacks {
            on_port_created: Some(Arc::new(move |_port| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            })),
            ..Default::default()
        });

        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0])
            .unwrap();
        orch.add_port_from_hardware("Ethernet4".to_string(), 0x5678, vec![1])
            .unwrap();

        assert_eq!(port_created_count.load(Ordering::SeqCst), 2);
    }
}
