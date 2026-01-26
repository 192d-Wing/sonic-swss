//! OrchDaemon implementation.
//!
//! The OrchDaemon is the central coordinator for all Orch modules.
//! It manages:
//! - Event loop using Select/epoll
//! - Orch registration and priority ordering
//! - Task dispatch to appropriate Orchs
//! - Warm restart coordination

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use log::{debug, error, info};
use sonic_orch_common::{
    ConsumerConfig, Orch, OrchContext, RedisBoundConsumer, RedisConfig, RedisDatabase,
};
use sonic_sai::{SaiError, SaiResult, SwitchKind, SwitchOid};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for the OrchDaemon.
#[derive(Debug, Clone)]
pub struct OrchDaemonConfig {
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
    /// Batch size for consumer operations
    pub batch_size: usize,
    /// Enable warm boot mode
    pub warm_boot: bool,
    /// Redis host for databases
    pub redis_host: String,
    /// Redis port for databases
    pub redis_port: u16,
}

impl Default for OrchDaemonConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: 1000,
            batch_size: 128,
            warm_boot: false,
            redis_host: "127.0.0.1".to_string(),
            redis_port: 6379,
        }
    }
}

/// The main orchestration daemon.
///
/// OrchDaemon coordinates all Orch modules and runs the main event loop.
pub struct OrchDaemon {
    /// Configuration
    config: OrchDaemonConfig,
    /// Registered Orchs sorted by priority
    orchs: BTreeMap<i32, Vec<Box<dyn Orch>>>,
    /// Shared context
    context: Arc<RwLock<OrchContext>>,
    /// Running flag
    running: bool,
    /// APPL_DB connection for table polling
    appl_db: Option<Arc<RwLock<RedisDatabase>>>,
    /// STATE_DB connection for state writes
    state_db: Option<Arc<RwLock<RedisDatabase>>>,
    /// Redis consumers for key tables
    port_table_consumer: Option<RedisBoundConsumer>,
    intf_table_consumer: Option<RedisBoundConsumer>,
    route_table_consumer: Option<RedisBoundConsumer>,
    /// SAI switch object (OID for the switch abstraction)
    switch_oid: Option<SwitchOid>,
}

impl OrchDaemon {
    /// Creates a new OrchDaemon with the given configuration.
    pub fn new(config: OrchDaemonConfig) -> Self {
        Self {
            config,
            orchs: BTreeMap::new(),
            context: Arc::new(RwLock::new(OrchContext::default())),
            running: false,
            appl_db: None,
            state_db: None,
            port_table_consumer: None,
            intf_table_consumer: None,
            route_table_consumer: None,
            switch_oid: None,
        }
    }

    /// Registers an Orch with the daemon.
    ///
    /// Orchs are ordered by priority (lower = higher priority).
    pub fn register_orch(&mut self, orch: Box<dyn Orch>) {
        let priority = orch.priority();
        let orch_name = orch.name().to_string();
        info!("Registering {} with priority {}", orch_name, priority);

        let record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "OrchDaemon",
            format!("register_orch: {}", orch_name),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&orch_name)
        .with_object_type("orch_module")
        .with_details(serde_json::json!({
            "priority": priority,
        }));
        audit_log!(record);

        self.orchs.entry(priority).or_default().push(orch);
    }

    /// Returns the shared context.
    pub fn context(&self) -> Arc<RwLock<OrchContext>> {
        Arc::clone(&self.context)
    }

    /// Returns a reference to the PORT_TABLE consumer.
    pub fn port_table_consumer(&mut self) -> Option<&mut RedisBoundConsumer> {
        self.port_table_consumer.as_mut()
    }

    /// Returns a reference to the INTF_TABLE consumer.
    pub fn intf_table_consumer(&mut self) -> Option<&mut RedisBoundConsumer> {
        self.intf_table_consumer.as_mut()
    }

    /// Returns a reference to the ROUTE_TABLE consumer.
    pub fn route_table_consumer(&mut self) -> Option<&mut RedisBoundConsumer> {
        self.route_table_consumer.as_mut()
    }

    /// Returns the SAI switch OID (object identifier).
    pub fn switch_oid(&self) -> Option<SwitchOid> {
        self.switch_oid
    }

    /// Initializes all registered Orchs.
    ///
    /// Called during startup before the event loop begins.
    pub async fn init(&mut self) -> bool {
        info!(
            "Initializing OrchDaemon with {} orch groups",
            self.orchs.len()
        );

        let record = AuditRecord::new(
            AuditCategory::SystemLifecycle,
            "OrchDaemon",
            "daemon_initialization_start",
        )
        .with_outcome(AuditOutcome::InProgress)
        .with_details(serde_json::json!({
            "orch_count": self.orchs.len(),
        }));
        audit_log!(record);

        // Initialize SAI (Switch Abstraction Interface)
        // NIST: SC-3 - Access Enforcement via SAI layer
        info!("Initializing SAI (Switch Abstraction Interface)...");
        match self.init_sai().await {
            Ok(()) => {
                info!("SAI initialization successful");
            }
            Err(e) => {
                error!("Failed to initialize SAI: {}", e);
                let fail_record = AuditRecord::new(
                    AuditCategory::SystemLifecycle,
                    "OrchDaemon",
                    "sai_initialization_failed",
                )
                .with_outcome(AuditOutcome::Failure)
                .with_error(format!("SAI initialization failed: {}", e));
                audit_log!(fail_record);
                return false;
            }
        }

        // Create switch object
        // NIST: CM-6 - Configuration Settings (switch configuration)
        info!("Creating switch object...");
        match self.create_switch().await {
            Ok(()) => {
                info!("Switch object created successfully");
            }
            Err(e) => {
                error!("Failed to create switch: {}", e);
                let fail_record = AuditRecord::new(
                    AuditCategory::SystemLifecycle,
                    "OrchDaemon",
                    "switch_creation_failed",
                )
                .with_outcome(AuditOutcome::Failure)
                .with_error(format!("Switch creation failed: {}", e));
                audit_log!(fail_record);
                return false;
            }
        }

        // Initialize database connections
        // NIST: SC-7 - Boundary Protection (database communication)
        info!("Initializing database connections...");
        match self.init_databases().await {
            Ok(()) => {
                info!("Database connections established");
            }
            Err(e) => {
                error!("Failed to initialize databases: {}", e);
                let fail_record = AuditRecord::new(
                    AuditCategory::SystemLifecycle,
                    "OrchDaemon",
                    "database_initialization_failed",
                )
                .with_outcome(AuditOutcome::Failure)
                .with_error(format!("Database initialization failed: {}", e));
                audit_log!(fail_record);
                return false;
            }
        }

        let success_record = AuditRecord::new(
            AuditCategory::SystemLifecycle,
            "OrchDaemon",
            "daemon_initialization_end",
        )
        .with_outcome(AuditOutcome::Success)
        .with_details(serde_json::json!({
            "orch_count": self.orchs.len(),
            "sai_initialized": true,
            "switch_created": true,
            "databases_connected": true,
        }));
        audit_log!(success_record);

        true
    }

    /// Initializes the SAI (Switch Abstraction Interface) layer.
    ///
    /// This step prepares SAI for use by:
    /// - Loading the SAI library implementation
    /// - Initializing the SAI profile with hardware-specific settings
    /// - Creating SAI service methods for API access
    ///
    /// # NIST Controls
    /// - SC-3: Access Enforcement (SAI access control)
    /// - SC-7: Boundary Protection (switch abstraction layer)
    async fn init_sai(&self) -> Result<(), String> {
        info!("Initializing SAI library and profile");

        // TODO: In production implementation, this would:
        // 1. Load SAI library based on switch type (libsai.so)
        // 2. Call sai_api_initialize() to create service methods
        // 3. Set up SAI profile with hardware-specific tuning (packet buffer, QoS, etc.)
        // 4. Enable SAI logging for debug output
        //
        // For now, return success - actual SAI library linking will be in sonic-ffi-bridge
        // with cpp-interop feature enabled.

        debug!("SAI initialization deferred to FFI layer (sonic-ffi-bridge/cpp-link)");
        Ok(())
    }

    /// Creates the switch object and initializes hardware access.
    ///
    /// This is a critical initialization step that:
    /// - Creates the SAI switch object representing the hardware
    /// - Queries hardware capabilities (port count, LAG members, etc.)
    /// - Initializes port lists and default configurations
    /// - Sets up switch attributes for forwarding, LAG, mirroring, etc.
    ///
    /// # NIST Controls
    /// - CM-6: Configuration Settings (switch configuration)
    /// - AC-3: Access Control (switch access control)
    async fn create_switch(&mut self) -> Result<(), String> {
        info!("Creating SAI switch object");

        // TODO: In production implementation, this would:
        // 1. Call sai_switch_api->create_switch() with switch attributes
        // 2. Retrieve switch OID for future operations
        // 3. Query hardware capabilities:
        //    - Port list and capabilities
        //    - CPU port OID
        //    - Available LAG members
        //    - QoS queue capabilities
        //    - ACL table sizes
        // 4. Initialize default switch configuration
        // 5. Store switch OID in OrchContext for access by all modules
        //
        // For now, create a dummy switch OID (zero value in simulation)

        // Create a switch OID (in real implementation, this comes from SAI API)
        // Using a hardcoded value for now - this would be the return from sai_switch_api->create_switch()
        // Raw value 1u64 represents a valid switch in simulation mode
        let dummy_switch_oid = SwitchOid::from_raw_unchecked(1u64);

        debug!("Created switch OID: {:?}", dummy_switch_oid);

        // Store the switch OID for access by other modules
        self.switch_oid = Some(dummy_switch_oid);

        // TODO: Query capabilities and store in context
        // let capabilities = sai_switch_api->get_switch_capabilities(switch_oid)?;
        // Update shared context
        let mut ctx = self.context.write().await;
        ctx.all_ports_ready = false; // Will be set to true after port sync

        Ok(())
    }

    /// Initializes connections to SONiC databases.
    ///
    /// # NIST Controls
    /// - SC-7: Boundary Protection
    /// - SC-8: Transmission Confidentiality
    async fn init_databases(&mut self) -> Result<(), String> {
        let config = &self.config;
        info!(
            "Connecting to Redis databases at {}:{}",
            config.redis_host, config.redis_port
        );

        // Connect to CONFIG_DB (database 4) - for initial configuration loads
        let config_db_config = RedisConfig::config_db(config.redis_host.clone(), config.redis_port);
        match RedisDatabase::new(config_db_config).await {
            Ok(_db) => {
                info!("Connected to CONFIG_DB");
            }
            Err(e) => {
                return Err(format!("Failed to connect to CONFIG_DB: {}", e));
            }
        }

        // Connect to APPL_DB (database 0) - for table polling and event updates
        let appl_db_config = RedisConfig::appl_db(config.redis_host.clone(), config.redis_port);
        match RedisDatabase::new(appl_db_config).await {
            Ok(db) => {
                info!("Connected to APPL_DB");
                self.appl_db = Some(Arc::new(RwLock::new(db)));
            }
            Err(e) => {
                return Err(format!("Failed to connect to APPL_DB: {}", e));
            }
        }

        // Connect to STATE_DB (database 6) - for state writes
        let state_db_config = RedisConfig::state_db(config.redis_host.clone(), config.redis_port);
        match RedisDatabase::new(state_db_config).await {
            Ok(db) => {
                info!("Connected to STATE_DB");
                self.state_db = Some(Arc::new(RwLock::new(db)));
            }
            Err(e) => {
                return Err(format!("Failed to connect to STATE_DB: {}", e));
            }
        }

        // Connect to COUNTER_DB (database 2)
        let counter_db_config =
            RedisConfig::counter_db(config.redis_host.clone(), config.redis_port);
        match RedisDatabase::new(counter_db_config).await {
            Ok(_db) => {
                info!("Connected to COUNTER_DB");
            }
            Err(e) => {
                return Err(format!("Failed to connect to COUNTER_DB: {}", e));
            }
        }

        // Initialize Redis consumers for critical tables
        // These are bound to APPL_DB for event polling
        self.init_redis_consumers()?;

        info!("All database connections initialized successfully");
        Ok(())
    }

    /// Initializes Redis consumers for key tables.
    ///
    /// Creates RedisBoundConsumer instances that integrate with the event loop.
    fn init_redis_consumers(&mut self) -> Result<(), String> {
        if let Some(appl_db) = &self.appl_db {
            info!("Initializing Redis consumers for table polling");

            // PORT_TABLE consumer (priority 0 - highest)
            let port_config = ConsumerConfig::new("PORT_TABLE")
                .with_priority(0)
                .with_batch_size(self.config.batch_size);
            self.port_table_consumer =
                Some(RedisBoundConsumer::new(port_config, Arc::clone(appl_db)));
            info!("  Created PORT_TABLE consumer");

            // INTF_TABLE consumer (priority 5)
            let intf_config = ConsumerConfig::new("INTF_TABLE")
                .with_priority(5)
                .with_batch_size(self.config.batch_size);
            self.intf_table_consumer =
                Some(RedisBoundConsumer::new(intf_config, Arc::clone(appl_db)));
            info!("  Created INTF_TABLE consumer");

            // ROUTE_TABLE consumer (priority 20)
            let route_config = ConsumerConfig::new("ROUTE_TABLE")
                .with_priority(20)
                .with_batch_size(self.config.batch_size);
            self.route_table_consumer =
                Some(RedisBoundConsumer::new(route_config, Arc::clone(appl_db)));
            info!("  Created ROUTE_TABLE consumer");

            Ok(())
        } else {
            Err("APPL_DB not initialized".to_string())
        }
    }

    /// Runs the main event loop.
    ///
    /// This method blocks until `stop()` is called.
    pub async fn run(&mut self) {
        info!("Starting OrchDaemon event loop");
        self.running = true;

        let record = AuditRecord::new(
            AuditCategory::AdminAction,
            "OrchDaemon",
            "event_loop_started",
        )
        .with_outcome(AuditOutcome::Success)
        .with_details(serde_json::json!({
            "heartbeat_interval_ms": self.config.heartbeat_interval_ms,
            "orch_count": self.orchs.len(),
        }));
        audit_log!(record);

        while self.running {
            // Poll Redis consumers for new entries
            // NIST: SI-4 - System Monitoring (event polling)
            debug!("Polling Redis consumers for new entries");
            self.poll_redis_consumers().await;

            // Process tasks from all Orchs in priority order
            for (_priority, orchs) in self.orchs.iter_mut() {
                for orch in orchs.iter_mut() {
                    if orch.has_pending_tasks() {
                        debug!("Processing tasks for {}", orch.name());
                        orch.do_task().await;
                    }
                }
            }

            // Sleep for heartbeat interval
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.heartbeat_interval_ms,
            ))
            .await;
        }

        info!("OrchDaemon event loop stopped");

        let stop_record = AuditRecord::new(
            AuditCategory::AdminAction,
            "OrchDaemon",
            "event_loop_stopped",
        )
        .with_outcome(AuditOutcome::Success);
        audit_log!(stop_record);
    }

    /// Stops the event loop.
    pub fn stop(&mut self) {
        info!("Stopping OrchDaemon");

        let record = AuditRecord::new(AuditCategory::AdminAction, "OrchDaemon", "stop_requested")
            .with_outcome(AuditOutcome::Success);
        audit_log!(record);

        self.running = false;
    }

    /// Prepares for warm boot.
    pub async fn prepare_warm_boot(&mut self) -> bool {
        info!("Preparing for warm boot");

        let record = AuditRecord::new(
            AuditCategory::WarmRestart,
            "OrchDaemon",
            "warm_boot_preparation_start",
        )
        .with_outcome(AuditOutcome::InProgress);
        audit_log!(record);

        for (_priority, orchs) in self.orchs.iter_mut() {
            for orch in orchs.iter_mut() {
                if !orch.bake() {
                    error!("Failed to bake {}", orch.name());

                    let fail_record = AuditRecord::new(
                        AuditCategory::WarmRestart,
                        "OrchDaemon",
                        format!("warm_boot_preparation_failed: {}", orch.name()),
                    )
                    .with_outcome(AuditOutcome::Failure)
                    .with_error(format!("Failed to bake {}", orch.name()));
                    audit_log!(fail_record);

                    return false;
                }
            }
        }

        let success_record = AuditRecord::new(
            AuditCategory::WarmRestart,
            "OrchDaemon",
            "warm_boot_preparation_complete",
        )
        .with_outcome(AuditOutcome::Success);
        audit_log!(success_record);

        true
    }

    /// Called after warm boot APPLY_VIEW.
    pub async fn on_warm_boot_end(&mut self) {
        info!("Warm boot ended, resuming normal operation");

        let record = AuditRecord::new(AuditCategory::WarmRestart, "OrchDaemon", "warm_boot_ended")
            .with_outcome(AuditOutcome::Success);
        audit_log!(record);

        for (_priority, orchs) in self.orchs.iter_mut() {
            for orch in orchs.iter_mut() {
                orch.on_warm_boot_end();
            }
        }

        // Update context
        let mut ctx = self.context.write().await;
        ctx.warm_boot_in_progress = false;
    }

    /// Polls Redis consumers for new entries and makes them available to Orchs.
    ///
    /// This is called in the event loop before Orch task processing.
    /// It populates consumers with entries from Redis, which Orchs then process
    /// during their do_task() calls.
    ///
    /// # NIST Controls
    /// - SI-4: System Monitoring (event polling and detection)
    async fn poll_redis_consumers(&mut self) {
        let batch_size = self.config.batch_size;
        let timeout_secs = 0.1; // 100ms timeout for non-blocking poll

        // Poll PORT_TABLE consumer
        if let Some(consumer) = &mut self.port_table_consumer {
            match consumer.populate_from_redis(batch_size, timeout_secs).await {
                Ok(()) => {
                    if consumer.has_pending() {
                        debug!(
                            "PORT_TABLE consumer has {} pending entries",
                            consumer.consumer().pending_count()
                        );
                    }
                }
                Err(e) => {
                    debug!("Error polling PORT_TABLE: {}", e);
                }
            }
        }

        // Poll INTF_TABLE consumer
        if let Some(consumer) = &mut self.intf_table_consumer {
            match consumer.populate_from_redis(batch_size, timeout_secs).await {
                Ok(()) => {
                    if consumer.has_pending() {
                        debug!(
                            "INTF_TABLE consumer has {} pending entries",
                            consumer.consumer().pending_count()
                        );
                    }
                }
                Err(e) => {
                    debug!("Error polling INTF_TABLE: {}", e);
                }
            }
        }

        // Poll ROUTE_TABLE consumer
        if let Some(consumer) = &mut self.route_table_consumer {
            match consumer.populate_from_redis(batch_size, timeout_secs).await {
                Ok(()) => {
                    if consumer.has_pending() {
                        debug!(
                            "ROUTE_TABLE consumer has {} pending entries",
                            consumer.consumer().pending_count()
                        );
                    }
                }
                Err(e) => {
                    debug!("Error polling ROUTE_TABLE: {}", e);
                }
            }
        }
    }

    /// Dumps state for debugging.
    pub fn dump(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("OrchDaemon running: {}", self.running));

        for (priority, orchs) in &self.orchs {
            for orch in orchs {
                lines.push(format!(
                    "  [{:3}] {} - {} pending",
                    priority,
                    orch.name(),
                    orch.dump_pending_tasks().len()
                ));
            }
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc as StdArc;

    struct TestOrch {
        name: String,
        priority: i32,
        task_count: StdArc<AtomicU32>,
        has_pending: bool,
    }

    impl TestOrch {
        fn new(name: &str, priority: i32) -> Self {
            Self {
                name: name.to_string(),
                priority,
                task_count: StdArc::new(AtomicU32::new(0)),
                has_pending: false,
            }
        }

        fn with_pending(mut self) -> Self {
            self.has_pending = true;
            self
        }
    }

    #[async_trait]
    impl Orch for TestOrch {
        fn name(&self) -> &str {
            &self.name
        }

        async fn do_task(&mut self) {
            self.task_count.fetch_add(1, Ordering::SeqCst);
        }

        fn priority(&self) -> i32 {
            self.priority
        }

        fn has_pending_tasks(&self) -> bool {
            self.has_pending
        }
    }

    // ============================================================================
    // 1. Configuration Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_default_config() {
        let config = OrchDaemonConfig::default();
        assert_eq!(config.heartbeat_interval_ms, 1000);
        assert_eq!(config.batch_size, 128);
        assert!(!config.warm_boot);
    }

    #[tokio::test]
    async fn test_orchdaemon_custom_config() {
        let config = OrchDaemonConfig {
            heartbeat_interval_ms: 500,
            batch_size: 256,
            warm_boot: true,
            redis_host: "localhost".to_string(),
            redis_port: 6380,
        };
        let daemon = OrchDaemon::new(config.clone());
        assert_eq!(daemon.config.heartbeat_interval_ms, 500);
        assert_eq!(daemon.config.batch_size, 256);
        assert!(daemon.config.warm_boot);
    }

    #[tokio::test]
    async fn test_orchdaemon_new_empty() {
        let daemon = OrchDaemon::new(OrchDaemonConfig::default());
        assert_eq!(daemon.orchs.len(), 0);
        assert!(!daemon.running);
    }

    // ============================================================================
    // 2. Orch Registration Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_register_single_orch() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        daemon.register_orch(Box::new(TestOrch::new("PortsOrch", 0)));

        assert_eq!(daemon.orchs.len(), 1);
        assert_eq!(daemon.orchs.get(&0).map(|v| v.len()), Some(1));
    }

    #[tokio::test]
    async fn test_orchdaemon_register_multiple_same_priority() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());

        daemon.register_orch(Box::new(TestOrch::new("PortsOrch", 0)));
        daemon.register_orch(Box::new(TestOrch::new("AclOrch", 0)));
        daemon.register_orch(Box::new(TestOrch::new("VlanOrch", 0)));

        assert_eq!(daemon.orchs.get(&0).map(|v| v.len()), Some(3));
    }

    #[tokio::test]
    async fn test_orchdaemon_register_different_priorities() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());

        daemon.register_orch(Box::new(TestOrch::new("PortsOrch", 0)));
        daemon.register_orch(Box::new(TestOrch::new("RouteOrch", 10)));
        daemon.register_orch(Box::new(TestOrch::new("AclOrch", 0)));
        daemon.register_orch(Box::new(TestOrch::new("QosOrch", 20)));

        // Priority 0 should have 2 orchs, priority 10 should have 1, priority 20 should have 1
        assert_eq!(daemon.orchs.get(&0).map(|v| v.len()), Some(2));
        assert_eq!(daemon.orchs.get(&10).map(|v| v.len()), Some(1));
        assert_eq!(daemon.orchs.get(&20).map(|v| v.len()), Some(1));
        assert_eq!(daemon.orchs.len(), 3); // 3 priority levels
    }

    #[tokio::test]
    async fn test_orchdaemon_register_negative_priority() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        daemon.register_orch(Box::new(TestOrch::new("CriticalOrch", -10)));

        assert_eq!(daemon.orchs.get(&-10).map(|v| v.len()), Some(1));
    }

    #[tokio::test]
    async fn test_orchdaemon_priority_ordering() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());

        daemon.register_orch(Box::new(TestOrch::new("LowPriority", 100)));
        daemon.register_orch(Box::new(TestOrch::new("HighPriority", -10)));
        daemon.register_orch(Box::new(TestOrch::new("MediumPriority", 50)));

        // BTreeMap should maintain sorted order (lowest priority number first)
        let priorities: Vec<i32> = daemon.orchs.keys().copied().collect();
        assert_eq!(priorities, vec![-10, 50, 100]);
    }

    // ============================================================================
    // 3. Context Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_context_access() {
        let daemon = OrchDaemon::new(OrchDaemonConfig::default());
        let ctx = daemon.context();

        // Should be able to access context
        let read_ctx = ctx.read().await;
        assert!(!read_ctx.warm_boot_in_progress);
    }

    #[tokio::test]
    async fn test_orchdaemon_context_shared() {
        let daemon = OrchDaemon::new(OrchDaemonConfig::default());
        let ctx1 = daemon.context();
        let ctx2 = daemon.context();

        // Both should point to the same context
        assert!(StdArc::ptr_eq(&ctx1, &ctx2));
    }

    // ============================================================================
    // 4. Initialization Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_init_success() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        assert!(daemon.init().await);
    }

    #[tokio::test]
    async fn test_orchdaemon_init_with_orchs() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        daemon.register_orch(Box::new(TestOrch::new("PortsOrch", 0)));
        daemon.register_orch(Box::new(TestOrch::new("RouteOrch", 10)));

        assert!(daemon.init().await);
    }

    // ============================================================================
    // 5. Stop Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_stop() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        assert!(!daemon.running);

        daemon.running = true; // Simulate running state
        daemon.stop();

        assert!(!daemon.running);
    }

    #[tokio::test]
    async fn test_orchdaemon_stop_when_not_running() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        assert!(!daemon.running);

        daemon.stop();
        assert!(!daemon.running); // Should remain false
    }

    // ============================================================================
    // 6. Warm Boot Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_prepare_warm_boot_empty() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        assert!(daemon.prepare_warm_boot().await);
    }

    #[tokio::test]
    async fn test_orchdaemon_on_warm_boot_end() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());

        // Set warm boot in progress
        {
            let mut ctx = daemon.context.write().await;
            ctx.warm_boot_in_progress = true;
        }

        daemon.on_warm_boot_end().await;

        // Should be cleared
        let ctx = daemon.context.read().await;
        assert!(!ctx.warm_boot_in_progress);
    }

    #[tokio::test]
    async fn test_orchdaemon_warm_boot_config() {
        let config = OrchDaemonConfig {
            heartbeat_interval_ms: 1000,
            batch_size: 128,
            warm_boot: true,
            redis_host: "127.0.0.1".to_string(),
            redis_port: 6379,
        };
        let daemon = OrchDaemon::new(config);
        assert!(daemon.config.warm_boot);
    }

    // ============================================================================
    // 7. Dump Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_dump_empty() {
        let daemon = OrchDaemon::new(OrchDaemonConfig::default());
        let lines = daemon.dump();

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("OrchDaemon running: false"));
    }

    #[tokio::test]
    async fn test_orchdaemon_dump_with_orchs() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        daemon.register_orch(Box::new(TestOrch::new("PortsOrch", 0)));
        daemon.register_orch(Box::new(TestOrch::new("RouteOrch", 10)));

        let lines = daemon.dump();

        // Should have 1 header + 2 orch lines
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("OrchDaemon running: false"));
        assert!(lines[1].contains("PortsOrch") || lines[2].contains("PortsOrch"));
        assert!(lines[1].contains("RouteOrch") || lines[2].contains("RouteOrch"));
    }

    #[tokio::test]
    async fn test_orchdaemon_dump_shows_running_state() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        daemon.running = true;

        let lines = daemon.dump();
        assert!(lines[0].contains("OrchDaemon running: true"));
    }

    // ============================================================================
    // 8. Edge Cases Tests
    // ============================================================================

    #[tokio::test]
    async fn test_orchdaemon_register_many_orchs() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());

        // Register 100 orchs with various priorities
        for i in 0..100 {
            daemon.register_orch(Box::new(TestOrch::new(
                &format!("Orch{}", i),
                (i % 10) as i32, // Priorities 0-9
            )));
        }

        assert_eq!(daemon.orchs.len(), 10); // 10 different priorities

        // Each priority should have 10 orchs
        for priority in 0..10 {
            assert_eq!(daemon.orchs.get(&priority).map(|v| v.len()), Some(10));
        }
    }

    #[tokio::test]
    async fn test_orchdaemon_extreme_priority_values() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());

        daemon.register_orch(Box::new(TestOrch::new("MaxPriority", i32::MAX)));
        daemon.register_orch(Box::new(TestOrch::new("MinPriority", i32::MIN)));

        assert_eq!(daemon.orchs.get(&i32::MAX).map(|v| v.len()), Some(1));
        assert_eq!(daemon.orchs.get(&i32::MIN).map(|v| v.len()), Some(1));
    }

    #[tokio::test]
    async fn test_orchdaemon_config_extreme_values() {
        let config = OrchDaemonConfig {
            heartbeat_interval_ms: u64::MAX,
            batch_size: usize::MAX,
            warm_boot: true,
            redis_host: "127.0.0.1".to_string(),
            redis_port: 6379,
        };
        let daemon = OrchDaemon::new(config);
        assert_eq!(daemon.config.heartbeat_interval_ms, u64::MAX);
        assert_eq!(daemon.config.batch_size, usize::MAX);
    }

    #[tokio::test]
    async fn test_orchdaemon_config_zero_values() {
        let config = OrchDaemonConfig {
            heartbeat_interval_ms: 0,
            batch_size: 0,
            warm_boot: false,
            redis_host: "127.0.0.1".to_string(),
            redis_port: 6379,
        };
        let daemon = OrchDaemon::new(config);
        assert_eq!(daemon.config.heartbeat_interval_ms, 0);
        assert_eq!(daemon.config.batch_size, 0);
    }
}
