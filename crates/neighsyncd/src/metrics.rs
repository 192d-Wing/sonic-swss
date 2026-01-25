//! Prometheus metrics collection for neighsyncd
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - AU-6: Audit Record Review - Metrics available for analysis
//! - SI-4: System Monitoring - Performance and health metrics
//! - CP-10: System Recovery - Track recovery metrics

use prometheus::{Counter, Gauge, Histogram, HistogramOpts, Opts, Registry};
use std::sync::Arc;

/// Global metrics collector for neighsyncd
///
/// # NIST Controls
/// - SI-4: System Monitoring - Centralized metrics collection
#[derive(Clone)]
pub struct MetricsCollector {
    // Counters
    pub neighbors_processed_total: Counter,
    pub neighbors_added_total: Counter,
    pub neighbors_deleted_total: Counter,
    pub events_failed_total: Counter,
    pub netlink_errors_total: Counter,
    pub redis_errors_total: Counter,

    // Gauges
    pub pending_neighbors: Gauge,
    pub queue_depth: Gauge,
    pub memory_bytes: Gauge,
    pub redis_connected: Gauge,
    pub netlink_connected: Gauge,
    pub health_status: Gauge,

    // Histograms
    pub event_latency_seconds: Histogram,
    pub redis_latency_seconds: Histogram,
    pub batch_size: Histogram,

    // Registry for export
    pub registry: Arc<Registry>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Initialize audit metrics
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // Counters
        let neighbors_processed_total = Counter::with_opts(Opts::new(
            "neighsyncd_neighbors_processed_total",
            "Total number of neighbor events processed",
        ))?;
        registry.register(Box::new(neighbors_processed_total.clone()))?;

        let neighbors_added_total = Counter::with_opts(Opts::new(
            "neighsyncd_neighbors_added_total",
            "Total number of neighbors added",
        ))?;
        registry.register(Box::new(neighbors_added_total.clone()))?;

        let neighbors_deleted_total = Counter::with_opts(Opts::new(
            "neighsyncd_neighbors_deleted_total",
            "Total number of neighbors deleted",
        ))?;
        registry.register(Box::new(neighbors_deleted_total.clone()))?;

        let events_failed_total = Counter::with_opts(Opts::new(
            "neighsyncd_events_failed_total",
            "Total number of failed events",
        ))?;
        registry.register(Box::new(events_failed_total.clone()))?;

        let netlink_errors_total = Counter::with_opts(Opts::new(
            "neighsyncd_netlink_errors_total",
            "Total number of netlink socket errors",
        ))?;
        registry.register(Box::new(netlink_errors_total.clone()))?;

        let redis_errors_total = Counter::with_opts(Opts::new(
            "neighsyncd_redis_errors_total",
            "Total number of Redis operation errors",
        ))?;
        registry.register(Box::new(redis_errors_total.clone()))?;

        // Gauges
        let pending_neighbors = Gauge::with_opts(Opts::new(
            "neighsyncd_pending_neighbors",
            "Current number of pending neighbor events",
        ))?;
        registry.register(Box::new(pending_neighbors.clone()))?;

        let queue_depth = Gauge::with_opts(Opts::new(
            "neighsyncd_queue_depth",
            "Current event queue depth",
        ))?;
        registry.register(Box::new(queue_depth.clone()))?;

        let memory_bytes = Gauge::with_opts(Opts::new(
            "neighsyncd_memory_bytes",
            "Current process memory usage in bytes",
        ))?;
        registry.register(Box::new(memory_bytes.clone()))?;

        let redis_connected = Gauge::with_opts(Opts::new(
            "neighsyncd_redis_connected",
            "Redis connection status (1=connected, 0=disconnected)",
        ))?;
        registry.register(Box::new(redis_connected.clone()))?;

        let netlink_connected = Gauge::with_opts(Opts::new(
            "neighsyncd_netlink_connected",
            "Netlink socket status (1=connected, 0=disconnected)",
        ))?;
        registry.register(Box::new(netlink_connected.clone()))?;

        let health_status = Gauge::with_opts(Opts::new(
            "neighsyncd_health_status",
            "Service health status (1.0=healthy, 0.5=degraded, 0.0=unhealthy)",
        ))?;
        registry.register(Box::new(health_status.clone()))?;

        // Histograms
        let event_latency_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "neighsyncd_event_latency_seconds",
                "Event processing latency in seconds",
            )
            .buckets(vec![
                0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
            ]),
        )?;
        registry.register(Box::new(event_latency_seconds.clone()))?;

        let redis_latency_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "neighsyncd_redis_latency_seconds",
                "Redis operation latency in seconds",
            )
            .buckets(vec![
                0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
            ]),
        )?;
        registry.register(Box::new(redis_latency_seconds.clone()))?;

        let batch_size = Histogram::with_opts(
            HistogramOpts::new(
                "neighsyncd_batch_size",
                "Distribution of batch sizes for Redis operations",
            )
            .buckets(vec![
                1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0,
            ]),
        )?;
        registry.register(Box::new(batch_size.clone()))?;

        Ok(Self {
            neighbors_processed_total,
            neighbors_added_total,
            neighbors_deleted_total,
            events_failed_total,
            netlink_errors_total,
            redis_errors_total,
            pending_neighbors,
            queue_depth,
            memory_bytes,
            redis_connected,
            netlink_connected,
            health_status,
            event_latency_seconds,
            redis_latency_seconds,
            batch_size,
            registry: Arc::new(registry),
        })
    }

    /// Record a neighbor event processed
    pub fn record_neighbor_processed(&self, is_add: bool) {
        self.neighbors_processed_total.inc();
        if is_add {
            self.neighbors_added_total.inc();
        } else {
            self.neighbors_deleted_total.inc();
        }
    }

    /// Record a failed event
    pub fn record_event_failed(&self) {
        self.events_failed_total.inc();
    }

    /// Record a netlink error
    pub fn record_netlink_error(&self) {
        self.netlink_errors_total.inc();
    }

    /// Record a Redis error
    pub fn record_redis_error(&self) {
        self.redis_errors_total.inc();
    }

    /// Update pending neighbors count
    pub fn set_pending_neighbors(&self, count: usize) {
        self.pending_neighbors.set(count as f64);
    }

    /// Update queue depth
    pub fn set_queue_depth(&self, depth: usize) {
        self.queue_depth.set(depth as f64);
    }

    /// Update memory usage
    pub fn set_memory_bytes(&self, bytes: usize) {
        self.memory_bytes.set(bytes as f64);
    }

    /// Update Redis connection status
    pub fn set_redis_connected(&self, connected: bool) {
        self.redis_connected.set(if connected { 1.0 } else { 0.0 });
    }

    /// Update netlink connection status
    pub fn set_netlink_connected(&self, connected: bool) {
        self.netlink_connected
            .set(if connected { 1.0 } else { 0.0 });
    }

    /// Update health status
    pub fn set_health_status(&self, status: HealthStatus) {
        let value = match status {
            HealthStatus::Healthy => 1.0,
            HealthStatus::Degraded => 0.5,
            HealthStatus::Unhealthy => 0.0,
        };
        self.health_status.set(value);
    }

    /// Record event processing latency
    pub fn observe_event_latency(&self, duration_secs: f64) {
        self.event_latency_seconds.observe(duration_secs);
    }

    /// Record Redis operation latency
    pub fn observe_redis_latency(&self, duration_secs: f64) {
        self.redis_latency_seconds.observe(duration_secs);
    }

    /// Record batch size
    pub fn observe_batch_size(&self, size: usize) {
        self.batch_size.observe(size as f64);
    }
}

/// Health status for the service
///
/// # NIST Controls
/// - CP-10: System Recovery - Track health during recovery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Service is fully operational
    Healthy,
    /// Service is operational but degraded
    Degraded,
    /// Service is not operational
    Unhealthy,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics collector")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new().unwrap();
        assert_eq!(collector.neighbors_processed_total.get(), 0.0);
        assert_eq!(collector.neighbors_added_total.get(), 0.0);
        assert_eq!(collector.neighbors_deleted_total.get(), 0.0);
    }

    #[test]
    fn test_record_neighbor_processed() {
        let collector = MetricsCollector::new().unwrap();
        collector.record_neighbor_processed(true);
        assert_eq!(collector.neighbors_processed_total.get(), 1.0);
        assert_eq!(collector.neighbors_added_total.get(), 1.0);

        collector.record_neighbor_processed(false);
        assert_eq!(collector.neighbors_processed_total.get(), 2.0);
        assert_eq!(collector.neighbors_deleted_total.get(), 1.0);
    }

    #[test]
    fn test_set_health_status() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_health_status(HealthStatus::Healthy);
        assert_eq!(collector.health_status.get(), 1.0);

        collector.set_health_status(HealthStatus::Degraded);
        assert_eq!(collector.health_status.get(), 0.5);

        collector.set_health_status(HealthStatus::Unhealthy);
        assert_eq!(collector.health_status.get(), 0.0);
    }

    #[test]
    fn test_redis_connection_status() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_redis_connected(true);
        assert_eq!(collector.redis_connected.get(), 1.0);

        collector.set_redis_connected(false);
        assert_eq!(collector.redis_connected.get(), 0.0);
    }
}
