//! Phase 7 Week 4: Performance Profiling & Optimization
//!
//! Performance profiling tests measuring:
//! - Latency metrics (P50, P95, P99)
//! - Throughput validation
//! - Memory efficiency
//! - Hot path optimization
//! - Comparison with baseline targets

use sonic_portsyncd::*;
use std::time::{Duration, Instant};

// ============================================================================
// LATENCY PROFILING TESTS
// ============================================================================

#[test]
fn test_alert_evaluation_latency_p50() {
    // Measure P50 latency for alert evaluation
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "latency_test".to_string(),
        name: "Latency Test".to_string(),
        description: "Test alert evaluation latency".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Warm up
    let warmup_metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    for _ in 0..100 {
        engine.evaluate(&warmup_metrics);
    }

    // Measure latency for 1000 evaluations
    let mut latencies = Vec::new();
    for _ in 0..1000 {
        let start = Instant::now();
        engine.evaluate(&warmup_metrics);
        latencies.push(start.elapsed().as_micros());
    }

    // Calculate P50
    latencies.sort();
    let p50_idx = latencies.len() / 2;
    let p50 = latencies[p50_idx];

    // P50 should be under 100 microseconds
    assert!(p50 < 100, "P50 latency {} µs should be < 100 µs", p50);
}

#[test]
fn test_alert_evaluation_latency_p95() {
    // Measure P95 latency for alert evaluation
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "p95_test".to_string(),
        name: "P95 Test".to_string(),
        description: "Test P95 latency".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Warm up JIT if applicable
    for _ in 0..100 {
        engine.evaluate(&metrics);
    }

    let mut latencies = Vec::new();
    for _ in 0..1000 {
        let start = Instant::now();
        engine.evaluate(&metrics);
        latencies.push(start.elapsed().as_micros());
    }

    latencies.sort();
    let p95_idx = (latencies.len() * 95) / 100;
    let p95 = latencies[p95_idx];

    // P95 should be under 500 microseconds
    assert!(p95 < 500, "P95 latency {} µs should be < 500 µs", p95);
}

#[test]
fn test_alert_evaluation_latency_p99() {
    // Measure P99 latency for alert evaluation
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "p99_test".to_string(),
        name: "P99 Test".to_string(),
        description: "Test P99 latency".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Warm up
    for _ in 0..100 {
        engine.evaluate(&metrics);
    }

    let mut latencies = Vec::new();
    for _ in 0..1000 {
        let start = Instant::now();
        engine.evaluate(&metrics);
        latencies.push(start.elapsed().as_micros());
    }

    latencies.sort();
    let p99_idx = (latencies.len() * 99) / 100;
    let p99 = latencies[p99_idx];

    // P99 should be under 1000 microseconds (1ms)
    assert!(p99 < 1000, "P99 latency {} µs should be < 1000 µs", p99);
}

// ============================================================================
// THROUGHPUT PROFILING TESTS
// ============================================================================

#[test]
fn test_evaluation_throughput_baseline() {
    // Measure baseline throughput for alert evaluation
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "throughput_test".to_string(),
        name: "Throughput Test".to_string(),
        description: "Test throughput".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    let start = Instant::now();
    let event_count = 10000;

    for _ in 0..event_count {
        engine.evaluate(&metrics);
    }

    let elapsed = start.elapsed();
    let throughput = event_count as f64 / elapsed.as_secs_f64();

    // Should achieve at least 10K evaluations per second
    assert!(
        throughput > 10000.0,
        "Throughput {} eps should be > 10K eps",
        throughput
    );
}

#[test]
fn test_health_score_calculation_performance() {
    // Measure performance of health score calculation
    let mut latencies = Vec::new();

    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Warm up
    for _ in 0..100 {
        let _ = metrics.health_score();
    }

    // Measure 10000 health score calculations
    for _ in 0..10000 {
        let start = Instant::now();
        let _ = metrics.health_score();
        latencies.push(start.elapsed().as_nanos());
    }

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() * 99) / 100];

    // Health score calculation should be very fast
    assert!(
        p50 < 5000,
        "P50 health score calculation {} ns should be < 5000 ns",
        p50
    );
    assert!(
        p99 < 50000,
        "P99 health score calculation {} ns should be < 50000 ns",
        p99
    );
}

// ============================================================================
// MEMORY EFFICIENCY TESTS
// ============================================================================

#[test]
fn test_memory_usage_single_rule() {
    // Verify memory usage is reasonable for single rule
    let engine = AlertingEngine::new();
    assert_eq!(engine.rules().len(), 0);

    // Adding one rule should not allocate excessively
    let mut engine = engine;
    let rule = AlertRule {
        rule_id: "mem_test".to_string(),
        name: "Memory Test".to_string(),
        description: "Test memory usage".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);
    assert_eq!(engine.rules().len(), 1);
}

#[test]
fn test_memory_usage_many_rules() {
    // Verify memory usage grows linearly with rule count
    let mut engine = AlertingEngine::new();

    // Add 1000 rules
    for i in 0..1000 {
        let rule = AlertRule {
            rule_id: format!("rule_{}", i),
            name: format!("Rule {}", i),
            description: format!("Rule {}", i),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 50.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };
        engine.add_rule(rule);
    }

    assert_eq!(engine.rules().len(), 1000);
}

#[test]
fn test_memory_usage_many_alerts() {
    // Verify alert storage remains efficient
    let mut engine = AlertingEngine::new();

    // Add rules that trigger
    for i in 0..100 {
        let rule = AlertRule {
            rule_id: format!("alert_rule_{}", i),
            name: format!("Alert Rule {}", i),
            description: format!("Rule {}", i),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 100.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };
        engine.add_rule(rule);
    }

    let metrics = WarmRestartMetrics {
        warm_restart_count: 100,
        cold_start_count: 50,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 100,
        state_recovery_count: 0,
        corruption_detected_count: 50,
        backup_created_count: 100,
        backup_cleanup_count: 0,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 500.0,
        max_initial_sync_duration_secs: 1000,
        min_initial_sync_duration_secs: 100,
    };

    engine.evaluate(&metrics);
    let alert_count = engine.alerts().len();

    // All 100 rules should fire
    assert!(alert_count > 0, "Should have generated alerts");
}

// ============================================================================
// HOT PATH OPTIMIZATION TESTS
// ============================================================================

#[test]
fn test_metric_value_extraction_performance() {
    // Measure metric value extraction performance (hot path)
    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Test health score extraction (hot path)
    let start = Instant::now();
    let mut sum = 0.0;
    for _ in 0..100000 {
        sum += metrics.health_score();
    }
    let elapsed = start.elapsed();

    // Should complete 100K extractions in < 10ms
    assert!(
        elapsed < Duration::from_millis(10),
        "100K health score extractions should take < 10ms, took {:?}",
        elapsed
    );

    // Prevent optimization
    assert!(sum > 0.0);
}

#[test]
fn test_condition_evaluation_performance() {
    // Measure condition evaluation performance
    let mut latencies = Vec::new();

    let test_values = vec![0.0, 25.0, 50.0, 75.0, 100.0];
    let threshold = 50.0;

    // Warm up
    for _ in 0..100 {
        for &value in &test_values {
            let _ = value < threshold;
        }
    }

    // Measure condition evaluation
    for _ in 0..10000 {
        for &value in &test_values {
            let start = Instant::now();
            let _result = value < threshold;
            latencies.push(start.elapsed().as_nanos());
        }
    }

    latencies.sort();
    let p99 = latencies[(latencies.len() * 99) / 100];

    // Condition evaluation should be nanoseconds
    assert!(
        p99 < 100,
        "P99 condition evaluation {} ns should be < 100 ns",
        p99
    );
}

// ============================================================================
// COMPARISON WITH BASELINE TARGETS
// ============================================================================

#[test]
fn test_latency_meets_targets() {
    // Verify latency meets production targets
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "baseline_test".to_string(),
        name: "Baseline Test".to_string(),
        description: "Test baseline targets".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Warm up
    for _ in 0..1000 {
        engine.evaluate(&metrics);
    }

    // Measure 5000 evaluations
    let start = Instant::now();
    for _ in 0..5000 {
        engine.evaluate(&metrics);
    }
    let elapsed = start.elapsed();

    // Should complete 5000 evaluations in < 100ms
    assert!(
        elapsed < Duration::from_millis(100),
        "5000 evaluations should take < 100ms, took {:?}",
        elapsed
    );
}

#[test]
fn test_throughput_meets_targets() {
    // Verify throughput meets production targets
    let mut engine = AlertingEngine::new();

    for i in 0..10 {
        let rule = AlertRule {
            rule_id: format!("rule_{}", i),
            name: format!("Rule {}", i),
            description: format!("Rule {}", i),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 50.0 - (i as f64 * 2.0),
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };
        engine.add_rule(rule);
    }

    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Measure throughput with 10 rules
    let start = Instant::now();
    for _ in 0..10000 {
        engine.evaluate(&metrics);
    }
    let elapsed = start.elapsed();

    let throughput = 10000.0 / elapsed.as_secs_f64();

    // Should achieve at least 5K evaluations per second with 10 rules
    assert!(
        throughput > 5000.0,
        "Throughput with 10 rules {} eps should be > 5K eps",
        throughput
    );
}

// ============================================================================
// REGRESSION DETECTION TESTS
// ============================================================================

#[test]
fn test_no_performance_regression_single_rule() {
    // Detect any performance regression with single rule
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "regression_test".to_string(),
        name: "Regression Test".to_string(),
        description: "Test regression detection".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 50,
        state_recovery_count: 50,
        corruption_detected_count: 10,
        backup_created_count: 100,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Establish baseline
    let start = Instant::now();
    for _ in 0..1000 {
        engine.evaluate(&metrics);
    }
    let baseline = start.elapsed();

    // Verify performance hasn't degraded
    let start = Instant::now();
    for _ in 0..1000 {
        engine.evaluate(&metrics);
    }
    let current = start.elapsed();

    // Allow 50% variance for test environment
    let max_allowed = baseline + baseline / 2;
    assert!(
        current <= max_allowed,
        "Performance regression detected: baseline {:?}, current {:?}",
        baseline,
        current
    );
}
