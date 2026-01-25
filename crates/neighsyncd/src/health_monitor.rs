//! Health monitoring for neighsyncd
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - CP-10: System Recovery - Track system health during recovery
//! - SI-4: System Monitoring - Continuous health monitoring

use crate::metrics::{HealthStatus, MetricsCollector};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Default maximum stall duration before marking as degraded
const DEFAULT_MAX_STALL_DURATION: Duration = Duration::from_secs(10);

/// Default maximum failure rate before marking as degraded (5%)
const DEFAULT_MAX_FAILURE_RATE: f64 = 0.05;

/// Health monitor for tracking service health
///
/// # NIST Controls
/// - CP-10: System Recovery - Monitor recovery health
/// - SI-4: System Monitoring - Continuous monitoring
pub struct HealthMonitor {
    /// Metrics collector to update
    metrics: MetricsCollector,

    /// Last time an event was successfully processed
    last_event_time: Instant,

    /// Maximum stall duration before degraded
    max_stall_duration: Duration,

    /// Total events processed
    total_events: u64,

    /// Total events failed
    failed_events: u64,

    /// Maximum failure rate
    max_failure_rate: f64,

    /// Current health status
    current_status: HealthStatus,
}

impl HealthMonitor {
    /// Create a new health monitor
    ///
    /// # NIST Controls
    /// - SI-4: System Monitoring - Initialize monitoring
    pub fn new(metrics: MetricsCollector) -> Self {
        let status = HealthStatus::Healthy;
        metrics.set_health_status(status);

        Self {
            metrics,
            last_event_time: Instant::now(),
            max_stall_duration: DEFAULT_MAX_STALL_DURATION,
            total_events: 0,
            failed_events: 0,
            max_failure_rate: DEFAULT_MAX_FAILURE_RATE,
            current_status: status,
        }
    }

    /// Create a new health monitor with custom configuration
    pub fn with_config(
        metrics: MetricsCollector,
        max_stall_duration: Duration,
        max_failure_rate: f64,
    ) -> Self {
        let status = HealthStatus::Healthy;
        metrics.set_health_status(status);

        Self {
            metrics,
            last_event_time: Instant::now(),
            max_stall_duration,
            total_events: 0,
            failed_events: 0,
            max_failure_rate,
            current_status: status,
        }
    }

    /// Record a successful event
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Track successful events
    pub fn record_success(&mut self) {
        self.last_event_time = Instant::now();
        self.total_events += 1;
        self.update_health();
    }

    /// Record a failed event
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Track failed events
    pub fn record_failure(&mut self) {
        self.last_event_time = Instant::now();
        self.total_events += 1;
        self.failed_events += 1;
        self.update_health();
    }

    /// Check and update health status
    ///
    /// # NIST Controls
    /// - CP-10: System Recovery - Assess recovery health
    /// - SI-4: System Monitoring - Evaluate health status
    pub fn update_health(&mut self) {
        let new_status = self.calculate_health();

        if new_status != self.current_status {
            info!(
                old_status = ?self.current_status,
                new_status = ?new_status,
                "Health status changed"
            );
            self.current_status = new_status;
            self.metrics.set_health_status(new_status);
        }
    }

    /// Calculate current health status
    fn calculate_health(&self) -> HealthStatus {
        // Check for stall
        let stalled = self.last_event_time.elapsed() > self.max_stall_duration;

        // Check failure rate
        let failure_rate = if self.total_events > 0 {
            self.failed_events as f64 / self.total_events as f64
        } else {
            0.0
        };

        if stalled {
            warn!(
                elapsed_secs = self.last_event_time.elapsed().as_secs(),
                max_stall_secs = self.max_stall_duration.as_secs(),
                "Service stalled - no events processed recently"
            );
            return HealthStatus::Unhealthy;
        }

        if failure_rate > self.max_failure_rate {
            warn!(
                failure_rate = failure_rate,
                max_failure_rate = self.max_failure_rate,
                "High failure rate detected"
            );
            return HealthStatus::Degraded;
        }

        HealthStatus::Healthy
    }

    /// Get current health status
    pub fn status(&self) -> HealthStatus {
        self.current_status
    }

    /// Get failure rate
    pub fn failure_rate(&self) -> f64 {
        if self.total_events > 0 {
            self.failed_events as f64 / self.total_events as f64
        } else {
            0.0
        }
    }

    /// Get time since last event
    pub fn time_since_last_event(&self) -> Duration {
        self.last_event_time.elapsed()
    }

    /// Reset counters (useful for warm restart)
    pub fn reset_counters(&mut self) {
        self.total_events = 0;
        self.failed_events = 0;
        self.last_event_time = Instant::now();
        self.current_status = HealthStatus::Healthy;
        self.metrics.set_health_status(HealthStatus::Healthy);
        info!("Health monitor counters reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_monitor() -> HealthMonitor {
        let metrics = MetricsCollector::new().unwrap();
        HealthMonitor::new(metrics)
    }

    #[test]
    fn test_health_monitor_creation() {
        let monitor = create_test_monitor();
        assert_eq!(monitor.status(), HealthStatus::Healthy);
        assert_eq!(monitor.failure_rate(), 0.0);
    }

    #[test]
    fn test_record_success() {
        let mut monitor = create_test_monitor();
        monitor.record_success();
        assert_eq!(monitor.status(), HealthStatus::Healthy);
        assert_eq!(monitor.total_events, 1);
        assert_eq!(monitor.failed_events, 0);
    }

    #[test]
    fn test_record_failure() {
        let mut monitor = create_test_monitor();
        monitor.record_failure();
        assert_eq!(monitor.total_events, 1);
        assert_eq!(monitor.failed_events, 1);
        assert_eq!(monitor.failure_rate(), 1.0);
    }

    #[test]
    fn test_failure_rate_threshold() {
        let mut monitor = create_test_monitor();

        // Add successes to get to 5% failure rate
        for _ in 0..95 {
            monitor.record_success();
        }
        assert_eq!(monitor.status(), HealthStatus::Healthy);

        // Add 5 failures to hit 5% threshold
        for _ in 0..5 {
            monitor.record_failure();
        }
        // At exactly 5%, should still be healthy (> threshold triggers)
        assert_eq!(monitor.status(), HealthStatus::Healthy);

        // One more failure pushes over threshold
        monitor.record_failure();
        assert_eq!(monitor.status(), HealthStatus::Degraded);
    }

    #[test]
    fn test_reset_counters() {
        let mut monitor = create_test_monitor();
        monitor.record_success();
        monitor.record_failure();
        assert_eq!(monitor.total_events, 2);

        monitor.reset_counters();
        assert_eq!(monitor.total_events, 0);
        assert_eq!(monitor.failed_events, 0);
        assert_eq!(monitor.status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_custom_configuration() {
        let metrics = MetricsCollector::new().unwrap();
        let monitor = HealthMonitor::with_config(
            metrics,
            Duration::from_secs(30),
            0.10, // 10% failure rate
        );

        assert_eq!(monitor.max_stall_duration, Duration::from_secs(30));
        assert_eq!(monitor.max_failure_rate, 0.10);
    }
}
