//! Base Orch trait and context.

use async_trait::async_trait;

/// Context shared across all Orch modules.
///
/// This provides access to shared state and coordination primitives
/// that multiple Orchs may need to access.
#[derive(Debug, Clone)]
pub struct OrchContext {
    /// Flag indicating if all ports are ready
    pub all_ports_ready: bool,
    /// Flag indicating if warm boot is in progress
    pub warm_boot_in_progress: bool,
    /// Flag indicating if the system is healthy
    pub system_healthy: bool,
}

impl Default for OrchContext {
    fn default() -> Self {
        Self {
            all_ports_ready: false,
            warm_boot_in_progress: false,
            system_healthy: true,
        }
    }
}

/// Base trait for all orchestration agents.
///
/// Each Orch module implements this trait to participate in the
/// OrchDaemon event loop. The daemon calls these methods in response
/// to Redis table changes and timer events.
///
/// # Lifecycle
///
/// 1. Construction: Orch is created with database connections
/// 2. Registration: Orch registers its consumers with the daemon
/// 3. Event Loop: `do_task()` is called when data is available
/// 4. Warm Boot: `bake()` and `on_warm_boot_end()` handle state recovery
/// 5. Shutdown: Orch is dropped (cleanup via Drop trait)
///
/// # Thread Safety
///
/// Orch implementations must be `Send + Sync` to allow for potential
/// concurrent access from the daemon and notification handlers.
#[async_trait]
pub trait Orch: Send + Sync {
    /// Returns the name of this Orch (for logging and debugging).
    fn name(&self) -> &str;

    /// Processes pending tasks from all consumers.
    ///
    /// This is the main entry point called by the OrchDaemon when
    /// data is available on any of this Orch's consumers.
    ///
    /// Implementations should:
    /// 1. Drain pending entries from consumers
    /// 2. Process each entry (translate to SAI calls)
    /// 3. Handle errors appropriately (retry, log, etc.)
    async fn do_task(&mut self);

    /// Prepares for warm boot by saving state.
    ///
    /// Called before warm boot to allow the Orch to save any
    /// state that needs to be preserved across the restart.
    ///
    /// Returns `true` if preparation was successful.
    fn bake(&mut self) -> bool {
        true
    }

    /// Called after APPLY_VIEW during warm/fast boot.
    ///
    /// This is the signal that SAI state has been restored and
    /// the Orch can resume normal operation.
    fn on_warm_boot_end(&mut self) {
        // Default: no-op
    }

    /// Returns the priority of this Orch (lower = higher priority).
    ///
    /// Orchs with lower priority values are processed first.
    /// Default is 0 (highest priority).
    fn priority(&self) -> i32 {
        0
    }

    /// Returns true if this Orch has pending work.
    ///
    /// Used by the daemon to determine if `do_task()` should be called.
    fn has_pending_tasks(&self) -> bool {
        false
    }

    /// Dumps pending tasks for debugging.
    ///
    /// Returns a list of human-readable strings describing pending work.
    fn dump_pending_tasks(&self) -> Vec<String> {
        vec![]
    }

    /// Called periodically by the daemon's timer.
    ///
    /// Orchs can use this for periodic maintenance tasks.
    fn on_timer(&mut self) {
        // Default: no-op
    }

    /// Handles a notification from SAI.
    ///
    /// Override this to handle asynchronous SAI notifications
    /// (e.g., port state changes, FDB events).
    fn on_notification(&mut self, _notification: &str) {
        // Default: no-op
    }
}

/// Trait for Orchs that follow the simplified request-based pattern.
///
/// This is an alternative to the base Orch trait that provides a more
/// structured interface for processing add/delete operations.
#[async_trait]
pub trait Orch2: Orch {
    /// The request type this Orch processes.
    type Request;

    /// Processes an add operation.
    ///
    /// Called when a SET operation is received for a key.
    async fn add_operation(&mut self, request: &Self::Request) -> crate::TaskResult<()>;

    /// Processes a delete operation.
    ///
    /// Called when a DEL operation is received for a key.
    async fn del_operation(&mut self, request: &Self::Request) -> crate::TaskResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestOrch {
        name: String,
        task_count: usize,
    }

    #[async_trait]
    impl Orch for TestOrch {
        fn name(&self) -> &str {
            &self.name
        }

        async fn do_task(&mut self) {
            self.task_count += 1;
        }

        fn has_pending_tasks(&self) -> bool {
            self.task_count < 10
        }
    }

    #[tokio::test]
    async fn test_orch_trait() {
        let mut orch = TestOrch {
            name: "test".to_string(),
            task_count: 0,
        };

        assert_eq!(orch.name(), "test");
        assert!(orch.has_pending_tasks());
        assert!(orch.bake());

        orch.do_task().await;
        assert_eq!(orch.task_count, 1);
    }

    #[test]
    fn test_orch_context_default() {
        let ctx = OrchContext::default();
        assert!(!ctx.all_ports_ready);
        assert!(!ctx.warm_boot_in_progress);
        assert!(ctx.system_healthy);
    }
}
