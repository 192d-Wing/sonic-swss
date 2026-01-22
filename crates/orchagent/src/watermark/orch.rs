//! WatermarkOrch implementation.
//!
//! Manages buffer watermark statistics and clearing.

use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::types::{
    ClearRequest, QueueIds, QueueType, WatermarkConfig, WatermarkGroup, WatermarkStatus,
    WatermarkTable, DEFAULT_TELEMETRY_INTERVAL,
};

/// Error type for WatermarkOrch operations.
#[derive(Debug, Clone)]
pub enum WatermarkOrchError {
    /// Unknown clear request.
    UnknownClearRequest(String),
    /// Unknown watermark table.
    UnknownTable(String),
    /// Ports not ready.
    PortsNotReady,
    /// Callback error.
    CallbackError(String),
}

impl std::fmt::Display for WatermarkOrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownClearRequest(req) => write!(f, "Unknown clear request: {}", req),
            Self::UnknownTable(table) => write!(f, "Unknown watermark table: {}", table),
            Self::PortsNotReady => write!(f, "Ports not ready"),
            Self::CallbackError(msg) => write!(f, "Callback error: {}", msg),
        }
    }
}

impl std::error::Error for WatermarkOrchError {}

/// Callbacks for WatermarkOrch operations.
pub trait WatermarkOrchCallbacks: Send + Sync {
    /// Returns true if all ports are ready.
    fn all_ports_ready(&self) -> bool {
        true
    }

    /// Clears a watermark value for an object ID.
    fn clear_watermark(&self, _table: WatermarkTable, _stat_name: &str, _obj_id: RawSaiObjectId) {}

    /// Clears a watermark value for a named object (buffer pool).
    fn clear_watermark_by_name(&self, _table: WatermarkTable, _stat_name: &str, _name: &str) {}

    /// Gets the buffer pool name to OID mapping.
    fn get_buffer_pool_oids(&self) -> HashMap<String, RawSaiObjectId> {
        HashMap::new()
    }
}

/// Configuration for WatermarkOrch.
#[derive(Debug, Clone)]
pub struct WatermarkOrchConfig {
    /// Telemetry interval.
    pub telemetry_interval: Duration,
}

impl Default for WatermarkOrchConfig {
    fn default() -> Self {
        Self {
            telemetry_interval: Duration::from_secs(DEFAULT_TELEMETRY_INTERVAL),
        }
    }
}

impl WatermarkOrchConfig {
    /// Creates a new config with the given telemetry interval.
    pub fn new(telemetry_interval: Duration) -> Self {
        Self { telemetry_interval }
    }

    /// Creates a config with interval in seconds.
    pub fn with_interval_secs(secs: u64) -> Self {
        Self {
            telemetry_interval: Duration::from_secs(secs),
        }
    }
}

/// Statistics for WatermarkOrch operations.
#[derive(Debug, Clone, Default)]
pub struct WatermarkOrchStats {
    /// Number of clear requests processed.
    pub clears_processed: u64,
    /// Number of timer expirations.
    pub timer_expirations: u64,
    /// Number of config updates.
    pub config_updates: u64,
}

/// WatermarkOrch - manages buffer watermark statistics.
pub struct WatermarkOrch {
    /// Configuration.
    config: WatermarkOrchConfig,
    /// Callbacks.
    callbacks: Option<Arc<dyn WatermarkOrchCallbacks>>,
    /// Watermark status (which groups are enabled).
    status: WatermarkStatus,
    /// Whether timer interval changed and needs reset.
    timer_changed: bool,
    /// Priority Group IDs.
    pg_ids: Vec<RawSaiObjectId>,
    /// Queue IDs by type.
    queue_ids: QueueIds,
    /// Statistics.
    stats: WatermarkOrchStats,
    /// Initialized flag.
    initialized: bool,
}

impl std::fmt::Debug for WatermarkOrch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatermarkOrch")
            .field("config", &self.config)
            .field("status", &self.status)
            .field("pg_ids_count", &self.pg_ids.len())
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl WatermarkOrch {
    /// Creates a new WatermarkOrch with the given configuration.
    pub fn new(config: WatermarkOrchConfig) -> Self {
        Self {
            config,
            callbacks: None,
            status: WatermarkStatus::new(),
            timer_changed: false,
            pg_ids: Vec::new(),
            queue_ids: QueueIds::new(),
            stats: WatermarkOrchStats::default(),
            initialized: false,
        }
    }

    /// Sets the callbacks.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn WatermarkOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the configuration.
    pub fn config(&self) -> &WatermarkOrchConfig {
        &self.config
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &WatermarkOrchStats {
        &self.stats
    }

    /// Returns true if initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Sets the initialized state.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.initialized = initialized;
    }

    /// Returns the watermark status.
    pub fn status(&self) -> &WatermarkStatus {
        &self.status
    }

    /// Returns true if any watermark collection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.status.any_enabled()
    }

    /// Returns the telemetry interval.
    pub fn telemetry_interval(&self) -> Duration {
        self.config.telemetry_interval
    }

    /// Sets the telemetry interval.
    pub fn set_telemetry_interval(&mut self, interval: Duration) {
        if interval != self.config.telemetry_interval {
            self.config.telemetry_interval = interval;
            self.timer_changed = true;
            self.stats.config_updates += 1;
        }
    }

    /// Sets the telemetry interval from seconds.
    pub fn set_telemetry_interval_secs(&mut self, secs: u64) {
        self.set_telemetry_interval(Duration::from_secs(secs));
    }

    /// Returns true if the timer interval changed.
    pub fn timer_changed(&self) -> bool {
        self.timer_changed
    }

    /// Clears the timer changed flag.
    pub fn clear_timer_changed(&mut self) {
        self.timer_changed = false;
    }

    /// Handles flex counter status update.
    pub fn handle_flex_counter_status(&mut self, group: WatermarkGroup, enabled: bool) -> bool {
        let was_enabled = self.status.any_enabled();

        if enabled {
            self.status.enable(group);
        } else {
            self.status.disable(group);
        }

        self.stats.config_updates += 1;

        // Return true if timer should be started (transition from disabled to enabled)
        !was_enabled && self.status.any_enabled()
    }

    /// Sets the PG IDs.
    pub fn set_pg_ids(&mut self, ids: Vec<RawSaiObjectId>) {
        self.pg_ids = ids;
    }

    /// Adds a PG ID.
    pub fn add_pg_id(&mut self, id: RawSaiObjectId) {
        self.pg_ids.push(id);
    }

    /// Returns the PG IDs.
    pub fn pg_ids(&self) -> &[RawSaiObjectId] {
        &self.pg_ids
    }

    /// Returns true if PG IDs are initialized.
    pub fn pg_ids_initialized(&self) -> bool {
        !self.pg_ids.is_empty()
    }

    /// Sets queue IDs.
    pub fn set_queue_ids(&mut self, ids: QueueIds) {
        self.queue_ids = ids;
    }

    /// Adds a queue ID.
    pub fn add_queue_id(&mut self, queue_type: QueueType, id: RawSaiObjectId) {
        self.queue_ids.add(queue_type, id);
    }

    /// Returns the queue IDs.
    pub fn queue_ids(&self) -> &QueueIds {
        &self.queue_ids
    }

    /// Returns true if queue IDs are initialized.
    pub fn queue_ids_initialized(&self) -> bool {
        !self.queue_ids.is_empty()
    }

    /// Handles a clear request.
    pub fn handle_clear_request(
        &mut self,
        table: WatermarkTable,
        request: ClearRequest,
    ) -> Result<(), WatermarkOrchError> {
        // Check if ports are ready
        if let Some(callbacks) = &self.callbacks {
            if !callbacks.all_ports_ready() {
                return Err(WatermarkOrchError::PortsNotReady);
            }
        }

        let stat_name = request.stat_name();

        match request {
            ClearRequest::PgHeadroom | ClearRequest::PgShared => {
                self.clear_watermarks(table, stat_name, &self.pg_ids.clone());
            }
            ClearRequest::QueueSharedUnicast => {
                let ids = self.queue_ids.unicast.clone();
                self.clear_watermarks(table, stat_name, &ids);
            }
            ClearRequest::QueueSharedMulticast => {
                let ids = self.queue_ids.multicast.clone();
                self.clear_watermarks(table, stat_name, &ids);
            }
            ClearRequest::QueueSharedAll => {
                let ids = self.queue_ids.all.clone();
                self.clear_watermarks(table, stat_name, &ids);
            }
            ClearRequest::BufferPool | ClearRequest::HeadroomPool => {
                self.clear_buffer_pool_watermarks(table, stat_name);
            }
        }

        self.stats.clears_processed += 1;

        Ok(())
    }

    /// Handles timer expiration (periodic watermark clearing).
    pub fn handle_timer_expiration(&mut self) {
        // Reset timer if interval changed
        if self.timer_changed {
            self.timer_changed = false;
        }

        // Don't clear if no watermarks are enabled
        if !self.status.any_enabled() {
            return;
        }

        let table = WatermarkTable::Periodic;

        // Clear PG watermarks
        let pg_ids = self.pg_ids.clone();
        self.clear_watermarks(
            table,
            "SAI_INGRESS_PRIORITY_GROUP_STAT_XOFF_ROOM_WATERMARK_BYTES",
            &pg_ids,
        );
        self.clear_watermarks(
            table,
            "SAI_INGRESS_PRIORITY_GROUP_STAT_SHARED_WATERMARK_BYTES",
            &pg_ids,
        );

        // Clear queue watermarks
        let unicast_ids = self.queue_ids.unicast.clone();
        let multicast_ids = self.queue_ids.multicast.clone();
        let all_ids = self.queue_ids.all.clone();

        self.clear_watermarks(table, "SAI_QUEUE_STAT_SHARED_WATERMARK_BYTES", &unicast_ids);
        self.clear_watermarks(table, "SAI_QUEUE_STAT_SHARED_WATERMARK_BYTES", &multicast_ids);
        self.clear_watermarks(table, "SAI_QUEUE_STAT_SHARED_WATERMARK_BYTES", &all_ids);

        // Clear buffer pool watermarks
        self.clear_buffer_pool_watermarks(table, "SAI_BUFFER_POOL_STAT_WATERMARK_BYTES");
        self.clear_buffer_pool_watermarks(table, "SAI_BUFFER_POOL_STAT_XOFF_ROOM_WATERMARK_BYTES");

        self.stats.timer_expirations += 1;
    }

    /// Clears watermarks for a list of object IDs.
    fn clear_watermarks(&self, table: WatermarkTable, stat_name: &str, obj_ids: &[RawSaiObjectId]) {
        if let Some(callbacks) = &self.callbacks {
            for &id in obj_ids {
                callbacks.clear_watermark(table, stat_name, id);
            }
        }
    }

    /// Clears buffer pool watermarks.
    fn clear_buffer_pool_watermarks(&self, table: WatermarkTable, stat_name: &str) {
        if let Some(callbacks) = &self.callbacks {
            let pool_oids = callbacks.get_buffer_pool_oids();
            for (name, oid) in pool_oids {
                callbacks.clear_watermark(table, stat_name, oid);
                // Also clear by name for reference
                callbacks.clear_watermark_by_name(table, stat_name, &name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watermark_orch_new() {
        let orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        assert!(!orch.is_initialized());
        assert!(!orch.is_enabled());
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(120));
    }

    #[test]
    fn test_telemetry_interval() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        orch.set_telemetry_interval_secs(60);
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(60));
        assert!(orch.timer_changed());

        orch.clear_timer_changed();
        assert!(!orch.timer_changed());
    }

    #[test]
    fn test_flex_counter_status() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // First enable returns true (should start timer)
        assert!(orch.handle_flex_counter_status(WatermarkGroup::Queue, true));
        assert!(orch.is_enabled());
        assert!(orch.status().queue_enabled());

        // Second enable returns false (timer already running)
        assert!(!orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, true));
        assert!(orch.status().pg_enabled());

        // Disable one group
        assert!(!orch.handle_flex_counter_status(WatermarkGroup::Queue, false));
        assert!(!orch.status().queue_enabled());
        assert!(orch.is_enabled()); // PG still enabled

        // Disable all
        assert!(!orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, false));
        assert!(!orch.is_enabled());
    }

    #[test]
    fn test_pg_ids() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        assert!(!orch.pg_ids_initialized());

        orch.add_pg_id(1);
        orch.add_pg_id(2);

        assert!(orch.pg_ids_initialized());
        assert_eq!(orch.pg_ids().len(), 2);
    }

    #[test]
    fn test_queue_ids() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        assert!(!orch.queue_ids_initialized());

        orch.add_queue_id(QueueType::Unicast, 1);
        orch.add_queue_id(QueueType::Multicast, 2);
        orch.add_queue_id(QueueType::All, 3);

        assert!(orch.queue_ids_initialized());
        assert_eq!(orch.queue_ids().unicast.len(), 1);
        assert_eq!(orch.queue_ids().multicast.len(), 1);
        assert_eq!(orch.queue_ids().all.len(), 1);
    }

    #[test]
    fn test_handle_timer_expiration() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Timer changed should be cleared
        orch.timer_changed = true;
        orch.handle_timer_expiration();
        assert!(!orch.timer_changed());

        // Enable watermarks and test timer stats
        orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
        orch.handle_timer_expiration();
        assert_eq!(orch.stats().timer_expirations, 1);
    }

    #[test]
    fn test_statistics() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        orch.set_telemetry_interval_secs(60);
        assert_eq!(orch.stats().config_updates, 1);

        orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
        assert_eq!(orch.stats().config_updates, 2);
    }
}
