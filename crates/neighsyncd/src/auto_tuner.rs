use std::sync::Arc;
/// Automatic configuration tuning based on runtime metrics
///
/// Monitors performance and automatically adjusts:
/// - Batch size (50-1000 neighbors)
/// - Batch timeout (10-500ms)
/// - Worker threads (1-16)
/// - Socket buffer size (adaptive)
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Auto-tuning configuration
#[derive(Debug, Clone)]
pub struct AutoTuningConfig {
    /// Enable automatic tuning
    pub enabled: bool,
    /// Tuning interval (seconds)
    pub tuning_interval_secs: u64,
    /// Target latency (microseconds)
    pub target_latency_us: u64,
    /// Target throughput (events/sec)
    pub target_throughput: f64,
    /// Min batch size
    pub min_batch_size: u64,
    /// Max batch size
    pub max_batch_size: u64,
    /// Min batch timeout (ms)
    pub min_batch_timeout_ms: u64,
    /// Max batch timeout (ms)
    pub max_batch_timeout_ms: u64,
    /// Min worker threads
    pub min_threads: u32,
    /// Max worker threads
    pub max_threads: u32,
}

impl Default for AutoTuningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tuning_interval_secs: 60,
            target_latency_us: 50_000, // 50ms
            target_throughput: 1000.0, // 1000 events/sec
            min_batch_size: 50,
            max_batch_size: 1000,
            min_batch_timeout_ms: 10,
            max_batch_timeout_ms: 500,
            min_threads: 1,
            max_threads: 16,
        }
    }
}

/// Runtime metrics for tuning decisions
#[derive(Debug, Clone)]
pub struct TuningMetrics {
    /// Current throughput (events/sec)
    pub throughput: f64,
    /// P99 latency (microseconds)
    pub latency_p99_us: u64,
    /// Error rate (0.0-1.0)
    pub error_rate: f64,
    /// Memory usage (bytes)
    pub memory_bytes: u64,
    /// CPU usage (0.0-1.0)
    pub cpu_usage: f64,
    /// Queue depth
    pub queue_depth: u64,
}

/// Tuning recommendations from analysis
#[derive(Debug, Clone)]
pub struct TuningRecommendation {
    /// Recommended batch size
    pub batch_size: u64,
    /// Recommended batch timeout (ms)
    pub batch_timeout_ms: u64,
    /// Recommended worker threads
    pub worker_threads: u32,
    /// Reasoning for recommendation
    pub reason: String,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// Auto-tuning engine
pub struct AutoTuner {
    config: AutoTuningConfig,
    current_batch_size: Arc<AtomicU64>,
    current_batch_timeout: Arc<AtomicU64>,
    current_threads: Arc<AtomicU32>,
    /// Last tuning timestamp
    last_tuning_time: Arc<AtomicU64>,
    /// Number of tuning iterations
    iteration_count: Arc<AtomicU64>,
}

impl AutoTuner {
    /// Create new auto-tuner
    pub fn new(config: AutoTuningConfig) -> Self {
        Self {
            config,
            current_batch_size: Arc::new(AtomicU64::new(100)),
            current_batch_timeout: Arc::new(AtomicU64::new(100)),
            current_threads: Arc::new(AtomicU32::new(4)),
            last_tuning_time: Arc::new(AtomicU64::new(0)),
            iteration_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Analyze metrics and generate tuning recommendation
    pub fn analyze(&mut self, metrics: &TuningMetrics) -> TuningRecommendation {
        let mut reason = String::new();
        let mut confidence: f64 = 0.8; // Base confidence

        // Determine batch size
        let (batch_size, batch_reason, batch_confidence) = self.calculate_batch_size(metrics);
        reason.push_str(&format!("Batch: {} ({}). ", batch_size, batch_reason));
        confidence = f64::min(confidence, batch_confidence);

        // Determine batch timeout
        let (batch_timeout, timeout_reason, timeout_confidence) =
            self.calculate_batch_timeout(metrics);
        reason.push_str(&format!(
            "Timeout: {}ms ({}). ",
            batch_timeout, timeout_reason
        ));
        confidence = f64::min(confidence, timeout_confidence);

        // Determine worker threads
        let (threads, thread_reason, thread_confidence) = self.calculate_worker_threads(metrics);
        reason.push_str(&format!("Threads: {} ({})", threads, thread_reason));
        confidence = f64::min(confidence, thread_confidence);

        // Store current config
        self.current_batch_size.store(batch_size, Ordering::Relaxed);
        self.current_batch_timeout
            .store(batch_timeout, Ordering::Relaxed);
        self.current_threads.store(threads, Ordering::Relaxed);
        self.iteration_count.fetch_add(1, Ordering::Relaxed);

        TuningRecommendation {
            batch_size,
            batch_timeout_ms: batch_timeout,
            worker_threads: threads,
            reason,
            confidence,
        }
    }

    /// Calculate optimal batch size
    fn calculate_batch_size(&self, metrics: &TuningMetrics) -> (u64, String, f64) {
        let current = self.current_batch_size.load(Ordering::Relaxed);

        // If throughput is too low, increase batch size
        if metrics.throughput < self.config.target_throughput * 0.8 {
            let increase = ((current as f64 * 1.1).min(self.config.max_batch_size as f64)) as u64;
            return (increase, "Low throughput".to_string(), 0.85);
        }

        // If error rate is high, decrease batch size
        if metrics.error_rate > 0.05 {
            let decrease = ((current as f64 * 0.9).max(self.config.min_batch_size as f64)) as u64;
            return (decrease, "High error rate".to_string(), 0.95);
        }

        // If memory is high, decrease batch size
        if metrics.memory_bytes > 150 * 1024 * 1024 {
            let decrease = ((current as f64 * 0.85).max(self.config.min_batch_size as f64)) as u64;
            return (decrease, "High memory usage".to_string(), 0.9);
        }

        // If queue depth is high, increase batch size for efficiency
        if metrics.queue_depth > 1000 {
            let increase = ((current as f64 * 1.05).min(self.config.max_batch_size as f64)) as u64;
            return (increase, "High queue depth".to_string(), 0.8);
        }

        (current, "Optimal".to_string(), 0.99)
    }

    /// Calculate optimal batch timeout
    fn calculate_batch_timeout(&self, metrics: &TuningMetrics) -> (u64, String, f64) {
        let current = self.current_batch_timeout.load(Ordering::Relaxed);

        // If latency is too high, reduce timeout for faster flushes
        if metrics.latency_p99_us > self.config.target_latency_us {
            let reduce =
                ((current as f64 * 0.5).max(self.config.min_batch_timeout_ms as f64)) as u64;
            return (reduce, "High latency".to_string(), 0.9);
        }

        // If latency is very low, increase timeout for better batching
        if metrics.latency_p99_us < self.config.target_latency_us / 2 {
            let increase =
                ((current as f64 * 1.5).min(self.config.max_batch_timeout_ms as f64)) as u64;
            return (increase, "Low latency opportunity".to_string(), 0.85);
        }

        // If throughput is low, increase timeout to allow more batching
        if metrics.throughput < self.config.target_throughput * 0.7 {
            let increase =
                ((current as f64 * 1.2).min(self.config.max_batch_timeout_ms as f64)) as u64;
            return (
                increase,
                "Low throughput, allow more batching".to_string(),
                0.8,
            );
        }

        (current, "Optimal".to_string(), 0.99)
    }

    /// Calculate optimal worker threads
    fn calculate_worker_threads(&self, metrics: &TuningMetrics) -> (u32, String, f64) {
        let current = self.current_threads.load(Ordering::Relaxed);

        // If CPU usage is low and throughput is also low, add threads
        if metrics.cpu_usage < 0.5 && metrics.throughput < self.config.target_throughput * 0.8 {
            let increase = (current + 1).min(self.config.max_threads);
            return (increase, "CPU available, throughput low".to_string(), 0.85);
        }

        // If CPU usage is high, reduce threads to avoid contention
        if metrics.cpu_usage > 0.9 && current > self.config.min_threads {
            let decrease = (current - 1).max(self.config.min_threads);
            return (decrease, "High CPU usage".to_string(), 0.9);
        }

        // If memory is high, reduce threads to reduce memory footprint
        if metrics.memory_bytes > 180 * 1024 * 1024 && current > self.config.min_threads {
            let decrease = (current - 1).max(self.config.min_threads);
            return (decrease, "High memory usage".to_string(), 0.8);
        }

        (current, "Optimal".to_string(), 0.99)
    }

    /// Get current configuration
    pub fn current_config(&self) -> (u64, u64, u32) {
        (
            self.current_batch_size.load(Ordering::Relaxed),
            self.current_batch_timeout.load(Ordering::Relaxed),
            self.current_threads.load(Ordering::Relaxed),
        )
    }

    /// Get tuning statistics
    pub fn stats(&self) -> (u64, u64) {
        (
            self.iteration_count.load(Ordering::Relaxed),
            self.last_tuning_time.load(Ordering::Relaxed),
        )
    }

    /// Should tuning be performed now?
    pub fn should_tune(&self, current_time_secs: u64) -> bool {
        if !self.config.enabled {
            return false;
        }

        let last_tune = self.last_tuning_time.load(Ordering::Relaxed);
        current_time_secs - last_tune >= self.config.tuning_interval_secs
    }

    /// Mark that tuning was performed
    pub fn mark_tuned(&self, current_time_secs: u64) {
        self.last_tuning_time
            .store(current_time_secs, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_tuner_creation() {
        let tuner = AutoTuner::new(AutoTuningConfig::default());
        let (batch, timeout, threads) = tuner.current_config();
        assert_eq!(batch, 100);
        assert_eq!(timeout, 100);
        assert_eq!(threads, 4);
    }

    #[test]
    fn test_low_throughput_recommendation() {
        let mut tuner = AutoTuner::new(AutoTuningConfig::default());
        let metrics = TuningMetrics {
            throughput: 500.0, // Below target
            latency_p99_us: 30_000,
            error_rate: 0.01,
            memory_bytes: 50 * 1024 * 1024,
            cpu_usage: 0.5,
            queue_depth: 50,
        };

        let rec = tuner.analyze(&metrics);
        assert!(rec.batch_size > 100); // Should increase batch size
    }

    #[test]
    fn test_high_latency_recommendation() {
        let mut tuner = AutoTuner::new(AutoTuningConfig::default());
        let metrics = TuningMetrics {
            throughput: 1000.0,
            latency_p99_us: 100_000, // Above target
            error_rate: 0.01,
            memory_bytes: 50 * 1024 * 1024,
            cpu_usage: 0.5,
            queue_depth: 50,
        };

        let rec = tuner.analyze(&metrics);
        assert!(rec.batch_timeout_ms < 100); // Should reduce timeout
    }

    #[test]
    fn test_high_error_rate_recommendation() {
        let mut tuner = AutoTuner::new(AutoTuningConfig::default());
        let metrics = TuningMetrics {
            throughput: 1000.0,
            latency_p99_us: 30_000,
            error_rate: 0.10, // High error rate
            memory_bytes: 50 * 1024 * 1024,
            cpu_usage: 0.5,
            queue_depth: 50,
        };

        let rec = tuner.analyze(&metrics);
        assert!(rec.batch_size < 100); // Should decrease batch size
    }

    #[test]
    fn test_should_tune() {
        let tuner = AutoTuner::new(AutoTuningConfig::default());
        assert!(tuner.should_tune(100));

        tuner.mark_tuned(100);
        assert!(!tuner.should_tune(120)); // Within interval
        assert!(tuner.should_tune(200)); // Past interval
    }

    #[test]
    fn test_high_cpu_recommendation() {
        let mut tuner = AutoTuner::new(AutoTuningConfig::default());
        let metrics = TuningMetrics {
            throughput: 1000.0,
            latency_p99_us: 30_000,
            error_rate: 0.01,
            memory_bytes: 50 * 1024 * 1024,
            cpu_usage: 0.95, // High CPU
            queue_depth: 50,
        };

        let rec = tuner.analyze(&metrics);
        assert!(rec.worker_threads < 4); // Should reduce threads
    }
}
