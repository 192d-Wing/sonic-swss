//! Fabric ports orchestration logic.
//!
//! FabricPortsOrch monitors fabric port health and manages port isolation
//! for maintaining fabric connectivity in modular chassis systems.
//!
//! Key responsibilities:
//! - Monitor fabric port link status
//! - Track port health metrics (error counters)
//! - Auto-isolate ports with excessive errors
//! - Support manual isolation configuration

use super::types::{FabricPortState, IsolationState, LinkStatus, PortHealthState};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};
use crate::audit_log;

/// Result type for FabricPortsOrch operations.
pub type Result<T> = std::result::Result<T, FabricPortsOrchError>;

#[derive(Debug, Clone)]
pub enum FabricPortsOrchError {
    PortNotFound(u32),
    PortExists(u32),
    InvalidLane(u32),
    SaiError(String),
}

#[derive(Debug, Clone)]
pub struct FabricPortsOrchConfig {
    /// Enable fabric port monitoring.
    pub monitoring_enabled: bool,
    /// Polling interval in milliseconds.
    pub poll_interval_ms: u64,
    /// Error threshold for auto-isolation.
    pub auto_isolate_threshold: u64,
    /// Recovery threshold (consecutive polls without errors).
    pub recovery_threshold: u64,
    /// Maximum lanes per fabric port.
    pub max_lanes: u32,
}

impl Default for FabricPortsOrchConfig {
    fn default() -> Self {
        Self {
            monitoring_enabled: true,
            poll_interval_ms: 1000,
            auto_isolate_threshold: 10,
            recovery_threshold: 5,
            max_lanes: 8,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FabricPortsOrchStats {
    pub ports_monitored: u64,
    pub ports_up: u64,
    pub ports_down: u64,
    pub auto_isolations: u64,
    pub config_isolations: u64,
    pub recoveries: u64,
    pub poll_cycles: u64,
    pub errors: u64,
}

/// Callbacks for Fabric Ports SAI operations.
pub trait FabricPortsOrchCallbacks: Send + Sync {
    /// Get fabric port SAI OID.
    fn get_fabric_port_oid(&self, lane: u32) -> Result<RawSaiObjectId>;

    /// Get fabric port link status from SAI.
    fn get_link_status(&self, oid: RawSaiObjectId) -> Result<LinkStatus>;

    /// Get fabric port error counters from SAI.
    fn get_error_counters(&self, oid: RawSaiObjectId) -> Result<u64>;

    /// Set fabric port isolation state in SAI.
    fn set_isolation(&self, oid: RawSaiObjectId, isolate: bool) -> Result<()>;

    /// Write fabric port state to STATE_DB.
    fn write_state_db(&self, lane: u32, state: &FabricPortState) -> Result<()>;

    /// Remove fabric port state from STATE_DB.
    fn remove_state_db(&self, lane: u32) -> Result<()>;

    /// Notification when link status changes.
    fn on_link_status_changed(&self, lane: u32, old_status: LinkStatus, new_status: LinkStatus);

    /// Notification when port is isolated.
    fn on_port_isolated(&self, lane: u32, reason: IsolationState);

    /// Notification when port is recovered.
    fn on_port_recovered(&self, lane: u32);
}

pub struct FabricPortsOrch<C: FabricPortsOrchCallbacks> {
    config: FabricPortsOrchConfig,
    stats: FabricPortsOrchStats,
    callbacks: Option<Arc<C>>,
    ports: HashMap<u32, FabricPortState>,
}

impl<C: FabricPortsOrchCallbacks> FabricPortsOrch<C> {
    pub fn new(config: FabricPortsOrchConfig) -> Self {
        Self {
            config,
            stats: FabricPortsOrchStats::default(),
            callbacks: None,
            ports: HashMap::new(),
        }
    }

    pub fn with_callbacks(config: FabricPortsOrchConfig, callbacks: Arc<C>) -> Self {
        Self {
            config,
            stats: FabricPortsOrchStats::default(),
            callbacks: Some(callbacks),
            ports: HashMap::new(),
        }
    }

    pub fn config(&self) -> &FabricPortsOrchConfig {
        &self.config
    }

    pub fn stats(&self) -> &FabricPortsOrchStats {
        &self.stats
    }

    // ===== Port Management =====

    /// Add a fabric port to monitor.
    pub fn add_port(&mut self, lane: u32) -> Result<()> {
        if self.ports.contains_key(&lane) {
            return Err(FabricPortsOrchError::PortExists(lane));
        }

        if lane >= self.config.max_lanes {
            return Err(FabricPortsOrchError::InvalidLane(lane));
        }

        let sai_oid = if let Some(ref callbacks) = self.callbacks {
            callbacks.get_fabric_port_oid(lane)?
        } else {
            0x3000 + lane as u64
        };

        let port = FabricPortState {
            lane,
            sai_oid,
            status: LinkStatus::Down,
            health: PortHealthState::default(),
            isolation: IsolationState::Active,
        };

        self.ports.insert(lane, port);
        self.stats.ports_monitored += 1;

        Ok(())
    }

    /// Remove a fabric port from monitoring.
    pub fn remove_port(&mut self, lane: u32) -> Result<()> {
        let _port = self
            .ports
            .remove(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        if let Some(ref callbacks) = self.callbacks {
            let _ = callbacks.remove_state_db(lane);
        }

        self.stats.ports_monitored = self.ports.len() as u64;
        Ok(())
    }

    /// Get a fabric port by lane.
    pub fn get_port(&self, lane: u32) -> Option<&FabricPortState> {
        self.ports.get(&lane)
    }

    /// Get a mutable fabric port by lane.
    pub fn get_port_mut(&mut self, lane: u32) -> Option<&mut FabricPortState> {
        self.ports.get_mut(&lane)
    }

    /// Get port count.
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    // ===== Link Status Management =====

    /// Update link status for a port.
    pub fn update_link_status(&mut self, lane: u32, new_status: LinkStatus) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        let old_status = port.status;

        if old_status == new_status {
            return Ok(());
        }

        port.status = new_status;

        // Update stats
        match (old_status, new_status) {
            (LinkStatus::Down, LinkStatus::Up) => {
                self.stats.ports_up += 1;
                if self.stats.ports_down > 0 {
                    self.stats.ports_down -= 1;
                }
            }
            (LinkStatus::Up, LinkStatus::Down) => {
                self.stats.ports_down += 1;
                if self.stats.ports_up > 0 {
                    self.stats.ports_up -= 1;
                }
            }
            _ => {}
        }

        let record = AuditRecord::new(
            AuditCategory::NetworkConfig,
            "FabricPortsOrch",
            format!("update_link_status: lane {}", lane),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("lane_{}", lane))
        .with_object_type("fabric_port")
        .with_details(serde_json::json!({
            "old_status": format!("{:?}", old_status),
            "new_status": format!("{:?}", new_status),
            "sai_oid": format!("{:#x}", port.sai_oid),
        }));
        audit_log!(record);

        if let Some(ref callbacks) = self.callbacks {
            callbacks.on_link_status_changed(lane, old_status, new_status);
            let _ = callbacks.write_state_db(lane, port);
        }

        Ok(())
    }

    /// Get all ports with a specific link status.
    pub fn get_ports_by_status(&self, status: LinkStatus) -> Vec<u32> {
        self.ports
            .iter()
            .filter(|(_, port)| port.status == status)
            .map(|(lane, _)| *lane)
            .collect()
    }

    // ===== Health Monitoring =====

    /// Record an error poll for a port.
    pub fn record_error(&mut self, lane: u32) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        port.health.consecutive_polls_with_errors += 1;
        port.health.consecutive_polls_with_no_errors = 0;

        // Check for auto-isolation threshold
        if port.health.consecutive_polls_with_errors >= self.config.auto_isolate_threshold
            && port.isolation == IsolationState::Active
        {
            self.auto_isolate_port(lane)?;
        }

        Ok(())
    }

    /// Record a successful poll for a port.
    pub fn record_success(&mut self, lane: u32) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        port.health.consecutive_polls_with_no_errors += 1;
        port.health.consecutive_polls_with_errors = 0;

        // Check for recovery threshold
        if port.health.consecutive_polls_with_no_errors >= self.config.recovery_threshold
            && port.isolation == IsolationState::AutoIsolated
        {
            self.recover_port(lane)?;
        }

        Ok(())
    }

    /// Get port health state.
    pub fn get_health(&self, lane: u32) -> Option<&PortHealthState> {
        self.ports.get(&lane).map(|p| &p.health)
    }

    // ===== Isolation Management =====

    /// Auto-isolate a port due to errors.
    fn auto_isolate_port(&mut self, lane: u32) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.set_isolation(port.sai_oid, true)?;
            callbacks.on_port_isolated(lane, IsolationState::AutoIsolated);
        }

        let record = AuditRecord::new(
            AuditCategory::ResourceModify,
            "FabricPortsOrch",
            format!("auto_isolate_port: lane {}", lane),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("lane_{}", lane))
        .with_object_type("fabric_port")
        .with_details(serde_json::json!({
            "isolation_state": "AutoIsolated",
            "sai_oid": format!("{:#x}", port.sai_oid),
            "error_threshold": self.config.auto_isolate_threshold,
        }));
        audit_log!(record);

        port.isolation = IsolationState::AutoIsolated;
        self.stats.auto_isolations += 1;

        Ok(())
    }

    /// Manually isolate a port (config-driven).
    pub fn config_isolate_port(&mut self, lane: u32) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        if port.isolation == IsolationState::ConfigIsolated {
            return Ok(());
        }

        if let Some(ref callbacks) = self.callbacks {
            callbacks.set_isolation(port.sai_oid, true)?;
            callbacks.on_port_isolated(lane, IsolationState::ConfigIsolated);
        }

        let record = AuditRecord::new(
            AuditCategory::ResourceModify,
            "FabricPortsOrch",
            format!("config_isolate_port: lane {}", lane),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("lane_{}", lane))
        .with_object_type("fabric_port")
        .with_details(serde_json::json!({
            "isolation_state": "ConfigIsolated",
            "sai_oid": format!("{:#x}", port.sai_oid),
        }));
        audit_log!(record);

        port.isolation = IsolationState::ConfigIsolated;
        self.stats.config_isolations += 1;

        Ok(())
    }

    /// Permanently isolate a port (hardware failure).
    pub fn perm_isolate_port(&mut self, lane: u32) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        if let Some(ref callbacks) = self.callbacks {
            callbacks.set_isolation(port.sai_oid, true)?;
            callbacks.on_port_isolated(lane, IsolationState::PermIsolated);
        }

        port.isolation = IsolationState::PermIsolated;

        Ok(())
    }

    /// Recover a port from auto-isolation.
    fn recover_port(&mut self, lane: u32) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        // Only recover from auto-isolation
        if port.isolation != IsolationState::AutoIsolated {
            return Ok(());
        }

        if let Some(ref callbacks) = self.callbacks {
            callbacks.set_isolation(port.sai_oid, false)?;
            callbacks.on_port_recovered(lane);
        }

        port.isolation = IsolationState::Active;
        self.stats.recoveries += 1;

        Ok(())
    }

    /// Manually recover a port (remove config isolation).
    pub fn config_recover_port(&mut self, lane: u32) -> Result<()> {
        let port = self
            .ports
            .get_mut(&lane)
            .ok_or(FabricPortsOrchError::PortNotFound(lane))?;

        // Can't recover from permanent isolation
        if port.isolation == IsolationState::PermIsolated {
            return Ok(());
        }

        if port.isolation == IsolationState::Active {
            return Ok(());
        }

        if let Some(ref callbacks) = self.callbacks {
            callbacks.set_isolation(port.sai_oid, false)?;
            callbacks.on_port_recovered(lane);
        }

        port.isolation = IsolationState::Active;
        self.stats.recoveries += 1;

        Ok(())
    }

    /// Get port isolation state.
    pub fn get_isolation(&self, lane: u32) -> Option<IsolationState> {
        self.ports.get(&lane).map(|p| p.isolation)
    }

    /// Get all ports with a specific isolation state.
    pub fn get_ports_by_isolation(&self, state: IsolationState) -> Vec<u32> {
        self.ports
            .iter()
            .filter(|(_, port)| port.isolation == state)
            .map(|(lane, _)| *lane)
            .collect()
    }

    /// Get isolated port count.
    pub fn isolated_port_count(&self) -> usize {
        self.ports
            .values()
            .filter(|p| p.isolation != IsolationState::Active)
            .count()
    }

    // ===== Polling =====

    /// Poll all ports for status updates.
    pub fn poll_ports(&mut self) -> Result<()> {
        if !self.config.monitoring_enabled {
            return Ok(());
        }

        let lanes: Vec<u32> = self.ports.keys().cloned().collect();

        for lane in lanes {
            // Collect SAI OID first
            let sai_oid = match self.ports.get(&lane) {
                Some(port) => port.sai_oid,
                None => continue,
            };

            // Get link status and error counters from callbacks
            let (link_status, error_count) = if let Some(ref callbacks) = self.callbacks {
                let status = callbacks.get_link_status(sai_oid).ok();
                let errors = callbacks.get_error_counters(sai_oid).ok();
                (status, errors)
            } else {
                (None, None)
            };

            // Now update with mutable borrow
            if let Some(status) = link_status {
                let _ = self.update_link_status(lane, status);
            }

            if let Some(errors) = error_count {
                if errors > 0 {
                    let _ = self.record_error(lane);
                } else {
                    let _ = self.record_success(lane);
                }
            }
        }

        self.stats.poll_cycles += 1;
        Ok(())
    }

    /// Update monitoring configuration.
    pub fn update_config(&mut self, new_config: FabricPortsOrchConfig) {
        self.config = new_config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock callbacks for testing without SAI.
    struct MockFabricPortsCallbacks;

    impl FabricPortsOrchCallbacks for MockFabricPortsCallbacks {
        fn get_fabric_port_oid(&self, lane: u32) -> Result<RawSaiObjectId> {
            Ok(0x3000 + lane as u64)
        }

        fn get_link_status(&self, _oid: RawSaiObjectId) -> Result<LinkStatus> {
            Ok(LinkStatus::Up)
        }

        fn get_error_counters(&self, _oid: RawSaiObjectId) -> Result<u64> {
            Ok(0)
        }

        fn set_isolation(&self, _oid: RawSaiObjectId, _isolate: bool) -> Result<()> {
            Ok(())
        }

        fn write_state_db(&self, _lane: u32, _state: &FabricPortState) -> Result<()> {
            Ok(())
        }

        fn remove_state_db(&self, _lane: u32) -> Result<()> {
            Ok(())
        }

        fn on_link_status_changed(
            &self,
            _lane: u32,
            _old_status: LinkStatus,
            _new_status: LinkStatus,
        ) {
        }
        fn on_port_isolated(&self, _lane: u32, _reason: IsolationState) {}
        fn on_port_recovered(&self, _lane: u32) {}
    }

    #[test]
    fn test_fabric_ports_orch_new() {
        let orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());
        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.stats.ports_monitored, 0);
    }

    #[test]
    fn test_get_port_not_found() {
        let orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());
        let result = orch.get_port(0);
        assert!(result.is_none());
    }

    #[test]
    fn test_add_port() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        let result = orch.add_port(0);
        assert!(result.is_ok());
        assert_eq!(orch.port_count(), 1);
        assert_eq!(orch.stats().ports_monitored, 1);

        let port = orch.get_port(0);
        assert!(port.is_some());
        assert_eq!(port.unwrap().lane, 0);
    }

    #[test]
    fn test_add_port_duplicate() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();
        let result = orch.add_port(0);

        assert!(matches!(result, Err(FabricPortsOrchError::PortExists(0))));
    }

    #[test]
    fn test_add_port_invalid_lane() {
        let config = FabricPortsOrchConfig {
            max_lanes: 4,
            ..Default::default()
        };
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> = FabricPortsOrch::new(config);

        let result = orch.add_port(10);
        assert!(matches!(result, Err(FabricPortsOrchError::InvalidLane(10))));
    }

    #[test]
    fn test_remove_port() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();
        let result = orch.remove_port(0);

        assert!(result.is_ok());
        assert_eq!(orch.port_count(), 0);
    }

    #[test]
    fn test_remove_port_not_found() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        let result = orch.remove_port(99);
        assert!(matches!(result, Err(FabricPortsOrchError::PortNotFound(99))));
    }

    #[test]
    fn test_stats_returns_reference() {
        let orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.ports_monitored, 0);
    }

    #[test]
    fn test_fabric_ports_orch_config_default() {
        let config = FabricPortsOrchConfig::default();
        let orch: FabricPortsOrch<MockFabricPortsCallbacks> = FabricPortsOrch::new(config);

        assert!(orch.config().monitoring_enabled);
        assert_eq!(orch.config().poll_interval_ms, 1000);
        assert_eq!(orch.config().auto_isolate_threshold, 10);
        assert_eq!(orch.config().recovery_threshold, 5);
    }

    #[test]
    fn test_multiple_fabric_ports() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        for i in 0..8 {
            orch.add_port(i).unwrap();
        }

        assert_eq!(orch.port_count(), 8);

        for i in 0..8 {
            assert!(orch.get_port(i).is_some());
        }
    }

    // ===== Link Status Tests =====

    #[test]
    fn test_update_link_status() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();

        // Initially down
        assert_eq!(orch.get_port(0).unwrap().status, LinkStatus::Down);

        // Update to up
        let result = orch.update_link_status(0, LinkStatus::Up);
        assert!(result.is_ok());
        assert_eq!(orch.get_port(0).unwrap().status, LinkStatus::Up);
        assert_eq!(orch.stats().ports_up, 1);
    }

    #[test]
    fn test_update_link_status_down() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();
        orch.update_link_status(0, LinkStatus::Up).unwrap();

        // Update to down
        let result = orch.update_link_status(0, LinkStatus::Down);
        assert!(result.is_ok());
        assert_eq!(orch.get_port(0).unwrap().status, LinkStatus::Down);
        assert_eq!(orch.stats().ports_down, 1);
    }

    #[test]
    fn test_get_ports_by_status() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        for i in 0..4 {
            orch.add_port(i).unwrap();
            if i % 2 == 0 {
                orch.update_link_status(i, LinkStatus::Up).unwrap();
            }
        }

        let up_ports = orch.get_ports_by_status(LinkStatus::Up);
        assert_eq!(up_ports.len(), 2);

        let down_ports = orch.get_ports_by_status(LinkStatus::Down);
        assert_eq!(down_ports.len(), 2);
    }

    // ===== Health Monitoring Tests =====

    #[test]
    fn test_record_error() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();

        for _ in 0..5 {
            orch.record_error(0).unwrap();
        }

        let health = orch.get_health(0).unwrap();
        assert_eq!(health.consecutive_polls_with_errors, 5);
        assert_eq!(health.consecutive_polls_with_no_errors, 0);
    }

    #[test]
    fn test_record_success() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();

        for _ in 0..5 {
            orch.record_success(0).unwrap();
        }

        let health = orch.get_health(0).unwrap();
        assert_eq!(health.consecutive_polls_with_no_errors, 5);
        assert_eq!(health.consecutive_polls_with_errors, 0);
    }

    #[test]
    fn test_auto_isolation_threshold() {
        let config = FabricPortsOrchConfig {
            auto_isolate_threshold: 5,
            ..Default::default()
        };
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> = FabricPortsOrch::new(config);

        orch.add_port(0).unwrap();

        // Record errors below threshold
        for _ in 0..4 {
            orch.record_error(0).unwrap();
        }
        assert_eq!(orch.get_isolation(0), Some(IsolationState::Active));

        // Record error at threshold
        orch.record_error(0).unwrap();
        assert_eq!(orch.get_isolation(0), Some(IsolationState::AutoIsolated));
        assert_eq!(orch.stats().auto_isolations, 1);
    }

    #[test]
    fn test_recovery_threshold() {
        let config = FabricPortsOrchConfig {
            auto_isolate_threshold: 3,
            recovery_threshold: 3,
            ..Default::default()
        };
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> = FabricPortsOrch::new(config);

        orch.add_port(0).unwrap();

        // Auto-isolate
        for _ in 0..3 {
            orch.record_error(0).unwrap();
        }
        assert_eq!(orch.get_isolation(0), Some(IsolationState::AutoIsolated));

        // Record successes below threshold
        for _ in 0..2 {
            orch.record_success(0).unwrap();
        }
        assert_eq!(orch.get_isolation(0), Some(IsolationState::AutoIsolated));

        // Record success at threshold - should recover
        orch.record_success(0).unwrap();
        assert_eq!(orch.get_isolation(0), Some(IsolationState::Active));
        assert_eq!(orch.stats().recoveries, 1);
    }

    // ===== Isolation Management Tests =====

    #[test]
    fn test_config_isolate_port() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();

        let result = orch.config_isolate_port(0);
        assert!(result.is_ok());
        assert_eq!(orch.get_isolation(0), Some(IsolationState::ConfigIsolated));
        assert_eq!(orch.stats().config_isolations, 1);
    }

    #[test]
    fn test_perm_isolate_port() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();

        let result = orch.perm_isolate_port(0);
        assert!(result.is_ok());
        assert_eq!(orch.get_isolation(0), Some(IsolationState::PermIsolated));
    }

    #[test]
    fn test_config_recover_port() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();
        orch.config_isolate_port(0).unwrap();

        let result = orch.config_recover_port(0);
        assert!(result.is_ok());
        assert_eq!(orch.get_isolation(0), Some(IsolationState::Active));
    }

    #[test]
    fn test_cannot_recover_perm_isolated() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();
        orch.perm_isolate_port(0).unwrap();

        // Attempt to recover - should not change state
        orch.config_recover_port(0).unwrap();
        assert_eq!(orch.get_isolation(0), Some(IsolationState::PermIsolated));
    }

    #[test]
    fn test_get_ports_by_isolation() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        for i in 0..4 {
            orch.add_port(i).unwrap();
        }

        orch.config_isolate_port(1).unwrap();
        orch.config_isolate_port(2).unwrap();
        orch.perm_isolate_port(3).unwrap();

        let active = orch.get_ports_by_isolation(IsolationState::Active);
        assert_eq!(active.len(), 1);

        let config_isolated = orch.get_ports_by_isolation(IsolationState::ConfigIsolated);
        assert_eq!(config_isolated.len(), 2);

        let perm_isolated = orch.get_ports_by_isolation(IsolationState::PermIsolated);
        assert_eq!(perm_isolated.len(), 1);
    }

    #[test]
    fn test_isolated_port_count() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        for i in 0..5 {
            orch.add_port(i).unwrap();
        }

        assert_eq!(orch.isolated_port_count(), 0);

        orch.config_isolate_port(0).unwrap();
        orch.config_isolate_port(1).unwrap();

        assert_eq!(orch.isolated_port_count(), 2);
    }

    // ===== Legacy Tests (adapted) =====

    #[test]
    fn test_fabric_port_isolation_states() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();
        orch.add_port(1).unwrap();
        orch.add_port(2).unwrap();
        orch.add_port(3).unwrap();

        // Set different isolation states
        orch.config_isolate_port(1).unwrap();
        orch.perm_isolate_port(2).unwrap();

        // Manually set auto-isolated for test
        orch.get_port_mut(3).unwrap().isolation = IsolationState::AutoIsolated;

        assert_eq!(orch.get_isolation(0), Some(IsolationState::Active));
        assert_eq!(orch.get_isolation(1), Some(IsolationState::ConfigIsolated));
        assert_eq!(orch.get_isolation(2), Some(IsolationState::PermIsolated));
        assert_eq!(orch.get_isolation(3), Some(IsolationState::AutoIsolated));
    }

    #[test]
    fn test_fabric_port_health_state() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        orch.add_port(0).unwrap();

        // Record some errors then successes
        for _ in 0..5 {
            orch.record_error(0).unwrap();
        }
        for _ in 0..10 {
            orch.record_success(0).unwrap();
        }

        let health = orch.get_health(0).unwrap();
        assert_eq!(health.consecutive_polls_with_errors, 0);
        assert_eq!(health.consecutive_polls_with_no_errors, 10);
    }

    #[test]
    fn test_fabric_ports_stats_structure() {
        let stats = FabricPortsOrchStats::default();
        assert_eq!(stats.ports_monitored, 0);
        assert_eq!(stats.ports_up, 0);
        assert_eq!(stats.ports_down, 0);
        assert_eq!(stats.auto_isolations, 0);
        assert_eq!(stats.config_isolations, 0);
        assert_eq!(stats.recoveries, 0);
    }

    #[test]
    fn test_fabric_ports_error_variants() {
        let err1 = FabricPortsOrchError::PortNotFound(42);
        let err2 = FabricPortsOrchError::PortExists(42);
        let err3 = FabricPortsOrchError::InvalidLane(99);
        let err4 = FabricPortsOrchError::SaiError("test".to_string());

        match err1 {
            FabricPortsOrchError::PortNotFound(port_id) => {
                assert_eq!(port_id, 42);
            }
            _ => panic!("Wrong error variant"),
        }

        match err2 {
            FabricPortsOrchError::PortExists(port_id) => {
                assert_eq!(port_id, 42);
            }
            _ => panic!("Wrong error variant"),
        }

        match err3 {
            FabricPortsOrchError::InvalidLane(lane) => {
                assert_eq!(lane, 99);
            }
            _ => panic!("Wrong error variant"),
        }

        match err4 {
            FabricPortsOrchError::SaiError(msg) => {
                assert_eq!(msg, "test");
            }
            _ => panic!("Wrong error variant"),
        }
    }

    #[test]
    fn test_with_callbacks() {
        let callbacks = Arc::new(MockFabricPortsCallbacks);
        let mut orch = FabricPortsOrch::with_callbacks(FabricPortsOrchConfig::default(), callbacks);

        orch.add_port(0).unwrap();
        assert_eq!(orch.port_count(), 1);
    }

    #[test]
    fn test_update_config() {
        let mut orch: FabricPortsOrch<MockFabricPortsCallbacks> =
            FabricPortsOrch::new(FabricPortsOrchConfig::default());

        let new_config = FabricPortsOrchConfig {
            monitoring_enabled: false,
            poll_interval_ms: 5000,
            auto_isolate_threshold: 20,
            recovery_threshold: 10,
            max_lanes: 16,
        };

        orch.update_config(new_config);

        assert!(!orch.config().monitoring_enabled);
        assert_eq!(orch.config().poll_interval_ms, 5000);
        assert_eq!(orch.config().auto_isolate_threshold, 20);
    }
}
