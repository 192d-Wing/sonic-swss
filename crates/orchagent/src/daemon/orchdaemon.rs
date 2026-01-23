//! OrchDaemon implementation.
//!
//! The OrchDaemon is the central coordinator for all Orch modules.
//! It manages:
//! - Event loop using Select/epoll
//! - Orch registration and priority ordering
//! - Task dispatch to appropriate Orchs
//! - Warm restart coordination

use log::{debug, info, error};
use sonic_orch_common::{Orch, OrchContext};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome, audit_log};

/// Configuration for the OrchDaemon.
#[derive(Debug, Clone)]
pub struct OrchDaemonConfig {
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
    /// Batch size for consumer operations
    pub batch_size: usize,
    /// Enable warm boot mode
    pub warm_boot: bool,
}

impl Default for OrchDaemonConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: 1000,
            batch_size: 128,
            warm_boot: false,
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
}

impl OrchDaemon {
    /// Creates a new OrchDaemon with the given configuration.
    pub fn new(config: OrchDaemonConfig) -> Self {
        Self {
            config,
            orchs: BTreeMap::new(),
            context: Arc::new(RwLock::new(OrchContext::default())),
            running: false,
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

    /// Initializes all registered Orchs.
    ///
    /// Called during startup before the event loop begins.
    pub async fn init(&mut self) -> bool {
        info!("Initializing OrchDaemon with {} orch groups", self.orchs.len());

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

        // TODO: Initialize SAI
        // TODO: Create switch
        // TODO: Initialize database connections

        let success_record = AuditRecord::new(
            AuditCategory::SystemLifecycle,
            "OrchDaemon",
            "daemon_initialization_end",
        )
        .with_outcome(AuditOutcome::Success)
        .with_details(serde_json::json!({
            "orch_count": self.orchs.len(),
        }));
        audit_log!(success_record);

        true
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

        let record = AuditRecord::new(
            AuditCategory::AdminAction,
            "OrchDaemon",
            "stop_requested",
        )
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

        let record = AuditRecord::new(
            AuditCategory::WarmRestart,
            "OrchDaemon",
            "warm_boot_ended",
        )
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

    /// Dumps state for debugging.
    pub fn dump(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("OrchDaemon running: {}", self.running));

        for (priority, orchs) in &self.orchs {
            for orch in orchs {
                lines.push(format!("  [{:3}] {} - {} pending", priority, orch.name(),
                    orch.dump_pending_tasks().len()));
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
        };
        let daemon = OrchDaemon::new(config);
        assert_eq!(daemon.config.heartbeat_interval_ms, 0);
        assert_eq!(daemon.config.batch_size, 0);
    }
}
