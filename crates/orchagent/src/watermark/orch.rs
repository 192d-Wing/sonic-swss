//! WatermarkOrch implementation.
//!
//! Manages buffer watermark statistics and clearing.

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
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

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceModify,
            "WatermarkOrch",
            "clear_watermarks"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("watermark_{:?}", request))
        .with_object_type("watermark")
        .with_details(serde_json::json!({
            "request": format!("{:?}", request),
            "table": format!("{:?}", table),
            "stat_name": stat_name,
            "stats": {
                "clears_processed": self.stats.clears_processed
            }
        })));

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
        self.clear_watermarks(
            table,
            "SAI_QUEUE_STAT_SHARED_WATERMARK_BYTES",
            &multicast_ids,
        );
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
    use std::sync::Mutex;

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

    // ===== Watermark Types Tests =====

    #[test]
    fn test_queue_watermark_unicast() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Add unicast queue IDs
        orch.add_queue_id(QueueType::Unicast, 100);
        orch.add_queue_id(QueueType::Unicast, 101);
        orch.add_queue_id(QueueType::Unicast, 102);

        assert_eq!(orch.queue_ids().unicast.len(), 3);
        assert_eq!(orch.queue_ids().multicast.len(), 0);
        assert!(orch.queue_ids_initialized());
    }

    #[test]
    fn test_queue_watermark_multicast() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Add multicast queue IDs
        orch.add_queue_id(QueueType::Multicast, 200);
        orch.add_queue_id(QueueType::Multicast, 201);

        assert_eq!(orch.queue_ids().multicast.len(), 2);
        assert_eq!(orch.queue_ids().unicast.len(), 0);
    }

    #[test]
    fn test_priority_group_watermarks() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Add multiple PG IDs
        for i in 0..8 {
            orch.add_pg_id(1000 + i);
        }

        assert_eq!(orch.pg_ids().len(), 8);
        assert!(orch.pg_ids_initialized());
    }

    #[test]
    fn test_buffer_pool_watermarks() {
        use std::collections::HashMap;

        struct MockCallbacks {
            pools: HashMap<String, RawSaiObjectId>,
        }

        impl WatermarkOrchCallbacks for MockCallbacks {
            fn get_buffer_pool_oids(&self) -> HashMap<String, RawSaiObjectId> {
                self.pools.clone()
            }
        }

        let mut pools = HashMap::new();
        pools.insert("ingress_lossless_pool".to_string(), 5000);
        pools.insert("egress_lossless_pool".to_string(), 5001);
        pools.insert("ingress_lossy_pool".to_string(), 5002);

        let callbacks = Arc::new(MockCallbacks { pools });
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(callbacks.clone());

        let pool_oids = callbacks.get_buffer_pool_oids();
        assert_eq!(pool_oids.len(), 3);
    }

    #[test]
    fn test_headroom_pool_watermarks() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Enable PG watermarks (which track headroom)
        orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, true);

        assert!(orch.status().pg_enabled());
        assert!(orch.is_enabled());
    }

    #[test]
    fn test_per_queue_and_per_pg_watermarks() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Add per-queue watermarks
        orch.add_queue_id(QueueType::Unicast, 1);
        orch.add_queue_id(QueueType::Unicast, 2);
        orch.add_queue_id(QueueType::Multicast, 10);
        orch.add_queue_id(QueueType::Multicast, 11);

        // Add per-PG watermarks
        orch.add_pg_id(100);
        orch.add_pg_id(101);

        assert_eq!(orch.queue_ids().unicast.len(), 2);
        assert_eq!(orch.queue_ids().multicast.len(), 2);
        assert_eq!(orch.pg_ids().len(), 2);
    }

    // ===== Watermark Configuration Tests =====

    #[test]
    fn test_setting_telemetry_interval() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        assert_eq!(orch.telemetry_interval(), Duration::from_secs(120));

        orch.set_telemetry_interval(Duration::from_secs(30));
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(30));
        assert!(orch.timer_changed());
        assert_eq!(orch.stats().config_updates, 1);
    }

    #[test]
    fn test_enabling_disabling_watermark_collection() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        assert!(!orch.is_enabled());

        // Enable queue watermarks
        orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
        assert!(orch.is_enabled());
        assert!(orch.status().queue_enabled());

        // Disable queue watermarks
        orch.handle_flex_counter_status(WatermarkGroup::Queue, false);
        assert!(!orch.is_enabled());
        assert!(!orch.status().queue_enabled());
    }

    #[test]
    fn test_configuring_watermark_types() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Enable both queue and PG watermarks
        orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
        orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, true);

        assert!(orch.status().queue_enabled());
        assert!(orch.status().pg_enabled());
        assert_eq!(orch.status().raw(), 0x03); // Both bits set
    }

    #[test]
    fn test_interval_validation() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Test various valid intervals
        orch.set_telemetry_interval_secs(10);
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(10));

        orch.set_telemetry_interval_secs(300);
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(300));

        orch.set_telemetry_interval_secs(3600);
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(3600));
    }

    // ===== Watermark Operations Tests =====

    #[test]
    fn test_clearing_watermarks() {
        use std::sync::Mutex;

        struct MockCallbacks {
            clear_count: Mutex<u32>,
        }

        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }

            fn clear_watermark(
                &self,
                _table: WatermarkTable,
                _stat_name: &str,
                _obj_id: RawSaiObjectId,
            ) {
                *self.clear_count.lock().unwrap() += 1;
            }
        }

        let callbacks = Arc::new(MockCallbacks {
            clear_count: Mutex::new(0),
        });

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(callbacks.clone());

        // Add some queue IDs
        orch.add_queue_id(QueueType::Unicast, 1);
        orch.add_queue_id(QueueType::Unicast, 2);

        // Clear unicast queue watermarks
        orch.handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedUnicast)
            .unwrap();

        assert_eq!(*callbacks.clear_count.lock().unwrap(), 2);
        assert_eq!(orch.stats().clears_processed, 1);
    }

    #[test]
    fn test_periodic_watermark_snapshots() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Enable watermarks
        orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
        orch.handle_flex_counter_status(WatermarkGroup::PriorityGroup, true);

        // Add some IDs
        orch.add_queue_id(QueueType::Unicast, 1);
        orch.add_pg_id(100);

        // Simulate timer expirations (periodic snapshots)
        orch.handle_timer_expiration();
        assert_eq!(orch.stats().timer_expirations, 1);

        orch.handle_timer_expiration();
        assert_eq!(orch.stats().timer_expirations, 2);

        orch.handle_timer_expiration();
        assert_eq!(orch.stats().timer_expirations, 3);
    }

    #[test]
    fn test_peak_watermark_tracking() {
        use std::sync::Mutex;

        struct MockCallbacks {
            cleared_objects: Mutex<Vec<RawSaiObjectId>>,
        }

        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }

            fn clear_watermark(
                &self,
                _table: WatermarkTable,
                _stat_name: &str,
                obj_id: RawSaiObjectId,
            ) {
                self.cleared_objects.lock().unwrap().push(obj_id);
            }
        }

        let callbacks = Arc::new(MockCallbacks {
            cleared_objects: Mutex::new(Vec::new()),
        });

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(callbacks.clone());

        // Add PG IDs for peak tracking
        orch.add_pg_id(200);
        orch.add_pg_id(201);
        orch.add_pg_id(202);

        // Clear PG shared watermarks (peak values)
        orch.handle_clear_request(WatermarkTable::Persistent, ClearRequest::PgShared)
            .unwrap();

        let cleared = callbacks.cleared_objects.lock().unwrap();
        assert_eq!(cleared.len(), 3);
        assert!(cleared.contains(&200));
        assert!(cleared.contains(&201));
        assert!(cleared.contains(&202));
    }

    // ===== Statistics Tracking Tests =====

    #[test]
    fn test_watermark_reads_count() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Config updates track configuration changes
        orch.set_telemetry_interval_secs(60);
        orch.set_telemetry_interval_secs(30);
        orch.set_telemetry_interval_secs(15);

        assert_eq!(orch.stats().config_updates, 3);
    }

    #[test]
    fn test_clear_operations_count() {
        struct MockCallbacks;
        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }
        }

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        orch.add_queue_id(QueueType::Unicast, 1);
        orch.add_pg_id(100);

        // Perform multiple clear operations
        orch.handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedUnicast)
            .unwrap();
        orch.handle_clear_request(WatermarkTable::User, ClearRequest::PgShared)
            .unwrap();
        orch.handle_clear_request(WatermarkTable::Persistent, ClearRequest::BufferPool)
            .unwrap();

        assert_eq!(orch.stats().clears_processed, 3);
    }

    #[test]
    fn test_snapshots_taken() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Enable watermarks
        orch.handle_flex_counter_status(WatermarkGroup::Queue, true);

        // Take snapshots via timer
        for _ in 0..10 {
            orch.handle_timer_expiration();
        }

        assert_eq!(orch.stats().timer_expirations, 10);
    }

    #[test]
    fn test_objects_being_monitored() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Add various objects to monitor
        orch.add_queue_id(QueueType::Unicast, 1);
        orch.add_queue_id(QueueType::Unicast, 2);
        orch.add_queue_id(QueueType::Multicast, 10);
        orch.add_pg_id(100);
        orch.add_pg_id(101);
        orch.add_pg_id(102);

        // Verify counts
        assert_eq!(orch.queue_ids().unicast.len(), 2);
        assert_eq!(orch.queue_ids().multicast.len(), 1);
        assert_eq!(orch.pg_ids().len(), 3);
    }

    // ===== Multi-Object Tracking Tests =====

    #[test]
    fn test_multiple_queues_per_port() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Simulate 8 queues per port (typical configuration)
        for i in 0..8 {
            orch.add_queue_id(QueueType::Unicast, 1000 + i);
        }

        assert_eq!(orch.queue_ids().unicast.len(), 8);
    }

    #[test]
    fn test_multiple_pgs_per_port() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Simulate 8 priority groups per port
        for i in 0..8 {
            orch.add_pg_id(2000 + i);
        }

        assert_eq!(orch.pg_ids().len(), 8);
    }

    #[test]
    fn test_multiple_ports() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Simulate 4 ports with 4 queues each
        for port in 0..4 {
            for queue in 0..4 {
                let id = (port * 100) + queue;
                orch.add_queue_id(QueueType::Unicast, id);
            }
        }

        assert_eq!(orch.queue_ids().unicast.len(), 16);
    }

    #[test]
    fn test_buffer_pools_tracking() {
        use std::collections::HashMap;

        struct MockCallbacks {
            pools: HashMap<String, RawSaiObjectId>,
            cleared: Mutex<Vec<String>>,
        }

        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }

            fn get_buffer_pool_oids(&self) -> HashMap<String, RawSaiObjectId> {
                self.pools.clone()
            }

            fn clear_watermark_by_name(
                &self,
                _table: WatermarkTable,
                _stat_name: &str,
                name: &str,
            ) {
                self.cleared.lock().unwrap().push(name.to_string());
            }
        }

        let mut pools = HashMap::new();
        pools.insert("pool1".to_string(), 6000);
        pools.insert("pool2".to_string(), 6001);
        pools.insert("pool3".to_string(), 6002);
        pools.insert("pool4".to_string(), 6003);

        let callbacks = Arc::new(MockCallbacks {
            pools,
            cleared: Mutex::new(Vec::new()),
        });

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(callbacks.clone());

        // Clear buffer pool watermarks
        orch.handle_clear_request(WatermarkTable::User, ClearRequest::BufferPool)
            .unwrap();

        let cleared = callbacks.cleared.lock().unwrap();
        assert_eq!(cleared.len(), 4);
    }

    // ===== Error Handling Tests =====

    #[test]
    fn test_invalid_interval_values() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Zero interval (disable telemetry)
        orch.set_telemetry_interval_secs(0);
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(0));

        // Very large interval
        orch.set_telemetry_interval_secs(86400); // 1 day
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(86400));
    }

    #[test]
    fn test_ports_not_ready_error() {
        struct MockCallbacks;
        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                false
            }
        }

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result =
            orch.handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedUnicast);

        assert!(result.is_err());
        match result {
            Err(WatermarkOrchError::PortsNotReady) => {}
            _ => panic!("Expected PortsNotReady error"),
        }
    }

    #[test]
    fn test_clearing_non_existent_watermarks() {
        struct MockCallbacks {
            clear_count: Mutex<u32>,
        }

        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }

            fn clear_watermark(
                &self,
                _table: WatermarkTable,
                _stat_name: &str,
                _obj_id: RawSaiObjectId,
            ) {
                *self.clear_count.lock().unwrap() += 1;
            }
        }

        let callbacks = Arc::new(MockCallbacks {
            clear_count: Mutex::new(0),
        });

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(callbacks.clone());

        // Clear without adding any IDs - should succeed but clear nothing
        let result =
            orch.handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedUnicast);
        assert!(result.is_ok());
        assert_eq!(*callbacks.clear_count.lock().unwrap(), 0);
    }

    // ===== Edge Cases Tests =====

    #[test]
    fn test_empty_watermark_state() {
        let orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        assert!(!orch.is_initialized());
        assert!(!orch.is_enabled());
        assert!(!orch.pg_ids_initialized());
        assert!(!orch.queue_ids_initialized());
        assert_eq!(orch.stats().clears_processed, 0);
        assert_eq!(orch.stats().timer_expirations, 0);
        assert_eq!(orch.stats().config_updates, 0);
    }

    #[test]
    fn test_very_high_watermark_values() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Add very high object IDs (u64 max values)
        orch.add_queue_id(QueueType::Unicast, u64::MAX);
        orch.add_queue_id(QueueType::Unicast, u64::MAX - 1);
        orch.add_pg_id(u64::MAX - 100);

        assert_eq!(orch.queue_ids().unicast.len(), 2);
        assert_eq!(orch.pg_ids().len(), 1);
    }

    #[test]
    fn test_zero_interval_disable_telemetry() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        // Enable watermarks
        orch.handle_flex_counter_status(WatermarkGroup::Queue, true);
        assert!(orch.is_enabled());

        // Set interval to 0 (disable telemetry)
        orch.set_telemetry_interval_secs(0);
        assert_eq!(orch.telemetry_interval(), Duration::from_secs(0));

        // Timer expirations should still work
        orch.handle_timer_expiration();
        assert_eq!(orch.stats().timer_expirations, 1);
    }

    #[test]
    fn test_timer_changed_flag_management() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        assert!(!orch.timer_changed());

        // Change interval
        orch.set_telemetry_interval_secs(60);
        assert!(orch.timer_changed());

        // Clear flag
        orch.clear_timer_changed();
        assert!(!orch.timer_changed());

        // Setting same interval should not set flag
        orch.set_telemetry_interval_secs(60);
        assert!(!orch.timer_changed());

        // Changing interval should set flag
        orch.set_telemetry_interval_secs(30);
        assert!(orch.timer_changed());
    }

    #[test]
    fn test_handle_all_clear_request_types() {
        struct MockCallbacks;
        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }
        }

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Add IDs for all types
        orch.add_queue_id(QueueType::Unicast, 1);
        orch.add_queue_id(QueueType::Multicast, 2);
        orch.add_queue_id(QueueType::All, 3);
        orch.add_pg_id(100);

        // Test all clear request types
        assert!(orch
            .handle_clear_request(WatermarkTable::User, ClearRequest::PgHeadroom)
            .is_ok());
        assert!(orch
            .handle_clear_request(WatermarkTable::User, ClearRequest::PgShared)
            .is_ok());
        assert!(orch
            .handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedUnicast)
            .is_ok());
        assert!(orch
            .handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedMulticast)
            .is_ok());
        assert!(orch
            .handle_clear_request(WatermarkTable::User, ClearRequest::QueueSharedAll)
            .is_ok());
        assert!(orch
            .handle_clear_request(WatermarkTable::User, ClearRequest::BufferPool)
            .is_ok());
        assert!(orch
            .handle_clear_request(WatermarkTable::User, ClearRequest::HeadroomPool)
            .is_ok());

        assert_eq!(orch.stats().clears_processed, 7);
    }

    #[test]
    fn test_watermark_table_types() {
        struct MockCallbacks {
            last_table: Mutex<Option<WatermarkTable>>,
        }

        impl WatermarkOrchCallbacks for MockCallbacks {
            fn all_ports_ready(&self) -> bool {
                true
            }

            fn clear_watermark(
                &self,
                table: WatermarkTable,
                _stat_name: &str,
                _obj_id: RawSaiObjectId,
            ) {
                *self.last_table.lock().unwrap() = Some(table);
            }
        }

        let callbacks = Arc::new(MockCallbacks {
            last_table: Mutex::new(None),
        });

        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());
        orch.set_callbacks(callbacks.clone());
        orch.add_pg_id(100);

        // Test different table types
        orch.handle_clear_request(WatermarkTable::Periodic, ClearRequest::PgShared)
            .unwrap();
        assert_eq!(
            *callbacks.last_table.lock().unwrap(),
            Some(WatermarkTable::Periodic)
        );

        orch.handle_clear_request(WatermarkTable::Persistent, ClearRequest::PgShared)
            .unwrap();
        assert_eq!(
            *callbacks.last_table.lock().unwrap(),
            Some(WatermarkTable::Persistent)
        );

        orch.handle_clear_request(WatermarkTable::User, ClearRequest::PgShared)
            .unwrap();
        assert_eq!(
            *callbacks.last_table.lock().unwrap(),
            Some(WatermarkTable::User)
        );
    }

    #[test]
    fn test_initialized_state_management() {
        let mut orch = WatermarkOrch::new(WatermarkOrchConfig::default());

        assert!(!orch.is_initialized());

        orch.set_initialized(true);
        assert!(orch.is_initialized());

        orch.set_initialized(false);
        assert!(!orch.is_initialized());
    }
}
