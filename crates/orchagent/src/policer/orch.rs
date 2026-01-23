//! PolicerOrch implementation.

use std::collections::HashMap;
use std::sync::Arc;

use sonic_sai::types::RawSaiObjectId;

use super::types::{PolicerConfig, PolicerEntry, StormType};

/// Policer orchestrator error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicerOrchError {
    /// Policer not found.
    PolicerNotFound(String),
    /// Policer already exists.
    PolicerExists(String),
    /// Invalid configuration.
    InvalidConfig(String),
    /// SAI error.
    SaiError(String),
    /// Port not found.
    PortNotFound(String),
    /// Port not ready.
    PortNotReady,
    /// Invalid storm type.
    InvalidStormType(String),
}

impl std::fmt::Display for PolicerOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PolicerNotFound(name) => write!(f, "Policer not found: {}", name),
            Self::PolicerExists(name) => write!(f, "Policer already exists: {}", name),
            Self::InvalidConfig(msg) => write!(f, "Invalid policer config: {}", msg),
            Self::SaiError(msg) => write!(f, "SAI error: {}", msg),
            Self::PortNotFound(name) => write!(f, "Port not found: {}", name),
            Self::PortNotReady => write!(f, "Ports not ready"),
            Self::InvalidStormType(t) => write!(f, "Invalid storm type: {}", t),
        }
    }
}

impl std::error::Error for PolicerOrchError {}

/// Callbacks for PolicerOrch operations.
pub trait PolicerOrchCallbacks: Send + Sync {
    /// Creates a policer via SAI.
    fn create_policer(&self, config: &PolicerConfig) -> Result<RawSaiObjectId, String>;

    /// Updates a policer's rate/burst parameters via SAI.
    fn update_policer(&self, oid: RawSaiObjectId, config: &PolicerConfig) -> Result<(), String>;

    /// Removes a policer via SAI.
    fn remove_policer(&self, oid: RawSaiObjectId) -> Result<(), String>;

    /// Gets port SAI object ID by name.
    fn get_port_id(&self, port_name: &str) -> Option<RawSaiObjectId>;

    /// Returns true if all ports are ready.
    fn all_ports_ready(&self) -> bool;

    /// Sets storm control policer on a port.
    fn set_port_storm_policer(
        &self,
        port_id: RawSaiObjectId,
        storm_type: StormType,
        policer_oid: Option<RawSaiObjectId>,
    ) -> Result<(), String>;
}

/// Policer orchestrator configuration.
#[derive(Debug, Clone, Default)]
pub struct PolicerOrchConfig {
    // Currently no configuration options, but reserved for future use
}

/// Policer orchestrator statistics.
#[derive(Debug, Clone, Default)]
pub struct PolicerOrchStats {
    /// Number of policers created.
    pub policers_created: u64,
    /// Number of policers removed.
    pub policers_removed: u64,
    /// Number of policers updated.
    pub policers_updated: u64,
    /// Number of storm control configs applied.
    pub storm_control_applied: u64,
}

/// Policer orchestrator for rate limiting and storm control.
pub struct PolicerOrch {
    /// Configuration.
    config: PolicerOrchConfig,
    /// Map from policer name to entry.
    policers: HashMap<String, PolicerEntry>,
    /// Callbacks for SAI and port queries.
    callbacks: Option<Arc<dyn PolicerOrchCallbacks>>,
    /// Whether the orch is initialized.
    initialized: bool,
    /// Statistics.
    stats: PolicerOrchStats,
}

impl std::fmt::Debug for PolicerOrch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicerOrch")
            .field("config", &self.config)
            .field("policer_count", &self.policers.len())
            .field("initialized", &self.initialized)
            .field("stats", &self.stats)
            .finish()
    }
}

impl PolicerOrch {
    /// Creates a new PolicerOrch with the given configuration.
    pub fn new(config: PolicerOrchConfig) -> Self {
        Self {
            config,
            policers: HashMap::new(),
            callbacks: None,
            initialized: false,
            stats: PolicerOrchStats::default(),
        }
    }

    /// Sets the callbacks for this orch.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn PolicerOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the configuration.
    pub fn config(&self) -> &PolicerOrchConfig {
        &self.config
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &PolicerOrchStats {
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

    /// Returns the number of policers.
    pub fn policer_count(&self) -> usize {
        self.policers.len()
    }

    /// Returns true if a policer exists.
    pub fn policer_exists(&self, name: &str) -> bool {
        self.policers.contains_key(name)
    }

    /// Gets the SAI OID for a policer.
    pub fn get_policer_oid(&self, name: &str) -> Option<RawSaiObjectId> {
        self.policers.get(name).map(|entry| entry.sai_oid)
    }

    /// Increments the reference count for a policer.
    pub fn increase_ref_count(&mut self, name: &str) -> Result<u32, PolicerOrchError> {
        let entry = self
            .policers
            .get_mut(name)
            .ok_or_else(|| PolicerOrchError::PolicerNotFound(name.to_string()))?;

        entry.add_ref();
        Ok(entry.ref_count)
    }

    /// Decrements the reference count for a policer.
    pub fn decrease_ref_count(&mut self, name: &str) -> Result<u32, PolicerOrchError> {
        let entry = self
            .policers
            .get_mut(name)
            .ok_or_else(|| PolicerOrchError::PolicerNotFound(name.to_string()))?;

        Ok(entry.remove_ref())
    }

    /// Creates or updates a policer.
    pub fn set_policer(&mut self, name: String, config: PolicerConfig) -> Result<(), PolicerOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| PolicerOrchError::InvalidConfig("No callbacks set".to_string()))?;

        if let Some(existing) = self.policers.get_mut(&name) {
            // Update existing policer
            // Only rate/burst parameters can be updated
            if !existing.config.is_rate_burst_update(&config) {
                return Err(PolicerOrchError::InvalidConfig(
                    "Cannot update policer mode/type/actions".to_string(),
                ));
            }

            callbacks
                .update_policer(existing.sai_oid, &config)
                .map_err(PolicerOrchError::SaiError)?;

            existing.config = config;
            self.stats.policers_updated += 1;
        } else {
            // Create new policer
            let sai_oid = callbacks
                .create_policer(&config)
                .map_err(PolicerOrchError::SaiError)?;

            let entry = PolicerEntry::new(sai_oid, config);
            self.policers.insert(name, entry);
            self.stats.policers_created += 1;
        }

        Ok(())
    }

    /// Removes a policer.
    pub fn remove_policer(&mut self, name: &str) -> Result<(), PolicerOrchError> {
        let callbacks = self
            .callbacks
            .as_ref()
            .ok_or_else(|| PolicerOrchError::InvalidConfig("No callbacks set".to_string()))?;

        let entry = self
            .policers
            .get(name)
            .ok_or_else(|| PolicerOrchError::PolicerNotFound(name.to_string()))?;

        // Check if policer is still in use
        if entry.ref_count > 0 {
            return Err(PolicerOrchError::InvalidConfig(format!(
                "Policer {} is still in use (ref_count={})",
                name, entry.ref_count
            )));
        }

        let sai_oid = entry.sai_oid;

        callbacks
            .remove_policer(sai_oid)
            .map_err(PolicerOrchError::SaiError)?;

        self.policers.remove(name);
        self.stats.policers_removed += 1;

        Ok(())
    }

    /// Configures storm control on a port.
    pub fn set_port_storm_control(
        &mut self,
        port_name: &str,
        storm_type: StormType,
        kbps: u64,
    ) -> Result<(), PolicerOrchError> {
        let callbacks = Arc::clone(
            self.callbacks
                .as_ref()
                .ok_or_else(|| PolicerOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        // Check if ports are ready
        if !callbacks.all_ports_ready() {
            return Err(PolicerOrchError::PortNotReady);
        }

        // Validate interface is Ethernet
        if !port_name.starts_with("Ethernet") {
            return Err(PolicerOrchError::InvalidConfig(format!(
                "Storm control only supported on Ethernet interfaces: {}",
                port_name
            )));
        }

        // Get port ID
        let port_id = callbacks
            .get_port_id(port_name)
            .ok_or_else(|| PolicerOrchError::PortNotFound(port_name.to_string()))?;

        // Generate policer name: "_<port>_<storm_type>"
        let policer_name = format!("_{}_{}", port_name, storm_type.as_str());

        // Create storm control policer config
        let config = PolicerConfig::storm_control(kbps);

        // Create or update the policer
        self.set_policer(policer_name.clone(), config)?;

        // Get the policer OID
        let policer_oid = self
            .get_policer_oid(&policer_name)
            .ok_or_else(|| PolicerOrchError::PolicerNotFound(policer_name.clone()))?;

        // Apply to port
        callbacks
            .set_port_storm_policer(port_id, storm_type, Some(policer_oid))
            .map_err(PolicerOrchError::SaiError)?;

        self.stats.storm_control_applied += 1;

        Ok(())
    }

    /// Removes storm control from a port.
    pub fn remove_port_storm_control(
        &mut self,
        port_name: &str,
        storm_type: StormType,
    ) -> Result<(), PolicerOrchError> {
        let callbacks = Arc::clone(
            self.callbacks
                .as_ref()
                .ok_or_else(|| PolicerOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        // Get port ID
        let port_id = callbacks
            .get_port_id(port_name)
            .ok_or_else(|| PolicerOrchError::PortNotFound(port_name.to_string()))?;

        // Detach policer from port
        callbacks
            .set_port_storm_policer(port_id, storm_type, None)
            .map_err(PolicerOrchError::SaiError)?;

        // Remove the policer
        let policer_name = format!("_{}_{}", port_name, storm_type.as_str());
        self.remove_policer(&policer_name)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policer::types::{MeterType, PacketAction, PolicerMode, ColorSource};
    use std::sync::Mutex;

    struct TestCallbacks {
        created_policers: Mutex<Vec<(RawSaiObjectId, PolicerConfig)>>,
        updated_policers: Mutex<Vec<RawSaiObjectId>>,
        removed_policers: Mutex<Vec<RawSaiObjectId>>,
        storm_policers: Mutex<Vec<(RawSaiObjectId, StormType, Option<RawSaiObjectId>)>>,
        next_oid: Mutex<RawSaiObjectId>,
        ports_ready: bool,
    }

    impl TestCallbacks {
        fn new() -> Self {
            Self {
                created_policers: Mutex::new(Vec::new()),
                updated_policers: Mutex::new(Vec::new()),
                removed_policers: Mutex::new(Vec::new()),
                storm_policers: Mutex::new(Vec::new()),
                next_oid: Mutex::new(0x1000),
                ports_ready: true,
            }
        }

        fn with_ports_ready(ports_ready: bool) -> Self {
            Self {
                ports_ready,
                ..Self::new()
            }
        }
    }

    impl PolicerOrchCallbacks for TestCallbacks {
        fn create_policer(&self, config: &PolicerConfig) -> Result<RawSaiObjectId, String> {
            let mut next_oid = self.next_oid.lock().unwrap();
            let oid = *next_oid;
            *next_oid += 1;
            self.created_policers
                .lock()
                .unwrap()
                .push((oid, config.clone()));
            Ok(oid)
        }

        fn update_policer(&self, oid: RawSaiObjectId, _config: &PolicerConfig) -> Result<(), String> {
            self.updated_policers.lock().unwrap().push(oid);
            Ok(())
        }

        fn remove_policer(&self, oid: RawSaiObjectId) -> Result<(), String> {
            self.removed_policers.lock().unwrap().push(oid);
            Ok(())
        }

        fn get_port_id(&self, port_name: &str) -> Option<RawSaiObjectId> {
            match port_name {
                "Ethernet0" => Some(0x100),
                "Ethernet4" => Some(0x104),
                _ => None,
            }
        }

        fn all_ports_ready(&self) -> bool {
            self.ports_ready
        }

        fn set_port_storm_policer(
            &self,
            port_id: RawSaiObjectId,
            storm_type: StormType,
            policer_oid: Option<RawSaiObjectId>,
        ) -> Result<(), String> {
            self.storm_policers
                .lock()
                .unwrap()
                .push((port_id, storm_type, policer_oid));
            Ok(())
        }
    }

    #[test]
    fn test_policer_orch_new() {
        let orch = PolicerOrch::new(PolicerOrchConfig::default());
        assert!(!orch.is_initialized());
        assert_eq!(orch.policer_count(), 0);
    }

    #[test]
    fn test_set_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 1000000,
            cbs: 8000,
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        let result = orch.set_policer("test_policer".to_string(), config);
        assert!(result.is_ok());
        assert_eq!(orch.policer_count(), 1);
        assert!(orch.policer_exists("test_policer"));

        // Check callback
        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].1.cir, 1000000);
    }

    #[test]
    fn test_update_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let mut config = PolicerConfig::new();
        config.cir = 1000000;

        orch.set_policer("test_policer".to_string(), config.clone())
            .unwrap();

        // Update rate
        config.cir = 2000000;
        let result = orch.set_policer("test_policer".to_string(), config);
        assert!(result.is_ok());

        // Should have 1 create and 1 update
        assert_eq!(callbacks.created_policers.lock().unwrap().len(), 1);
        assert_eq!(callbacks.updated_policers.lock().unwrap().len(), 1);
        assert_eq!(orch.stats().policers_updated, 1);
    }

    #[test]
    fn test_remove_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig::new();
        orch.set_policer("test_policer".to_string(), config).unwrap();

        let result = orch.remove_policer("test_policer");
        assert!(result.is_ok());
        assert_eq!(orch.policer_count(), 0);

        // Check callback
        let removed = callbacks.removed_policers.lock().unwrap();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_remove_policer_in_use() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let config = PolicerConfig::new();
        orch.set_policer("test_policer".to_string(), config).unwrap();

        // Increment ref count
        orch.increase_ref_count("test_policer").unwrap();

        // Should fail to remove
        let result = orch.remove_policer("test_policer");
        assert!(result.is_err());
        assert_eq!(orch.policer_count(), 1);
    }

    #[test]
    fn test_ref_count() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let config = PolicerConfig::new();
        orch.set_policer("test_policer".to_string(), config).unwrap();

        assert_eq!(orch.increase_ref_count("test_policer").unwrap(), 1);
        assert_eq!(orch.increase_ref_count("test_policer").unwrap(), 2);
        assert_eq!(orch.decrease_ref_count("test_policer").unwrap(), 1);
        assert_eq!(orch.decrease_ref_count("test_policer").unwrap(), 0);
    }

    #[test]
    fn test_storm_control() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let result = orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000);
        assert!(result.is_ok());

        // Should create policer named "_Ethernet0_broadcast"
        assert!(orch.policer_exists("_Ethernet0_broadcast"));

        // Check storm policer was applied to port
        let storm = callbacks.storm_policers.lock().unwrap();
        assert_eq!(storm.len(), 1);
        assert_eq!(storm[0].0, 0x100); // Ethernet0 port ID
        assert_eq!(storm[0].1, StormType::Broadcast);
        assert!(storm[0].2.is_some());
    }

    #[test]
    fn test_storm_control_ports_not_ready() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::with_ports_ready(false));
        orch.set_callbacks(callbacks);

        let result = orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000);
        assert!(matches!(result, Err(PolicerOrchError::PortNotReady)));
    }

    #[test]
    fn test_remove_storm_control() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000)
            .unwrap();

        let result = orch.remove_port_storm_control("Ethernet0", StormType::Broadcast);
        assert!(result.is_ok());
        assert!(!orch.policer_exists("_Ethernet0_broadcast"));

        // Check storm policer was detached
        let storm = callbacks.storm_policers.lock().unwrap();
        assert_eq!(storm.len(), 2); // One set, one unset
        assert_eq!(storm[1].2, None); // Unset
    }

    #[test]
    fn test_statistics() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let config = PolicerConfig::new();
        orch.set_policer("p1".to_string(), config.clone()).unwrap();
        orch.set_policer("p2".to_string(), config.clone()).unwrap();

        let mut updated_config = config;
        updated_config.cir = 2000000;
        orch.set_policer("p1".to_string(), updated_config).unwrap();

        orch.remove_policer("p2").unwrap();

        orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000)
            .unwrap();

        let stats = orch.stats();
        assert_eq!(stats.policers_created, 3); // p1, p2, storm
        assert_eq!(stats.policers_updated, 1); // p1
        assert_eq!(stats.policers_removed, 1); // p2
        assert_eq!(stats.storm_control_applied, 1);
    }

    // ==================== Policer Creation Tests ====================

    #[test]
    fn test_create_policer_with_cir_cbs() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 10_000_000, // 10 Mbps
            cbs: 100_000,    // 100 KB burst
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        let result = orch.set_policer("cir_cbs_policer".to_string(), config.clone());
        assert!(result.is_ok());
        assert!(orch.policer_exists("cir_cbs_policer"));

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].1.cir, 10_000_000);
        assert_eq!(created[0].1.cbs, 100_000);
        assert_eq!(created[0].1.pir, 0);
    }

    #[test]
    fn test_create_policer_with_pir_pbs() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::TrTcm,
            color_source: ColorSource::Blind,
            cir: 10_000_000,  // 10 Mbps committed
            cbs: 100_000,     // 100 KB committed burst
            pir: 20_000_000,  // 20 Mbps peak
            pbs: 200_000,     // 200 KB peak burst
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        let result = orch.set_policer("pir_pbs_policer".to_string(), config.clone());
        assert!(result.is_ok());

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.pir, 20_000_000);
        assert_eq!(created[0].1.pbs, 200_000);
    }

    #[test]
    fn test_create_trtcm_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::TrTcm,
            color_source: ColorSource::Blind,
            cir: 5_000_000,   // 5 Mbps
            cbs: 50_000,
            pir: 10_000_000,  // 10 Mbps
            pbs: 100_000,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        let result = orch.set_policer("trtcm_policer".to_string(), config.clone());
        assert!(result.is_ok());

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.mode, PolicerMode::TrTcm);
        assert_eq!(created[0].1.cir, 5_000_000);
        assert_eq!(created[0].1.pir, 10_000_000);
    }

    #[test]
    fn test_create_srtcm_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 8_000_000,
            cbs: 80_000,
            pir: 0,  // SR_TCM uses only CIR
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        let result = orch.set_policer("srtcm_policer".to_string(), config.clone());
        assert!(result.is_ok());

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.mode, PolicerMode::SrTcm);
        assert_eq!(created[0].1.cir, 8_000_000);
    }

    #[test]
    fn test_create_storm_control_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig::storm_control(16_000); // 16 Mbps

        let result = orch.set_policer("storm_policer".to_string(), config.clone());
        assert!(result.is_ok());

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.mode, PolicerMode::StormControl);
        assert_eq!(created[0].1.meter_type, MeterType::Bytes);
        // 16000 kbps = 16000 * 1000 / 8 = 2000000 bps
        assert_eq!(created[0].1.cir, 2_000_000);
    }

    #[test]
    fn test_create_policer_bytes_meter() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 1_000_000,
            cbs: 10_000,
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        orch.set_policer("bytes_policer".to_string(), config).unwrap();

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.meter_type, MeterType::Bytes);
    }

    #[test]
    fn test_create_policer_packets_meter() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Packets,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 10_000,  // 10k packets per second
            cbs: 1_000,   // 1k packet burst
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        orch.set_policer("packets_policer".to_string(), config).unwrap();

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.meter_type, MeterType::Packets);
        assert_eq!(created[0].1.cir, 10_000);
    }

    // ==================== Policer Actions Tests ====================

    #[test]
    fn test_policer_actions_forward_drop_trap() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::TrTcm,
            color_source: ColorSource::Blind,
            cir: 5_000_000,
            cbs: 50_000,
            pir: 10_000_000,
            pbs: 100_000,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Trap,
            red_action: PacketAction::Drop,
        };

        orch.set_policer("actions_policer".to_string(), config).unwrap();

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.green_action, PacketAction::Forward);
        assert_eq!(created[0].1.yellow_action, PacketAction::Trap);
        assert_eq!(created[0].1.red_action, PacketAction::Drop);
    }

    #[test]
    fn test_policer_color_aware_mode() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::TrTcm,
            color_source: ColorSource::Aware,  // Color-aware
            cir: 5_000_000,
            cbs: 50_000,
            pir: 10_000_000,
            pbs: 100_000,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        orch.set_policer("color_aware_policer".to_string(), config).unwrap();

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.color_source, ColorSource::Aware);
    }

    #[test]
    fn test_policer_color_blind_mode() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,  // Color-blind
            cir: 1_000_000,
            cbs: 10_000,
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        orch.set_policer("color_blind_policer".to_string(), config).unwrap();

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.color_source, ColorSource::Blind);
    }

    #[test]
    fn test_cannot_update_actions() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 1_000_000,
            cbs: 10_000,
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        orch.set_policer("test_policer".to_string(), config.clone()).unwrap();

        // Try to update action (should fail)
        let mut invalid_update = config;
        invalid_update.red_action = PacketAction::Trap;

        let result = orch.set_policer("test_policer".to_string(), invalid_update);
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::InvalidConfig(_))));
    }

    // ==================== Reference Counting Tests ====================

    #[test]
    fn test_ref_count_increment_decrement() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let config = PolicerConfig::new();
        orch.set_policer("ref_test".to_string(), config).unwrap();

        // Multiple increments
        assert_eq!(orch.increase_ref_count("ref_test").unwrap(), 1);
        assert_eq!(orch.increase_ref_count("ref_test").unwrap(), 2);
        assert_eq!(orch.increase_ref_count("ref_test").unwrap(), 3);

        // Multiple decrements
        assert_eq!(orch.decrease_ref_count("ref_test").unwrap(), 2);
        assert_eq!(orch.decrease_ref_count("ref_test").unwrap(), 1);
        assert_eq!(orch.decrease_ref_count("ref_test").unwrap(), 0);
    }

    #[test]
    fn test_cannot_remove_with_active_references() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let config = PolicerConfig::new();
        orch.set_policer("active_policer".to_string(), config).unwrap();

        // Add references
        orch.increase_ref_count("active_policer").unwrap();
        orch.increase_ref_count("active_policer").unwrap();

        // Cannot remove with active refs
        let result = orch.remove_policer("active_policer");
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::InvalidConfig(_))));
        assert!(orch.policer_exists("active_policer"));
    }

    #[test]
    fn test_ref_count_cleanup_allows_removal() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let config = PolicerConfig::new();
        orch.set_policer("cleanup_test".to_string(), config).unwrap();

        // Add and remove references
        orch.increase_ref_count("cleanup_test").unwrap();
        orch.increase_ref_count("cleanup_test").unwrap();
        orch.decrease_ref_count("cleanup_test").unwrap();
        orch.decrease_ref_count("cleanup_test").unwrap();

        // Should now be removable
        let result = orch.remove_policer("cleanup_test");
        assert!(result.is_ok());
        assert!(!orch.policer_exists("cleanup_test"));
    }

    #[test]
    fn test_ref_count_nonexistent_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let result = orch.increase_ref_count("nonexistent");
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::PolicerNotFound(_))));

        let result = orch.decrease_ref_count("nonexistent");
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::PolicerNotFound(_))));
    }

    // ==================== Storm Control Tests ====================

    #[test]
    fn test_storm_control_broadcast() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let result = orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 10_000);
        assert!(result.is_ok());

        assert!(orch.policer_exists("_Ethernet0_broadcast"));

        let storm = callbacks.storm_policers.lock().unwrap();
        assert_eq!(storm[0].1, StormType::Broadcast);
    }

    #[test]
    fn test_storm_control_unknown_unicast() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let result = orch.set_port_storm_control("Ethernet4", StormType::UnknownUnicast, 5_000);
        assert!(result.is_ok());

        assert!(orch.policer_exists("_Ethernet4_unknown-unicast"));

        let storm = callbacks.storm_policers.lock().unwrap();
        assert_eq!(storm[0].1, StormType::UnknownUnicast);
        assert_eq!(storm[0].0, 0x104); // Ethernet4 port ID
    }

    #[test]
    fn test_storm_control_unknown_multicast() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let result = orch.set_port_storm_control("Ethernet0", StormType::UnknownMulticast, 12_000);
        assert!(result.is_ok());

        assert!(orch.policer_exists("_Ethernet0_unknown-multicast"));

        let storm = callbacks.storm_policers.lock().unwrap();
        assert_eq!(storm[0].1, StormType::UnknownMulticast);
    }

    #[test]
    fn test_storm_control_non_ethernet_interface() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        // Try on non-Ethernet interface
        let result = orch.set_port_storm_control("PortChannel1", StormType::Broadcast, 8000);
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::InvalidConfig(_))));
    }

    #[test]
    fn test_storm_control_port_not_found() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let result = orch.set_port_storm_control("Ethernet999", StormType::Broadcast, 8000);
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::PortNotFound(_))));
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_remove_nonexistent_policer() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        let result = orch.remove_policer("nonexistent");
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::PolicerNotFound(_))));
    }

    #[test]
    fn test_get_oid_nonexistent_policer() {
        let orch = PolicerOrch::new(PolicerOrchConfig::default());
        let oid = orch.get_policer_oid("nonexistent");
        assert!(oid.is_none());
    }

    #[test]
    fn test_operations_without_callbacks() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        // No callbacks set

        let config = PolicerConfig::new();
        let result = orch.set_policer("test".to_string(), config);
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::InvalidConfig(_))));

        let result = orch.remove_policer("test");
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::InvalidConfig(_))));

        let result = orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000);
        assert!(result.is_err());
        assert!(matches!(result, Err(PolicerOrchError::InvalidConfig(_))));
    }

    // ==================== Edge Cases Tests ====================

    #[test]
    fn test_policer_with_only_cir_no_pir() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 5_000_000,
            cbs: 50_000,
            pir: 0,  // No PIR
            pbs: 0,  // No PBS
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        let result = orch.set_policer("cir_only".to_string(), config);
        assert!(result.is_ok());

        let created = callbacks.created_policers.lock().unwrap();
        assert_eq!(created[0].1.cir, 5_000_000);
        assert_eq!(created[0].1.pir, 0);
    }

    #[test]
    fn test_multiple_policers_with_same_config() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::SrTcm,
            color_source: ColorSource::Blind,
            cir: 1_000_000,
            cbs: 10_000,
            pir: 0,
            pbs: 0,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        // Create multiple policers with same config
        orch.set_policer("policer1".to_string(), config.clone()).unwrap();
        orch.set_policer("policer2".to_string(), config.clone()).unwrap();
        orch.set_policer("policer3".to_string(), config).unwrap();

        assert_eq!(orch.policer_count(), 3);
        assert!(orch.policer_exists("policer1"));
        assert!(orch.policer_exists("policer2"));
        assert!(orch.policer_exists("policer3"));

        // Each should have unique OID
        let oid1 = orch.get_policer_oid("policer1").unwrap();
        let oid2 = orch.get_policer_oid("policer2").unwrap();
        let oid3 = orch.get_policer_oid("policer3").unwrap();

        assert_ne!(oid1, oid2);
        assert_ne!(oid2, oid3);
        assert_ne!(oid1, oid3);
    }

    #[test]
    fn test_policer_update_rate_burst_only() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        let mut config = PolicerConfig {
            meter_type: MeterType::Bytes,
            mode: PolicerMode::TrTcm,
            color_source: ColorSource::Blind,
            cir: 5_000_000,
            cbs: 50_000,
            pir: 10_000_000,
            pbs: 100_000,
            green_action: PacketAction::Forward,
            yellow_action: PacketAction::Forward,
            red_action: PacketAction::Drop,
        };

        orch.set_policer("update_test".to_string(), config.clone()).unwrap();

        // Update rate and burst
        config.cir = 8_000_000;
        config.cbs = 80_000;
        config.pir = 15_000_000;
        config.pbs = 150_000;

        let result = orch.set_policer("update_test".to_string(), config);
        assert!(result.is_ok());

        let updated = callbacks.updated_policers.lock().unwrap();
        assert_eq!(updated.len(), 1);
    }

    #[test]
    fn test_policer_count_tracking() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks);

        assert_eq!(orch.policer_count(), 0);

        let config = PolicerConfig::new();
        orch.set_policer("p1".to_string(), config.clone()).unwrap();
        assert_eq!(orch.policer_count(), 1);

        orch.set_policer("p2".to_string(), config.clone()).unwrap();
        assert_eq!(orch.policer_count(), 2);

        orch.set_policer("p3".to_string(), config).unwrap();
        assert_eq!(orch.policer_count(), 3);

        orch.remove_policer("p2").unwrap();
        assert_eq!(orch.policer_count(), 2);

        orch.remove_policer("p1").unwrap();
        orch.remove_policer("p3").unwrap();
        assert_eq!(orch.policer_count(), 0);
    }

    #[test]
    fn test_multiple_storm_types_on_same_port() {
        let mut orch = PolicerOrch::new(PolicerOrchConfig::default());
        let callbacks = Arc::new(TestCallbacks::new());
        orch.set_callbacks(callbacks.clone());

        // Apply different storm control types to same port
        orch.set_port_storm_control("Ethernet0", StormType::Broadcast, 8000).unwrap();
        orch.set_port_storm_control("Ethernet0", StormType::UnknownUnicast, 6000).unwrap();
        orch.set_port_storm_control("Ethernet0", StormType::UnknownMulticast, 10000).unwrap();

        // Should have 3 policers
        assert_eq!(orch.policer_count(), 3);
        assert!(orch.policer_exists("_Ethernet0_broadcast"));
        assert!(orch.policer_exists("_Ethernet0_unknown-unicast"));
        assert!(orch.policer_exists("_Ethernet0_unknown-multicast"));

        let storm = callbacks.storm_policers.lock().unwrap();
        assert_eq!(storm.len(), 3);
    }
}
