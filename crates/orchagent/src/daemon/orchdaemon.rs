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
        info!("Registering {} with priority {}", orch.name(), priority);
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

        // TODO: Initialize SAI
        // TODO: Create switch
        // TODO: Initialize database connections

        true
    }

    /// Runs the main event loop.
    ///
    /// This method blocks until `stop()` is called.
    pub async fn run(&mut self) {
        info!("Starting OrchDaemon event loop");
        self.running = true;

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
    }

    /// Stops the event loop.
    pub fn stop(&mut self) {
        info!("Stopping OrchDaemon");
        self.running = false;
    }

    /// Prepares for warm boot.
    pub async fn prepare_warm_boot(&mut self) -> bool {
        info!("Preparing for warm boot");

        for (_priority, orchs) in self.orchs.iter_mut() {
            for orch in orchs.iter_mut() {
                if !orch.bake() {
                    error!("Failed to bake {}", orch.name());
                    return false;
                }
            }
        }

        true
    }

    /// Called after warm boot APPLY_VIEW.
    pub async fn on_warm_boot_end(&mut self) {
        info!("Warm boot ended, resuming normal operation");

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

    struct TestOrch {
        name: String,
        priority: i32,
    }

    #[async_trait]
    impl Orch for TestOrch {
        fn name(&self) -> &str {
            &self.name
        }

        async fn do_task(&mut self) {
            // No-op for test
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    #[tokio::test]
    async fn test_orchdaemon_registration() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());

        daemon.register_orch(Box::new(TestOrch {
            name: "PortsOrch".to_string(),
            priority: 0,
        }));
        daemon.register_orch(Box::new(TestOrch {
            name: "RouteOrch".to_string(),
            priority: 10,
        }));
        daemon.register_orch(Box::new(TestOrch {
            name: "AclOrch".to_string(),
            priority: 0,
        }));

        // Priority 0 should have 2 orchs, priority 10 should have 1
        assert_eq!(daemon.orchs.get(&0).map(|v| v.len()), Some(2));
        assert_eq!(daemon.orchs.get(&10).map(|v| v.len()), Some(1));
    }

    #[tokio::test]
    async fn test_orchdaemon_init() {
        let mut daemon = OrchDaemon::new(OrchDaemonConfig::default());
        assert!(daemon.init().await);
    }
}
