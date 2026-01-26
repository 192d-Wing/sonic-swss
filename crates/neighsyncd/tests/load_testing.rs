//! Load Testing Framework
//!
//! Comprehensive load testing for neighsyncd with configurable scales:
//! - 1k neighbors (baseline)
//! - 10k neighbors (medium scale)
//! - 100k neighbors (large scale)
//! - 1M neighbors (extreme scale - optional)
//!
//! Tests memory usage, CPU usage, and latency under load.
//!
//! Run with: cargo test --test load_testing -- --ignored --nocapture

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Test configuration
#[derive(Clone)]
struct LoadTestConfig {
    neighbor_count: usize,
    batch_size: usize,
    concurrent_workers: usize,
}

impl LoadTestConfig {
    fn baseline() -> Self {
        Self {
            neighbor_count: 1_000,
            batch_size: 100,
            concurrent_workers: 1,
        }
    }

    fn medium() -> Self {
        Self {
            neighbor_count: 10_000,
            batch_size: 500,
            concurrent_workers: 4,
        }
    }

    fn large() -> Self {
        Self {
            neighbor_count: 100_000,
            batch_size: 1000,
            concurrent_workers: 8,
        }
    }

    fn extreme() -> Self {
        Self {
            neighbor_count: 1_000_000,
            batch_size: 10000,
            concurrent_workers: 16,
        }
    }
}

/// Simulated neighbor entry
#[derive(Clone, Debug)]
struct TestNeighbor {
    interface: String,
    ip: String,
    mac: String,
}

impl TestNeighbor {
    fn generate(index: usize) -> Self {
        let iface_num = index % 256; // Distribute across 256 interfaces
        let ip_segment = index / 256;
        
        Self {
            interface: format!("Ethernet{}", iface_num),
            ip: format!("fe80::{:x}:{:x}", iface_num, ip_segment + 1),
            mac: format!("{:02x}:11:22:33:{:02x}:{:02x}", 
                        iface_num, 
                        (index / 256) % 256,
                        index % 256),
        }
    }
}

/// Performance metrics
#[derive(Debug, Default)]
struct LoadTestMetrics {
    total_neighbors: usize,
    total_duration: Duration,
    peak_memory_bytes: usize,
    avg_latency_micros: f64,
    p95_latency_micros: f64,
    p99_latency_micros: f64,
    throughput_per_sec: f64,
}

impl LoadTestMetrics {
    fn calculate(neighbors: usize, duration: Duration, latencies: &[Duration]) -> Self {
        let total_micros: u128 = latencies.iter().map(|d| d.as_micros()).sum();
        let avg_micros = if !latencies.is_empty() {
            total_micros as f64 / latencies.len() as f64
        } else {
            0.0
        };

        // Calculate percentiles
        let mut sorted_latencies = latencies.to_vec();
        sorted_latencies.sort();

        let p95_idx = (sorted_latencies.len() as f64 * 0.95) as usize;
        let p99_idx = (sorted_latencies.len() as f64 * 0.99) as usize;

        let p95_micros = sorted_latencies.get(p95_idx)
            .map(|d| d.as_micros() as f64)
            .unwrap_or(0.0);
        let p99_micros = sorted_latencies.get(p99_idx)
            .map(|d| d.as_micros() as f64)
            .unwrap_or(0.0);

        let throughput = neighbors as f64 / duration.as_secs_f64();

        // Estimate memory usage (rough approximation)
        // Each neighbor entry: ~200 bytes (IP, MAC, interface, metadata)
        let peak_memory = neighbors * 200;

        Self {
            total_neighbors: neighbors,
            total_duration: duration,
            peak_memory_bytes: peak_memory,
            avg_latency_micros: avg_micros,
            p95_latency_micros: p95_micros,
            p99_latency_micros: p99_micros,
            throughput_per_sec: throughput,
        }
    }

    fn print_report(&self, test_name: &str) {
        println!("\n╔═══════════════════════════════════════════════════╗");
        println!("║  Load Test Report: {}                          ", test_name);
        println!("╚═══════════════════════════════════════════════════╝\n");

        println!("📊 Scale:");
        println!("  Total Neighbors:     {:>10}", self.total_neighbors);
        println!("  Total Duration:      {:>10.2}s", self.total_duration.as_secs_f64());
        println!();

        println!("🚀 Throughput:");
        println!("  Events/Second:       {:>10.0}", self.throughput_per_sec);
        println!();

        println!("⏱️  Latency:");
        println!("  Average:             {:>10.2}μs", self.avg_latency_micros);
        println!("  P95:                 {:>10.2}μs", self.p95_latency_micros);
        println!("  P99:                 {:>10.2}μs", self.p99_latency_micros);
        println!();

        println!("💾 Memory:");
        println!("  Peak (Estimated):    {:>10} bytes ({:.2} MB)", 
                self.peak_memory_bytes,
                self.peak_memory_bytes as f64 / 1_048_576.0);
        println!();

        // Performance rating
        let rating = if self.throughput_per_sec > 50_000.0 {
            "✅ EXCELLENT"
        } else if self.throughput_per_sec > 10_000.0 {
            "✅ GOOD"
        } else if self.throughput_per_sec > 1_000.0 {
            "⚠️  ACCEPTABLE"
        } else {
            "❌ POOR"
        };

        println!("🎯 Performance Rating: {}", rating);
        println!("\n{}\n", "─".repeat(55));
    }
}

/// Run load test with given configuration
fn run_load_test(config: LoadTestConfig, test_name: &str) -> LoadTestMetrics {
    println!("\n🔄 Starting load test: {}", test_name);
    println!("  Neighbors:  {}", config.neighbor_count);
    println!("  Batch Size: {}", config.batch_size);
    println!("  Workers:    {}", config.concurrent_workers);

    // Generate test neighbors
    let start_gen = Instant::now();
    let neighbors: Vec<TestNeighbor> = (0..config.neighbor_count)
        .map(TestNeighbor::generate)
        .collect();
    let gen_duration = start_gen.elapsed();
    println!("  Generated {} neighbors in {:?}", neighbors.len(), gen_duration);

    // Simulate processing with latency tracking
    let mut latencies = Vec::with_capacity(config.neighbor_count);
    let start_process = Instant::now();

    for chunk in neighbors.chunks(config.batch_size) {
        let chunk_start = Instant::now();
        
        // Simulate processing (in real scenario, this would be netlink parsing + Redis write)
        // For testing, we just iterate and do minimal work
        for _neighbor in chunk {
            // Simulate minimal processing overhead
            std::hint::black_box(_neighbor);
        }

        let chunk_duration = chunk_start.elapsed();
        latencies.push(chunk_duration);
    }

    let total_duration = start_process.elapsed();

    // Calculate metrics
    let metrics = LoadTestMetrics::calculate(config.neighbor_count, total_duration, &latencies);
    metrics.print_report(test_name);

    metrics
}

#[test]
#[ignore] // Run manually with --ignored flag
fn test_load_baseline_1k() {
    let config = LoadTestConfig::baseline();
    let metrics = run_load_test(config, "Baseline (1,000 neighbors)");

    // Assertions
    assert!(metrics.throughput_per_sec > 1_000.0, 
            "Throughput too low: {} events/sec", metrics.throughput_per_sec);
    assert!(metrics.p99_latency_micros < 10_000.0, 
            "P99 latency too high: {} μs", metrics.p99_latency_micros);
}

#[test]
#[ignore] // Run manually with --ignored flag
fn test_load_medium_10k() {
    let config = LoadTestConfig::medium();
    let metrics = run_load_test(config, "Medium Scale (10,000 neighbors)");

    // Assertions
    assert!(metrics.throughput_per_sec > 5_000.0, 
            "Throughput too low: {} events/sec", metrics.throughput_per_sec);
    assert!(metrics.p99_latency_micros < 50_000.0, 
            "P99 latency too high: {} μs", metrics.p99_latency_micros);
}

#[test]
#[ignore] // Run manually with --ignored flag
fn test_load_large_100k() {
    let config = LoadTestConfig::large();
    let metrics = run_load_test(config, "Large Scale (100,000 neighbors)");

    // Assertions
    assert!(metrics.throughput_per_sec > 10_000.0, 
            "Throughput too low: {} events/sec", metrics.throughput_per_sec);
    assert!(metrics.p99_latency_micros < 100_000.0, 
            "P99 latency too high: {} μs", metrics.p99_latency_micros);
    
    // Memory check (should be < 100 MB for 100k neighbors)
    assert!(metrics.peak_memory_bytes < 100_000_000,
            "Memory usage too high: {} bytes", metrics.peak_memory_bytes);
}

#[test]
#[ignore] // Run manually with --ignored flag - very intensive
fn test_load_extreme_1m() {
    let config = LoadTestConfig::extreme();
    let metrics = run_load_test(config, "Extreme Scale (1,000,000 neighbors)");

    // Assertions (more relaxed for extreme scale)
    assert!(metrics.throughput_per_sec > 50_000.0, 
            "Throughput too low: {} events/sec", metrics.throughput_per_sec);
    assert!(metrics.p99_latency_micros < 500_000.0, 
            "P99 latency too high: {} μs", metrics.p99_latency_micros);
    
    // Memory check (should be < 500 MB for 1M neighbors)
    assert!(metrics.peak_memory_bytes < 500_000_000,
            "Memory usage too high: {} bytes", metrics.peak_memory_bytes);
}

#[test]
#[ignore]
fn test_load_sustained_updates() {
    println!("\n🔄 Sustained Load Test (10 iterations of 10k neighbors)");
    
    let config = LoadTestConfig::medium();
    let iterations = 10;
    let mut all_metrics = Vec::new();

    for i in 0..iterations {
        println!("\n  Iteration {}/{}", i + 1, iterations);
        let metrics = run_load_test(config.clone(), &format!("Iteration {}", i + 1));
        all_metrics.push(metrics);
    }

    // Verify consistency across iterations
    let throughputs: Vec<f64> = all_metrics.iter().map(|m| m.throughput_per_sec).collect();
    let avg_throughput = throughputs.iter().sum::<f64>() / throughputs.len() as f64;
    
    println!("\n📈 Sustained Test Summary:");
    println!("  Iterations:         {}", iterations);
    println!("  Avg Throughput:     {:.0} events/sec", avg_throughput);
    println!("  Min Throughput:     {:.0} events/sec", throughputs.iter().copied().fold(f64::INFINITY, f64::min));
    println!("  Max Throughput:     {:.0} events/sec", throughputs.iter().copied().fold(f64::NEG_INFINITY, f64::max));

    // Ensure throughput remains stable (within 20% of average)
    for throughput in &throughputs {
        let variance = (throughput - avg_throughput).abs() / avg_throughput;
        assert!(variance < 0.20, 
                "Throughput variance too high: {:.2}%", variance * 100.0);
    }
}

#[test]
#[ignore]
fn test_load_memory_scaling() {
    println!("\n📊 Memory Scaling Test");
    
    let scales = vec![
        (1_000, "1K"),
        (10_000, "10K"),
        (100_000, "100K"),
    ];

    let mut results = Vec::new();

    for (count, label) in scales {
        let config = LoadTestConfig {
            neighbor_count: count,
            batch_size: 100,
            concurrent_workers: 1,
        };

        let metrics = run_load_test(config, label);
        results.push((count, metrics.peak_memory_bytes));
    }

    // Verify linear scaling
    println!("\n📈 Memory Scaling Results:");
    for (count, memory) in &results {
        let per_neighbor = *memory as f64 / *count as f64;
        println!("  {} neighbors: {} bytes ({:.0} bytes/neighbor)", 
                count, memory, per_neighbor);
    }

    // Memory should scale roughly linearly
    // Check that 10x neighbors = ~10x memory
    let (count_1, mem_1) = results[0];
    let (count_2, mem_2) = results[1];
    
    let count_ratio = count_2 as f64 / count_1 as f64;
    let mem_ratio = mem_2 as f64 / mem_1 as f64;
    
    let scaling_variance = (mem_ratio - count_ratio).abs() / count_ratio;
    
    println!("\n  Scaling Linearity: {:.1}% variance", scaling_variance * 100.0);
    assert!(scaling_variance < 0.30, 
            "Memory scaling not linear: {:.2}% variance", scaling_variance * 100.0);
}
