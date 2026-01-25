//! Prometheus metrics collection for portsyncd
//!
//! Collects performance and operational metrics and exposes them via
//! a Prometheus-compatible HTTP endpoint (/metrics).
//!
//! Phase 6 Week 1 implementation.

use prometheus::{
    Counter, CounterVec, Encoder, Gauge, Histogram, HistogramOpts, Registry, TextEncoder,
};
use std::sync::Arc;

/// Prometheus metrics collector for portsyncd
#[derive(Clone)]
pub struct MetricsCollector {
    // Counters
    events_processed: Counter,
    events_failed: Counter,
    port_flaps: CounterVec,

    // Gauges
    queue_depth: Gauge,
    memory_bytes: Gauge,
    health_status: Gauge,
    redis_connected: Gauge,
    netlink_connected: Gauge,

    // Histograms
    event_latency_seconds: Histogram,
    redis_latency_seconds: Histogram,

    registry: Arc<Registry>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // Counters
        let events_processed = Counter::new(
            "portsyncd_events_processed_total",
            "Total events processed successfully",
        )?;
        registry.register(Box::new(events_processed.clone()))?;

        let events_failed = Counter::new(
            "portsyncd_events_failed_total",
            "Total events that failed to process",
        )?;
        registry.register(Box::new(events_failed.clone()))?;

        let port_flaps = prometheus::CounterVec::new(
            prometheus::Opts::new("portsyncd_port_flaps_total", "Port flap count by port"),
            &["port"],
        )?;
        registry.register(Box::new(port_flaps.clone()))?;

        // Gauges
        let queue_depth = Gauge::new("portsyncd_queue_depth", "Current event queue depth")?;
        registry.register(Box::new(queue_depth.clone()))?;

        let memory_bytes = Gauge::new("portsyncd_memory_bytes", "Process memory usage in bytes")?;
        registry.register(Box::new(memory_bytes.clone()))?;

        let health_status = Gauge::new(
            "portsyncd_health_status",
            "Health status (1=healthy, 0.5=degraded, 0=unhealthy)",
        )?;
        registry.register(Box::new(health_status.clone()))?;

        let redis_connected = Gauge::new(
            "portsyncd_redis_connected",
            "Redis connection status (1=connected, 0=disconnected)",
        )?;
        registry.register(Box::new(redis_connected.clone()))?;

        let netlink_connected = Gauge::new(
            "portsyncd_netlink_connected",
            "Netlink socket status (1=open, 0=closed)",
        )?;
        registry.register(Box::new(netlink_connected.clone()))?;

        // Histograms
        let event_latency_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "portsyncd_event_latency_seconds",
                "Event processing latency in seconds",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
        )?;
        registry.register(Box::new(event_latency_seconds.clone()))?;

        let redis_latency_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "portsyncd_redis_latency_seconds",
                "Redis operation latency in seconds",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1]),
        )?;
        registry.register(Box::new(redis_latency_seconds.clone()))?;

        Ok(Self {
            events_processed,
            events_failed,
            port_flaps,
            queue_depth,
            memory_bytes,
            health_status,
            redis_connected,
            netlink_connected,
            event_latency_seconds,
            redis_latency_seconds,
            registry: Arc::new(registry),
        })
    }

    /// Record successful event processing
    pub fn record_event_success(&self) {
        self.events_processed.inc();
    }

    /// Record failed event
    pub fn record_event_failure(&self) {
        self.events_failed.inc();
    }

    /// Record port flap
    pub fn record_port_flap(&self, port_name: &str) {
        self.port_flaps.with_label_values(&[port_name]).inc();
    }

    /// Set queue depth gauge
    pub fn set_queue_depth(&self, depth: usize) {
        self.queue_depth.set(depth as f64);
    }

    /// Set memory usage gauge
    pub fn set_memory_bytes(&self, bytes: u64) {
        self.memory_bytes.set(bytes as f64);
    }

    /// Set health status gauge (1.0 = healthy, 0.5 = degraded, 0.0 = unhealthy)
    pub fn set_health_status(&self, status: f64) {
        self.health_status.set(status);
    }

    /// Set Redis connection status
    pub fn set_redis_connected(&self, connected: bool) {
        self.redis_connected.set(if connected { 1.0 } else { 0.0 });
    }

    /// Set netlink socket status
    pub fn set_netlink_connected(&self, connected: bool) {
        self.netlink_connected
            .set(if connected { 1.0 } else { 0.0 });
    }

    /// Start event latency timer
    pub fn start_event_latency(&self) -> prometheus::HistogramTimer {
        self.event_latency_seconds.start_timer()
    }

    /// Start Redis latency timer
    pub fn start_redis_latency(&self) -> prometheus::HistogramTimer {
        self.redis_latency_seconds.start_timer()
    }

    /// Gather metrics in Prometheus text format
    pub fn gather_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let mut buf = vec![];
        encoder.encode(&self.registry.gather(), &mut buf).ok();
        String::from_utf8(buf).unwrap_or_else(|_| String::from("# Error encoding metrics\n"))
    }
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
        let collector = MetricsCollector::new();
        assert!(collector.is_ok());
    }

    #[test]
    fn test_record_event_success() {
        let collector = MetricsCollector::new().unwrap();
        collector.record_event_success();
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_events_processed_total 1"));
    }

    #[test]
    fn test_record_event_failure() {
        let collector = MetricsCollector::new().unwrap();
        collector.record_event_failure();
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_events_failed_total 1"));
    }

    #[test]
    fn test_record_port_flap() {
        let collector = MetricsCollector::new().unwrap();
        collector.record_port_flap("Ethernet0");
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_port_flaps_total"));
        assert!(metrics.contains("Ethernet0"));
    }

    #[test]
    fn test_set_queue_depth() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_queue_depth(42);
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_queue_depth 42"));
    }

    #[test]
    fn test_set_memory_bytes() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_memory_bytes(52428800);
        let metrics = collector.gather_metrics();
        // Check for the metric being present rather than exact formatting
        assert!(metrics.contains("portsyncd_memory_bytes ") && metrics.contains("52428800"));
    }

    #[test]
    fn test_set_health_status_healthy() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_health_status(1.0);
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_health_status 1"));
    }

    #[test]
    fn test_set_health_status_degraded() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_health_status(0.5);
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_health_status 0.5"));
    }

    #[test]
    fn test_set_redis_connected() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_redis_connected(true);
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_redis_connected 1"));
    }

    #[test]
    fn test_set_netlink_connected() {
        let collector = MetricsCollector::new().unwrap();
        collector.set_netlink_connected(true);
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_netlink_connected 1"));
    }

    #[test]
    fn test_event_latency_histogram() {
        let collector = MetricsCollector::new().unwrap();
        let timer = collector.start_event_latency();
        drop(timer); // Observe the timer
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("portsyncd_event_latency_seconds_bucket"));
    }

    #[test]
    fn test_gather_metrics_format() {
        let collector = MetricsCollector::new().unwrap();
        let metrics = collector.gather_metrics();
        assert!(metrics.contains("# HELP"));
        assert!(metrics.contains("# TYPE"));
        assert!(metrics.contains("portsyncd_"));
    }
}
