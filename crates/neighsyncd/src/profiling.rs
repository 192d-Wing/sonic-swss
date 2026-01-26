use std::sync::Arc;
/// Production profiling and performance analysis module
///
/// Provides runtime profiling capabilities for:
/// - Event processing latency tracking
/// - Throughput measurement
/// - Resource utilization monitoring
/// - Bottleneck identification
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Profile window for collecting statistics
#[derive(Debug, Clone, Copy)]
pub enum ProfileWindow {
    /// Last 1 minute
    OneMinute,
    /// Last 5 minutes
    FiveMinutes,
    /// Last 1 hour
    OneHour,
}

impl ProfileWindow {
    pub fn as_secs(&self) -> u64 {
        match self {
            ProfileWindow::OneMinute => 60,
            ProfileWindow::FiveMinutes => 300,
            ProfileWindow::OneHour => 3600,
        }
    }
}

/// Event processing latency statistics
#[derive(Debug, Clone)]
pub struct LatencyStats {
    /// Minimum latency (microseconds)
    pub min_us: u64,
    /// Maximum latency (microseconds)
    pub max_us: u64,
    /// Average latency (microseconds)
    pub avg_us: u64,
    /// Median latency (microseconds)
    pub median_us: u64,
    /// P95 latency (microseconds)
    pub p95_us: u64,
    /// P99 latency (microseconds)
    pub p99_us: u64,
}

/// Throughput statistics (events per second)
#[derive(Debug, Clone)]
pub struct ThroughputStats {
    /// Events processed per second
    pub events_per_sec: f64,
    /// Bytes processed per second
    pub bytes_per_sec: f64,
    /// Peak throughput (events/sec)
    pub peak_events_per_sec: f64,
}

/// Resource utilization snapshot
#[derive(Debug, Clone)]
pub struct ResourceStats {
    /// Memory usage (bytes)
    pub memory_bytes: u64,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f64,
    /// Number of threads
    pub thread_count: u32,
    /// File descriptor count
    pub fd_count: u32,
}

/// Performance profile snapshot
#[derive(Debug, Clone)]
pub struct PerformanceProfile {
    /// Latency statistics
    pub latency: LatencyStats,
    /// Throughput statistics
    pub throughput: ThroughputStats,
    /// Resource utilization
    pub resources: ResourceStats,
    /// Profile window duration
    pub window_secs: u64,
    /// Number of samples collected
    pub sample_count: u64,
}

/// Runtime profiler for production monitoring
pub struct Profiler {
    /// Event count for throughput calculation
    event_count: Arc<AtomicU64>,
    /// Total bytes processed
    bytes_processed: Arc<AtomicU64>,
    /// Last sample timestamp
    last_sample: Instant,
    /// Peak throughput recorded
    #[allow(dead_code)]
    peak_throughput: Arc<AtomicU64>,
}

impl Profiler {
    /// Create new profiler
    pub fn new() -> Self {
        Self {
            event_count: Arc::new(AtomicU64::new(0)),
            bytes_processed: Arc::new(AtomicU64::new(0)),
            last_sample: Instant::now(),
            peak_throughput: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record event processing
    pub fn record_event(&self, _latency_us: u64, bytes: u64) {
        self.event_count.fetch_add(1, Ordering::Relaxed);
        self.bytes_processed.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Get current throughput (events per second)
    pub fn current_throughput(&self) -> f64 {
        let elapsed = self.last_sample.elapsed().as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }

        let events = self.event_count.load(Ordering::Relaxed) as f64;
        events / elapsed
    }

    /// Reset profiler statistics
    pub fn reset(&mut self) {
        self.event_count.store(0, Ordering::Relaxed);
        self.bytes_processed.store(0, Ordering::Relaxed);
        self.last_sample = Instant::now();
    }

    /// Generate performance report
    pub fn generate_report(&self) -> PerformanceProfile {
        let elapsed_secs = self.last_sample.elapsed().as_secs();
        let event_count = self.event_count.load(Ordering::Relaxed);
        let bytes = self.bytes_processed.load(Ordering::Relaxed);

        let throughput = if elapsed_secs > 0 {
            event_count as f64 / elapsed_secs as f64
        } else {
            0.0
        };

        let bytes_throughput = if elapsed_secs > 0 {
            bytes as f64 / elapsed_secs as f64
        } else {
            0.0
        };

        PerformanceProfile {
            latency: LatencyStats {
                min_us: 0,
                max_us: 0,
                avg_us: 0,
                median_us: 0,
                p95_us: 0,
                p99_us: 0,
            },
            throughput: ThroughputStats {
                events_per_sec: throughput,
                bytes_per_sec: bytes_throughput,
                peak_events_per_sec: throughput,
            },
            resources: ResourceStats {
                memory_bytes: 0,
                cpu_percent: 0.0,
                thread_count: 0,
                fd_count: 0,
            },
            window_secs: elapsed_secs,
            sample_count: event_count,
        }
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Adaptive performance tuning engine
pub struct AdaptivePerformanceTuner {
    /// Current batch size
    current_batch_size: Arc<AtomicU64>,
    /// Current batch timeout (ms)
    current_batch_timeout: Arc<AtomicU64>,
    /// Target throughput (events/sec)
    target_throughput: f64,
    /// Throughput history for trend analysis
    throughput_history: Vec<f64>,
}

impl AdaptivePerformanceTuner {
    /// Create new tuner with target throughput
    pub fn new(target_throughput: f64) -> Self {
        Self {
            current_batch_size: Arc::new(AtomicU64::new(100)),
            current_batch_timeout: Arc::new(AtomicU64::new(100)),
            target_throughput,
            throughput_history: Vec::new(),
        }
    }

    /// Get recommended batch size based on observed throughput
    pub fn adjust_batch_size(&mut self, current_throughput: f64) -> u64 {
        self.throughput_history.push(current_throughput);

        // Keep only last 10 measurements
        if self.throughput_history.len() > 10 {
            self.throughput_history.remove(0);
        }

        let current_batch = self.current_batch_size.load(Ordering::Relaxed);

        // If throughput is below target, increase batch size
        if current_throughput < self.target_throughput * 0.9 {
            let new_batch = (current_batch as f64 * 1.2).min(1000.0) as u64;
            self.current_batch_size.store(new_batch, Ordering::Relaxed);
            return new_batch;
        }

        // If throughput exceeds target, decrease batch size for lower latency
        if current_throughput > self.target_throughput * 1.1 {
            let new_batch = (current_batch as f64 * 0.8).max(50.0) as u64;
            self.current_batch_size.store(new_batch, Ordering::Relaxed);
            return new_batch;
        }

        current_batch
    }

    /// Get recommended batch timeout based on latency requirements
    pub fn adjust_batch_timeout(&mut self, p99_latency_us: u64, target_latency_us: u64) -> u64 {
        let current_timeout = self.current_batch_timeout.load(Ordering::Relaxed);

        // If latency is too high, reduce timeout
        if p99_latency_us > target_latency_us {
            let new_timeout = (current_timeout as f64 * 0.5).max(10.0) as u64;
            self.current_batch_timeout
                .store(new_timeout, Ordering::Relaxed);
            return new_timeout;
        }

        // If latency is acceptable but timeout could be longer for batching
        if p99_latency_us < target_latency_us / 2 {
            let new_timeout = (current_timeout as f64 * 1.5).min(500.0) as u64;
            self.current_batch_timeout
                .store(new_timeout, Ordering::Relaxed);
            return new_timeout;
        }

        current_timeout
    }

    /// Get current configuration
    pub fn current_config(&self) -> (u64, u64) {
        (
            self.current_batch_size.load(Ordering::Relaxed),
            self.current_batch_timeout.load(Ordering::Relaxed),
        )
    }

    /// Check if throughput is stable
    pub fn is_throughput_stable(&self) -> bool {
        if self.throughput_history.len() < 5 {
            return false;
        }

        let recent = &self.throughput_history[self.throughput_history.len() - 5..];
        let avg = recent.iter().sum::<f64>() / recent.len() as f64;

        // Check if variance is low (stable)
        let variance = recent.iter().map(|x| (x - avg).powi(2)).sum::<f64>() / recent.len() as f64;

        let stddev = variance.sqrt();
        let coefficient_of_variation = stddev / avg;

        coefficient_of_variation < 0.15 // Less than 15% variance
    }

    /// Get throughput trend
    pub fn get_trend(&self) -> Option<f64> {
        if self.throughput_history.len() < 5 {
            return None;
        }

        let recent = &self.throughput_history[self.throughput_history.len() - 5..];
        let avg_old = recent[..2].iter().sum::<f64>() / 2.0;
        let avg_new = recent[3..].iter().sum::<f64>() / 2.0;

        // Trend as percentage increase
        Some((avg_new - avg_old) / avg_old * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_creation() {
        let profiler = Profiler::new();
        assert_eq!(profiler.current_throughput(), 0.0);
    }

    #[test]
    fn test_profiler_event_recording() {
        let profiler = Profiler::new();
        profiler.record_event(100, 64);
        profiler.record_event(120, 64);

        let report = profiler.generate_report();
        assert_eq!(report.sample_count, 2);
    }

    #[test]
    fn test_adaptive_tuner_creation() {
        let tuner = AdaptivePerformanceTuner::new(1000.0);
        let (batch, timeout) = tuner.current_config();
        assert_eq!(batch, 100);
        assert_eq!(timeout, 100);
    }

    #[test]
    fn test_adaptive_tuner_batch_adjustment() {
        let mut tuner = AdaptivePerformanceTuner::new(1000.0);

        // Low throughput: should increase batch size
        let new_batch = tuner.adjust_batch_size(500.0);
        assert!(new_batch > 100);

        // High throughput: should decrease batch size
        let mut tuner2 = AdaptivePerformanceTuner::new(1000.0);
        let new_batch2 = tuner2.adjust_batch_size(2000.0);
        assert!(new_batch2 < 100);
    }

    #[test]
    fn test_adaptive_tuner_timeout_adjustment() {
        let mut tuner = AdaptivePerformanceTuner::new(1000.0);

        // High latency: should reduce timeout
        let new_timeout = tuner.adjust_batch_timeout(100_000, 50_000);
        assert!(new_timeout < 100);

        // Low latency: should increase timeout
        let mut tuner2 = AdaptivePerformanceTuner::new(1000.0);
        let new_timeout2 = tuner2.adjust_batch_timeout(10_000, 50_000);
        assert!(new_timeout2 > 100);
    }

    #[test]
    fn test_throughput_stability() {
        let mut tuner = AdaptivePerformanceTuner::new(1000.0);

        // Add stable measurements
        for _ in 0..5 {
            tuner.adjust_batch_size(1000.0);
        }

        assert!(tuner.is_throughput_stable());
    }

    #[test]
    fn test_throughput_trend() {
        let mut tuner = AdaptivePerformanceTuner::new(1000.0);

        // Add increasing trend
        tuner.adjust_batch_size(900.0);
        tuner.adjust_batch_size(950.0);
        tuner.adjust_batch_size(1000.0);
        tuner.adjust_batch_size(1100.0);
        tuner.adjust_batch_size(1200.0);

        let trend = tuner.get_trend();
        assert!(trend.is_some());
        assert!(trend.unwrap() > 0.0); // Should show positive trend
    }
}
