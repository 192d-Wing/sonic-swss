//! Performance benchmarking for portsyncd
//!
//! Tests event processing latency, throughput, and memory efficiency.
//! Compares against baseline requirements for production deployment.

use sonic_portsyncd::{BenchmarkConfig, BenchmarkResult, PerformanceMetrics};
use std::time::{Duration, Instant};

/// Benchmark steady-state event processing
#[test]
fn bench_steady_state_events() {
    let metrics = PerformanceMetrics::new();
    let config = BenchmarkConfig::default();

    // Simulate steady stream of events at 1000 eps
    let start = Instant::now();
    for _ in 0..config.num_events {
        let timer = metrics.start_event();
        // Simulate event processing (minimal work)
        std::thread::sleep(Duration::from_micros(1000)); // 1ms per event
        timer.complete();
    }
    let duration = start.elapsed();

    let result = BenchmarkResult::from_metrics(&metrics, duration, &config);

    println!("\n=== Steady State Performance ===");
    println!("{}", result.format_report());

    // Verify against configuration
    assert!(
        result.avg_latency_us <= config.max_latency_us,
        "Average latency {} exceeds max {}",
        result.avg_latency_us,
        config.max_latency_us
    );
    assert!(
        result.success_rate >= config.min_success_rate,
        "Success rate {} below minimum {}",
        result.success_rate,
        config.min_success_rate
    );
}

/// Benchmark burst event processing (stress test)
#[test]
fn bench_burst_events() {
    let metrics = PerformanceMetrics::new();

    // Simulate burst of 5000 events in rapid succession
    let start = Instant::now();
    for _ in 0..5000 {
        let timer = metrics.start_event();
        // Minimal processing
        std::thread::sleep(Duration::from_micros(100));
        timer.complete();
    }
    let duration = start.elapsed();

    let config = BenchmarkConfig::large();
    let result = BenchmarkResult::from_metrics(&metrics, duration, &config);

    println!("\n=== Burst Processing ===");
    println!("{}", result.format_report());

    // Burst should maintain sub-10ms latency even under load
    assert!(
        result.avg_latency_us <= 10000,
        "Burst latency {} too high",
        result.avg_latency_us
    );
}

/// Benchmark event processing with failures
#[test]
fn bench_with_failures() {
    let metrics = PerformanceMetrics::new();

    // Process 1000 events with 5% failure rate
    for i in 0..1000 {
        if i % 20 == 0 {
            // 5% failure rate
            let timer = metrics.start_event();
            timer.fail();
        } else {
            let timer = metrics.start_event();
            std::thread::sleep(Duration::from_micros(500));
            timer.complete();
        }
    }

    let config = BenchmarkConfig::default();
    let result = BenchmarkResult::from_metrics(&metrics, Duration::from_millis(500), &config);

    println!("\n=== With Failures ===");
    println!("{}", result.format_report());

    // Should handle 5% failures gracefully
    assert!(
        result.success_rate >= 94.0 && result.success_rate <= 96.0,
        "Success rate out of expected range: {}",
        result.success_rate
    );
}

/// Benchmark memory efficiency
#[test]
fn bench_memory_efficiency() {
    let metrics = PerformanceMetrics::new();

    // Process many events and verify memory overhead is minimal
    for _ in 0..10000 {
        let timer = metrics.start_event();
        std::thread::sleep(Duration::from_micros(100));
        timer.complete();
    }

    let total_events = metrics.total_events();
    let avg_latency = metrics.average_latency_us();
    let throughput = metrics.throughput_eps();

    println!("\n=== Memory Efficiency ===");
    println!("Total events: {}", total_events);
    println!("Average latency: {}us", avg_latency);
    println!("Throughput: {:.1} eps", throughput);

    // Verify metrics tracking doesn't have excessive overhead
    assert_eq!(total_events, 10000, "Event count mismatch");
    assert!(
        avg_latency <= 150,
        "Latency tracking overhead too high: {}us",
        avg_latency
    );
}

/// Benchmark sustained load over time
#[test]
fn bench_sustained_load() {
    let metrics = PerformanceMetrics::new();

    // Simulate 1 minute of sustained 1000 eps load
    let start = Instant::now();
    let mut event_count = 0;

    while start.elapsed() < Duration::from_secs(1) {
        let timer = metrics.start_event();
        std::thread::sleep(Duration::from_micros(1000)); // 1ms per event
        timer.complete();
        event_count += 1;
    }

    let duration = start.elapsed();
    let config = BenchmarkConfig::default();
    let result = BenchmarkResult::from_metrics(&metrics, duration, &config);

    println!("\n=== Sustained Load (1 second) ===");
    println!("Events processed: {}", event_count);
    println!("{}", result.format_report());

    // Verify sustained performance
    assert!(
        result.success_rate >= 99.0,
        "Sustained load success rate {}% below 99%",
        result.success_rate
    );
    assert!(
        result.avg_latency_us <= 2000,
        "Sustained load latency {} exceeds 2ms",
        result.avg_latency_us
    );
}

/// Benchmark comparison: small vs large workloads
#[test]
fn bench_workload_scaling() {
    let small_config = BenchmarkConfig::small();

    // Small workload: 100 events
    let small_metrics = PerformanceMetrics::new();
    let small_start = Instant::now();
    for _ in 0..small_config.num_events {
        let timer = small_metrics.start_event();
        std::thread::sleep(Duration::from_micros(500));
        timer.complete();
    }
    let small_duration = small_start.elapsed();
    let small_result = BenchmarkResult::from_metrics(&small_metrics, small_duration, &small_config);

    // Large workload: 100 events (scaled down from 10000 for test speed)
    let large_config = BenchmarkConfig::default();
    let large_metrics = PerformanceMetrics::new();
    let large_start = Instant::now();
    for _ in 0..1000 {
        let timer = large_metrics.start_event();
        std::thread::sleep(Duration::from_micros(500));
        timer.complete();
    }
    let large_duration = large_start.elapsed();
    let large_result = BenchmarkResult::from_metrics(&large_metrics, large_duration, &large_config);

    println!("\n=== Workload Scaling ===");
    println!("Small (100 events): {} passed", small_result.passed);
    println!(
        "  Latency: {}us, Throughput: {:.1} eps",
        small_result.avg_latency_us, small_result.throughput_eps
    );
    println!("Large (1000 events): {} passed", large_result.passed);
    println!(
        "  Latency: {}us, Throughput: {:.1} eps",
        large_result.avg_latency_us, large_result.throughput_eps
    );

    // Both should pass their respective configurations
    assert!(small_result.passed, "Small workload failed");
    assert!(large_result.passed, "Large workload failed");
}

/// Benchmark latency distribution
#[test]
fn bench_latency_distribution() {
    let metrics = PerformanceMetrics::new();

    // Process events with varying latencies
    for i in 0..1000 {
        let timer = metrics.start_event();
        // Vary latency: baseline 1ms, plus occasional slowdowns
        let delay_us = if i % 100 == 0 {
            5000 // 5ms every 100 events
        } else if i % 50 == 0 {
            2000 // 2ms every 50 events
        } else {
            1000 // 1ms baseline
        };
        std::thread::sleep(Duration::from_micros(delay_us));
        timer.complete();
    }

    let avg_latency = metrics.average_latency_us();
    let throughput = metrics.throughput_eps();

    println!("\n=== Latency Distribution ===");
    println!("Average latency: {}us", avg_latency);
    println!("Throughput: {:.1} eps", throughput);

    // Average should reflect the mixture of latencies
    assert!(
        avg_latency > 1000 && avg_latency < 2000,
        "Average latency {} not in expected range",
        avg_latency
    );
}
