use std::sync::Arc;
/// Advanced health monitoring for neighsyncd
///
/// Provides comprehensive health status tracking with:
/// - Multi-metric health scoring
/// - Dependency health tracking
/// - Stall detection
/// - Performance degradation detection
/// - Predictive health status
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthStatus {
    /// All systems nominal, all metrics within thresholds
    Healthy = 100,
    /// Degraded performance, some metrics approaching thresholds
    Degraded = 50,
    /// Critical issues, service may be unavailable
    Unhealthy = 0,
}

impl HealthStatus {
    pub fn as_metric_value(&self) -> f64 {
        match self {
            HealthStatus::Healthy => 1.0,
            HealthStatus::Degraded => 0.5,
            HealthStatus::Unhealthy => 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DependencyHealth {
    /// Redis connection status (0.0 = disconnected, 1.0 = connected)
    pub redis_connected: f64,
    /// Netlink socket status (0.0 = disconnected, 1.0 = connected)
    pub netlink_connected: f64,
    /// System memory available (0.0 = full, 1.0 = plenty)
    pub memory_available: f64,
    /// CPU utilization (0.0 = idle, 1.0 = maxed)
    pub cpu_utilization: f64,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Event processing latency p99 in seconds
    pub event_latency_p99: f64,
    /// Event processing latency p95 in seconds
    pub event_latency_p95: f64,
    /// Redis operation latency p99 in seconds
    pub redis_latency_p99: f64,
    /// Event processing rate (events/sec)
    pub processing_rate: f64,
    /// Error rate (errors/events)
    pub error_rate: f64,
    /// Queue depth (pending events)
    pub queue_depth: u64,
}

/// Health score thresholds
#[derive(Debug, Clone)]
pub struct HealthThresholds {
    /// Max event latency p99 before degradation warning (seconds)
    pub max_event_latency_p99: f64,
    /// Max event latency p99 before unhealthy (seconds)
    pub critical_event_latency_p99: f64,
    /// Max error rate before degradation warning (ratio)
    pub max_error_rate: f64,
    /// Max error rate before unhealthy (ratio)
    pub critical_error_rate: f64,
    /// Max queue depth before degradation warning
    pub max_queue_depth: u64,
    /// Max queue depth before unhealthy
    pub critical_queue_depth: u64,
    /// Max memory usage before degradation warning (bytes)
    pub max_memory_bytes: u64,
    /// Max memory usage before unhealthy (bytes)
    pub critical_memory_bytes: u64,
    /// No events for N seconds triggers stall warning
    pub stall_detection_timeout: u64,
    /// No events for N seconds triggers unhealthy
    pub critical_stall_timeout: u64,
    /// Processing rate below N events/sec triggers warning
    pub min_processing_rate: f64,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            max_event_latency_p99: 0.050,             // 50ms warning
            critical_event_latency_p99: 0.100,        // 100ms critical
            max_error_rate: 0.01,                     // 1% warning
            critical_error_rate: 0.05,                // 5% critical
            max_queue_depth: 1000,                    // 1000 pending warning
            critical_queue_depth: 5000,               // 5000 pending critical
            max_memory_bytes: 150 * 1024 * 1024,      // 150MB warning
            critical_memory_bytes: 200 * 1024 * 1024, // 200MB critical
            stall_detection_timeout: 30,              // 30 seconds
            critical_stall_timeout: 60,               // 60 seconds
            min_processing_rate: 1.0,                 // At least 1 event/sec
        }
    }
}

/// Advanced health monitor combining multiple metrics
pub struct AdvancedHealthMonitor {
    thresholds: HealthThresholds,
    /// Last event processing timestamp
    last_event_timestamp: Arc<AtomicU64>,
    /// Consecutive degradation counts
    degradation_count: Arc<AtomicU64>,
    /// Current health status
    current_status: Arc<AtomicU64>,
}

impl AdvancedHealthMonitor {
    pub fn new(thresholds: HealthThresholds) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            thresholds,
            last_event_timestamp: Arc::new(AtomicU64::new(now)),
            degradation_count: Arc::new(AtomicU64::new(0)),
            current_status: Arc::new(AtomicU64::new(HealthStatus::Healthy as u64)),
        }
    }

    /// Record an event processing occurrence
    pub fn record_event(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_event_timestamp.store(now, Ordering::Relaxed);
    }

    /// Compute overall health status from multiple metrics
    pub fn compute_health_status(
        &self,
        dependencies: &DependencyHealth,
        performance: &PerformanceMetrics,
        memory_bytes: u64,
    ) -> HealthStatus {
        // Check for critical failures first
        if self.is_critical(dependencies, performance, memory_bytes) {
            self.current_status
                .store(HealthStatus::Unhealthy as u64, Ordering::Relaxed);
            return HealthStatus::Unhealthy;
        }

        // Check for degradation
        if self.is_degraded(dependencies, performance, memory_bytes) {
            let count = self.degradation_count.fetch_add(1, Ordering::Relaxed);
            // Require 3 consecutive degraded checks before marking degraded
            if count >= 2 {
                self.current_status
                    .store(HealthStatus::Degraded as u64, Ordering::Relaxed);
                return HealthStatus::Degraded;
            }
        } else {
            // Reset degradation counter on healthy check
            self.degradation_count.store(0, Ordering::Relaxed);
        }

        self.current_status
            .store(HealthStatus::Healthy as u64, Ordering::Relaxed);
        HealthStatus::Healthy
    }

    /// Check for critical health issues
    fn is_critical(
        &self,
        dependencies: &DependencyHealth,
        performance: &PerformanceMetrics,
        memory_bytes: u64,
    ) -> bool {
        // Critical: Both Redis and Netlink disconnected
        if dependencies.redis_connected < 0.5 && dependencies.netlink_connected < 0.5 {
            return true;
        }

        // Critical: Stall timeout exceeded
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last_event = self.last_event_timestamp.load(Ordering::Relaxed);
        if now - last_event > self.thresholds.critical_stall_timeout {
            return true;
        }

        // Critical: Error rate exceeds critical threshold
        if performance.error_rate > self.thresholds.critical_error_rate {
            return true;
        }

        // Critical: Event latency exceeds critical threshold
        if performance.event_latency_p99 > self.thresholds.critical_event_latency_p99 {
            return true;
        }

        // Critical: Memory usage exceeds critical threshold
        if memory_bytes > self.thresholds.critical_memory_bytes {
            return true;
        }

        // Critical: Queue depth exceeds critical threshold
        if performance.queue_depth > self.thresholds.critical_queue_depth {
            return true;
        }

        false
    }

    /// Check for degradation conditions
    fn is_degraded(
        &self,
        dependencies: &DependencyHealth,
        performance: &PerformanceMetrics,
        memory_bytes: u64,
    ) -> bool {
        // Degraded: Redis or Netlink disconnected
        if dependencies.redis_connected < 0.5 || dependencies.netlink_connected < 0.5 {
            return true;
        }

        // Degraded: Stall timeout warning
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last_event = self.last_event_timestamp.load(Ordering::Relaxed);
        if now - last_event > self.thresholds.stall_detection_timeout {
            return true;
        }

        // Degraded: Error rate exceeds warning threshold
        if performance.error_rate > self.thresholds.max_error_rate {
            return true;
        }

        // Degraded: Event latency exceeds warning threshold
        if performance.event_latency_p99 > self.thresholds.max_event_latency_p99 {
            return true;
        }

        // Degraded: Memory usage exceeds warning threshold
        if memory_bytes > self.thresholds.max_memory_bytes {
            return true;
        }

        // Degraded: Queue depth exceeds warning threshold
        if performance.queue_depth > self.thresholds.max_queue_depth {
            return true;
        }

        // Degraded: Processing rate below minimum
        if performance.processing_rate < self.thresholds.min_processing_rate && last_event < now - 5
        {
            return true;
        }

        false
    }

    /// Get current health status without updating
    pub fn get_current_status(&self) -> HealthStatus {
        match self.current_status.load(Ordering::Relaxed) {
            0 => HealthStatus::Unhealthy,
            50 => HealthStatus::Degraded,
            _ => HealthStatus::Healthy,
        }
    }

    /// Get time since last event
    pub fn time_since_last_event(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last_event = self.last_event_timestamp.load(Ordering::Relaxed);
        now - last_event
    }

    /// Check if currently in stall condition
    pub fn is_stalled(&self) -> bool {
        self.time_since_last_event() > self.thresholds.stall_detection_timeout
    }

    /// Health score calculation (0-100)
    pub fn calculate_health_score(
        &self,
        dependencies: &DependencyHealth,
        performance: &PerformanceMetrics,
    ) -> f64 {
        let mut score = 100.0;

        // Dependency score (30 points)
        let dep_score =
            (dependencies.redis_connected * 15.0) + (dependencies.netlink_connected * 15.0);
        score -= 30.0 - dep_score;

        // Performance score (40 points)
        // Latency impact (15 points)
        if performance.event_latency_p99 > self.thresholds.critical_event_latency_p99 {
            score -= 15.0;
        } else if performance.event_latency_p99 > self.thresholds.max_event_latency_p99 {
            score -= 7.5;
        }

        // Error rate impact (15 points)
        if performance.error_rate > self.thresholds.critical_error_rate {
            score -= 15.0;
        } else if performance.error_rate > self.thresholds.max_error_rate {
            score -= 7.5;
        }

        // Queue depth impact (10 points)
        if performance.queue_depth > self.thresholds.critical_queue_depth {
            score -= 10.0;
        } else if performance.queue_depth > self.thresholds.max_queue_depth {
            score -= 5.0;
        }

        // Stall detection impact (20 points)
        let time_since_event = self.time_since_last_event();
        if time_since_event > self.thresholds.critical_stall_timeout {
            score -= 20.0;
        } else if time_since_event > self.thresholds.stall_detection_timeout {
            score -= 10.0;
        }

        // Processing rate impact (10 points)
        if performance.processing_rate < self.thresholds.min_processing_rate {
            score -= 10.0;
        }

        score.clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_ordering() {
        assert!(HealthStatus::Healthy > HealthStatus::Degraded);
        assert!(HealthStatus::Degraded > HealthStatus::Unhealthy);
        assert_eq!(HealthStatus::Healthy.as_metric_value(), 1.0);
        assert_eq!(HealthStatus::Degraded.as_metric_value(), 0.5);
        assert_eq!(HealthStatus::Unhealthy.as_metric_value(), 0.0);
    }

    #[test]
    fn test_health_monitor_creation() {
        let monitor = AdvancedHealthMonitor::new(HealthThresholds::default());
        assert_eq!(monitor.get_current_status(), HealthStatus::Healthy);
        // time_since_last_event is always >= 0 for u64
        let _ = monitor.time_since_last_event();
    }

    #[test]
    fn test_stall_detection() {
        let mut thresholds = HealthThresholds::default();
        thresholds.stall_detection_timeout = 1; // 1 second for testing
        let monitor = AdvancedHealthMonitor::new(thresholds);

        // Initially not stalled
        assert!(!monitor.is_stalled());

        // After waiting, should be stalled
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(monitor.is_stalled());

        // After recording event, should not be stalled
        monitor.record_event();
        assert!(!monitor.is_stalled());
    }

    #[test]
    fn test_critical_redis_netlink_disconnect() {
        let monitor = AdvancedHealthMonitor::new(HealthThresholds::default());

        let dependencies = DependencyHealth {
            redis_connected: 0.0,
            netlink_connected: 0.0,
            memory_available: 1.0,
            cpu_utilization: 0.5,
        };

        let performance = PerformanceMetrics {
            event_latency_p99: 0.010,
            event_latency_p95: 0.005,
            redis_latency_p99: 0.005,
            processing_rate: 1000.0,
            error_rate: 0.0,
            queue_depth: 0,
        };

        let status = monitor.compute_health_status(&dependencies, &performance, 50 * 1024 * 1024);
        assert_eq!(status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_degraded_high_latency() {
        let monitor = AdvancedHealthMonitor::new(HealthThresholds::default());
        monitor.record_event(); // Avoid stall

        let dependencies = DependencyHealth {
            redis_connected: 1.0,
            netlink_connected: 1.0,
            memory_available: 1.0,
            cpu_utilization: 0.5,
        };

        let performance = PerformanceMetrics {
            event_latency_p99: 0.075, // Between warning and critical
            event_latency_p95: 0.050,
            redis_latency_p99: 0.005,
            processing_rate: 1000.0,
            error_rate: 0.0,
            queue_depth: 100,
        };

        // First call - still healthy (degradation counter at 1)
        let _ = monitor.compute_health_status(&dependencies, &performance, 50 * 1024 * 1024);
        // Second call - still healthy (degradation counter at 2)
        let _ = monitor.compute_health_status(&dependencies, &performance, 50 * 1024 * 1024);
        // Third call - now degraded (degradation counter at 3)
        let status = monitor.compute_health_status(&dependencies, &performance, 50 * 1024 * 1024);
        assert_eq!(status, HealthStatus::Degraded);
    }

    #[test]
    fn test_healthy_all_metrics_good() {
        let monitor = AdvancedHealthMonitor::new(HealthThresholds::default());
        monitor.record_event();

        let dependencies = DependencyHealth {
            redis_connected: 1.0,
            netlink_connected: 1.0,
            memory_available: 1.0,
            cpu_utilization: 0.5,
        };

        let performance = PerformanceMetrics {
            event_latency_p99: 0.010,
            event_latency_p95: 0.005,
            redis_latency_p99: 0.002,
            processing_rate: 5000.0,
            error_rate: 0.001,
            queue_depth: 50,
        };

        let status = monitor.compute_health_status(&dependencies, &performance, 50 * 1024 * 1024);
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_score_calculation() {
        let monitor = AdvancedHealthMonitor::new(HealthThresholds::default());
        monitor.record_event();

        let dependencies = DependencyHealth {
            redis_connected: 1.0,
            netlink_connected: 1.0,
            memory_available: 1.0,
            cpu_utilization: 0.5,
        };

        let performance = PerformanceMetrics {
            event_latency_p99: 0.010,
            event_latency_p95: 0.005,
            redis_latency_p99: 0.002,
            processing_rate: 5000.0,
            error_rate: 0.001,
            queue_depth: 50,
        };

        let score = monitor.calculate_health_score(&dependencies, &performance);
        assert!(score > 90.0); // Should be nearly perfect
    }
}
