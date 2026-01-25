//! Performance monitoring and benchmarking for portsyncd
//!
//! Tracks event processing latency, throughput, and system resource usage.
//! Supports comparison against C++ portsyncd baseline implementation.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Performance metrics for portsyncd
#[derive(Clone, Debug)]
pub struct PerformanceMetrics {
    /// Total events processed
    events_processed: Arc<AtomicU64>,
    /// Total events failed
    events_failed: Arc<AtomicU64>,
    /// Timestamp of last event
    last_event_time: Arc<std::sync::Mutex<Option<Instant>>>,
    /// Total processing time
    total_processing_us: Arc<AtomicU64>,
}

impl PerformanceMetrics {
    /// Create new performance metrics tracker
    pub fn new() -> Self {
        Self {
            events_processed: Arc::new(AtomicU64::new(0)),
            events_failed: Arc::new(AtomicU64::new(0)),
            last_event_time: Arc::new(std::sync::Mutex::new(None)),
            total_processing_us: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record event start time
    pub fn start_event(&self) -> EventTimer {
        EventTimer {
            start: Instant::now(),
            metrics: self.clone(),
        }
    }

    /// Record a successful event
    pub fn record_event(&self, duration_us: u64) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);
        self.total_processing_us
            .fetch_add(duration_us, Ordering::Relaxed);
        if let Ok(mut last) = self.last_event_time.lock() {
            *last = Some(Instant::now());
        }
    }

    /// Record a failed event
    pub fn record_failure(&self) {
        self.events_failed.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total events processed
    pub fn total_events(&self) -> u64 {
        self.events_processed.load(Ordering::Relaxed)
    }

    /// Get total events failed
    pub fn total_failures(&self) -> u64 {
        self.events_failed.load(Ordering::Relaxed)
    }

    /// Get average event processing latency in microseconds
    pub fn average_latency_us(&self) -> u64 {
        let total = self.total_events();
        if total == 0 {
            return 0;
        }
        let total_us = self.total_processing_us.load(Ordering::Relaxed);
        total_us / total
    }

    /// Get throughput in events per second
    pub fn throughput_eps(&self) -> f64 {
        let total = self.total_events();
        let total_us = self.total_processing_us.load(Ordering::Relaxed);
        if total_us == 0 {
            return 0.0;
        }
        (total as f64 / total_us as f64) * 1_000_000.0
    }

    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        let processed = self.total_events();
        let failed = self.total_failures();
        let total = processed + failed;
        if total == 0 {
            return 100.0;
        }
        (processed as f64 / total as f64) * 100.0
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.events_processed.store(0, Ordering::Relaxed);
        self.events_failed.store(0, Ordering::Relaxed);
        self.total_processing_us.store(0, Ordering::Relaxed);
        if let Ok(mut last) = self.last_event_time.lock() {
            *last = None;
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Timer for tracking event processing latency
pub struct EventTimer {
    start: Instant,
    metrics: PerformanceMetrics,
}

impl EventTimer {
    /// Record successful event completion
    pub fn complete(self) {
        let duration = self.start.elapsed();
        let duration_us = duration.as_micros() as u64;
        self.metrics.record_event(duration_us);
    }

    /// Record event failure
    pub fn fail(self) {
        self.metrics.record_failure();
    }

    /// Get elapsed duration since start
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

/// Benchmark configuration for performance testing
#[derive(Clone, Debug)]
pub struct BenchmarkConfig {
    /// Number of test events to process
    pub num_events: u64,
    /// Target events per second (for load testing)
    pub target_eps: u64,
    /// Maximum acceptable average latency in microseconds
    pub max_latency_us: u64,
    /// Minimum acceptable success rate (percentage)
    pub min_success_rate: f64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            num_events: 1000,
            target_eps: 1000,
            max_latency_us: 10000, // 10ms max latency
            min_success_rate: 99.5,
        }
    }
}

impl BenchmarkConfig {
    /// Create benchmark for small-scale testing
    pub fn small() -> Self {
        Self {
            num_events: 100,
            target_eps: 100,
            max_latency_us: 50000,
            min_success_rate: 99.0,
        }
    }

    /// Create benchmark for large-scale testing
    pub fn large() -> Self {
        Self {
            num_events: 10000,
            target_eps: 5000,
            max_latency_us: 5000, // 5ms max latency
            min_success_rate: 99.9,
        }
    }
}

/// Benchmark result comparing performance against baseline
#[derive(Clone, Debug)]
pub struct BenchmarkResult {
    /// Total events processed
    pub events_processed: u64,
    /// Average latency in microseconds
    pub avg_latency_us: u64,
    /// Throughput in events per second
    pub throughput_eps: f64,
    /// Success rate as percentage
    pub success_rate: f64,
    /// Duration of benchmark
    pub duration: Duration,
    /// Pass/fail against configuration
    pub passed: bool,
}

impl BenchmarkResult {
    /// Create benchmark result from metrics and config
    pub fn from_metrics(
        metrics: &PerformanceMetrics,
        duration: Duration,
        config: &BenchmarkConfig,
    ) -> Self {
        let avg_latency_us = metrics.average_latency_us();
        let throughput_eps = metrics.throughput_eps();
        let success_rate = metrics.success_rate();

        let passed = avg_latency_us <= config.max_latency_us
            && throughput_eps >= config.target_eps as f64 * 0.9
            && success_rate >= config.min_success_rate;

        Self {
            events_processed: metrics.total_events(),
            avg_latency_us,
            throughput_eps,
            success_rate,
            duration,
            passed,
        }
    }

    /// Format result for display
    pub fn format_report(&self) -> String {
        format!(
            "Benchmark Results:\n  Events: {}\n  Avg Latency: {}us\n  Throughput: {:.1} eps\n  Success Rate: {:.2}%\n  Duration: {:.2}s\n  Status: {}\n",
            self.events_processed,
            self.avg_latency_us,
            self.throughput_eps,
            self.success_rate,
            self.duration.as_secs_f64(),
            if self.passed { "PASSED" } else { "FAILED" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_metrics_creation() {
        let metrics = PerformanceMetrics::new();
        assert_eq!(metrics.total_events(), 0);
        assert_eq!(metrics.total_failures(), 0);
    }

    #[test]
    fn test_record_event() {
        let metrics = PerformanceMetrics::new();
        metrics.record_event(1000); // 1ms
        assert_eq!(metrics.total_events(), 1);
        assert_eq!(metrics.average_latency_us(), 1000);
    }

    #[test]
    fn test_multiple_events() {
        let metrics = PerformanceMetrics::new();
        metrics.record_event(1000);
        metrics.record_event(2000);
        metrics.record_event(3000);
        assert_eq!(metrics.total_events(), 3);
        assert_eq!(metrics.average_latency_us(), 2000);
    }

    #[test]
    fn test_throughput_calculation() {
        let metrics = PerformanceMetrics::new();
        // 1000 events in 1 second
        for _ in 0..1000 {
            metrics.record_event(1000);
        }
        let eps = metrics.throughput_eps();
        assert!(eps > 900.0); // Allow some variance
    }

    #[test]
    fn test_success_rate() {
        let metrics = PerformanceMetrics::new();
        metrics.record_event(1000);
        metrics.record_event(1000);
        metrics.record_failure();
        let rate = metrics.success_rate();
        assert!(rate > 66.0 && rate < 67.0);
    }

    #[test]
    fn test_reset_metrics() {
        let metrics = PerformanceMetrics::new();
        metrics.record_event(1000);
        assert_eq!(metrics.total_events(), 1);
        metrics.reset();
        assert_eq!(metrics.total_events(), 0);
    }

    #[test]
    fn test_event_timer() {
        let metrics = PerformanceMetrics::new();
        let timer = metrics.start_event();
        std::thread::sleep(Duration::from_micros(100));
        timer.complete();
        assert_eq!(metrics.total_events(), 1);
        assert!(metrics.average_latency_us() >= 100);
    }

    #[test]
    fn test_event_timer_fail() {
        let metrics = PerformanceMetrics::new();
        let timer = metrics.start_event();
        timer.fail();
        assert_eq!(metrics.total_failures(), 1);
    }

    #[test]
    fn test_benchmark_config_default() {
        let config = BenchmarkConfig::default();
        assert_eq!(config.num_events, 1000);
        assert_eq!(config.target_eps, 1000);
        assert_eq!(config.max_latency_us, 10000);
    }

    #[test]
    fn test_benchmark_result() {
        let metrics = PerformanceMetrics::new();
        metrics.record_event(5000);
        let config = BenchmarkConfig::default();
        let result = BenchmarkResult::from_metrics(&metrics, Duration::from_secs(1), &config);
        assert_eq!(result.events_processed, 1);
        assert!(result.success_rate >= 99.0);
    }

    #[test]
    fn test_benchmark_report_format() {
        let metrics = PerformanceMetrics::new();
        metrics.record_event(5000);
        let config = BenchmarkConfig::default();
        let result = BenchmarkResult::from_metrics(&metrics, Duration::from_millis(100), &config);
        let report = result.format_report();
        assert!(report.contains("Benchmark Results"));
        assert!(report.contains("Events: 1"));
    }
}
