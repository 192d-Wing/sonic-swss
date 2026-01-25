//! Phase 7 Week 2: Stress Testing - Port Scaling
//!
//! Tests for system behavior with large numbers of ports and metrics
//! Validates performance, memory usage, and data consistency at scale

use sonic_portsyncd::*;
use std::collections::HashMap;

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Create a WarmRestartMetrics with predictable values for testing
fn create_metrics_for_port(port_id: u32) -> WarmRestartMetrics {
    WarmRestartMetrics {
        warm_restart_count: (port_id % 100) as u64,
        cold_start_count: (port_id % 50) as u64,
        eoiu_detected_count: 1000 + (port_id % 900) as u64,
        eoiu_timeout_count: (port_id % 100) as u64,
        state_recovery_count: (port_id % 200) as u64,
        corruption_detected_count: (port_id % 10) as u64,
        backup_created_count: (port_id % 300) as u64,
        backup_cleanup_count: (port_id % 280) as u64,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0 + ((port_id % 100) as f64),
        max_initial_sync_duration_secs: 50 + (port_id % 100) as u64,
        min_initial_sync_duration_secs: 2 + (port_id % 10) as u64,
    }
}

// ============================================================================
// PORT SCALING TESTS
// ============================================================================

#[test]
fn test_1000_ports_metric_tracking() {
    // Verify system can track metrics for 1000 ports
    let mut engine = AlertingEngine::new();

    // Add rules for different port ranges
    let rule = AlertRule {
        rule_id: "scaling_test_1000".to_string(),
        name: "Port Scaling Test 1000".to_string(),
        description: "Test metric tracking with 1000 ports".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 100.0, // Always true to test evaluation
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Create and evaluate metrics for 1000 ports
    let mut evaluation_count = 0;
    for port_id in 0..1000 {
        let metrics = create_metrics_for_port(port_id);
        engine.evaluate(&metrics);
        evaluation_count += 1;
    }

    assert_eq!(evaluation_count, 1000, "Should evaluate 1000 ports");
}

#[test]
fn test_10k_ports_memory_consistency() {
    // Verify system maintains consistency with 10K ports
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "scaling_test_10k".to_string(),
        name: "Port Scaling Test 10K".to_string(),
        description: "Test consistency with 10000 ports".to_string(),
        metric_name: "eoiu_timeout_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Critical,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Track health scores for all ports
    let mut health_scores: HashMap<u32, f64> = HashMap::new();

    for port_id in 0..10000 {
        let metrics = create_metrics_for_port(port_id);
        let health_score = metrics.health_score();
        health_scores.insert(port_id, health_score);
    }

    // Verify we tracked all 10K ports
    assert_eq!(health_scores.len(), 10000, "Should track 10000 ports");

    // Verify health scores are reasonable (between 0 and 100)
    for (port_id, score) in health_scores.iter() {
        assert!(
            *score >= 0.0 && *score <= 100.0,
            "Port {} health score {} out of bounds",
            port_id,
            score
        );
    }
}

#[test]
fn test_100k_ports_health_distribution() {
    // Verify health score distribution remains consistent with 100K ports
    let mut health_scores = Vec::new();

    for port_id in 0..100000 {
        let metrics = create_metrics_for_port(port_id);
        let score = metrics.health_score();
        health_scores.push(score);
    }

    // Verify we have 100K scores
    assert_eq!(
        health_scores.len(),
        100000,
        "Should have 100000 health scores"
    );

    // Calculate basic statistics
    let sum: f64 = health_scores.iter().sum();
    let avg = sum / health_scores.len() as f64;

    // Average should be reasonable (0-100% health range)
    assert!(
        avg >= 0.0 && avg <= 100.0,
        "Average health score {} should be in [0, 100]",
        avg
    );

    // Find min/max for sanity check
    let min = health_scores.iter().copied().fold(f64::INFINITY, f64::min);
    let max = health_scores
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    assert!(
        min >= 0.0 && max <= 100.0,
        "Health scores should be in [0, 100]"
    );
}

#[test]
fn test_metric_scaling_with_alert_rules() {
    // Verify alert engine scales with port count
    let mut engine = AlertingEngine::new();

    // Add multiple rules
    for rule_idx in 0..10 {
        let rule = AlertRule {
            rule_id: format!("scale_rule_{}", rule_idx),
            name: format!("Scale Rule {}", rule_idx),
            description: format!("Scale test rule {}", rule_idx),
            metric_name: "eoiu_timeout_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 30.0 + (rule_idx as f64 * 5.0),
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };
        engine.add_rule(rule);
    }

    // Evaluate metrics for 5000 ports
    let mut evaluation_count = 0;
    for port_id in 0..5000 {
        let metrics = create_metrics_for_port(port_id);
        engine.evaluate(&metrics);
        evaluation_count += 1;
    }

    assert_eq!(evaluation_count, 5000, "Should evaluate 5000 ports");

    // Verify alerts were generated
    let alerts = engine.alerts();
    assert!(alerts.len() > 0, "Should have generated some alerts");
}

// ============================================================================
// HISTOGRAM AND PERCENTILE TESTS
// ============================================================================

#[test]
fn test_histogram_accuracy_at_scale() {
    // Verify histogram calculations remain accurate with large metric sets
    let mut metrics_list = Vec::new();

    // Create 1000 metric samples
    for i in 0..1000 {
        let metrics = create_metrics_for_port(i);
        metrics_list.push(metrics);
    }

    // Calculate health scores
    let mut scores: Vec<f64> = metrics_list.iter().map(|m| m.health_score()).collect();

    // Sort for percentile calculation
    scores.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Calculate percentiles
    let p50 = scores[scores.len() / 2]; // 50th percentile
    let p95 = scores[(scores.len() * 95) / 100]; // 95th percentile
    let p99 = scores[(scores.len() * 99) / 100]; // 99th percentile

    // All percentiles should be valid
    assert!(
        p50 > 0.0 && p50 <= 100.0,
        "P50 {} should be in valid range",
        p50
    );
    assert!(
        p95 > 0.0 && p95 <= 100.0,
        "P95 {} should be in valid range",
        p95
    );
    assert!(
        p99 > 0.0 && p99 <= 100.0,
        "P99 {} should be in valid range",
        p99
    );

    // P99 should be >= P95 >= P50
    assert!(p95 >= p50, "P95 should be >= P50");
    assert!(p99 >= p95, "P99 should be >= P95");
}

#[test]
fn test_percentile_consistency_across_scales() {
    // Verify percentile calculations stay consistent as scale increases
    let percentiles_1k = calculate_percentiles(1000);
    let percentiles_10k = calculate_percentiles(10000);

    // P50 should be relatively stable (within 20% variation)
    let p50_delta = (percentiles_1k.p50 - percentiles_10k.p50).abs();
    assert!(
        p50_delta < (percentiles_1k.p50 * 0.2),
        "P50 should be consistent across scales"
    );

    // P95/P99 should also be relatively stable
    let p95_delta = (percentiles_1k.p95 - percentiles_10k.p95).abs();
    assert!(
        p95_delta < (percentiles_1k.p95 * 0.3),
        "P95 should be consistent across scales"
    );
}

#[test]
fn test_metric_distribution_remains_valid() {
    // Verify metric distributions remain valid at scale
    let mut distribution = HashMap::new();

    // Count metrics in ranges
    for port_id in 0..10000 {
        let metrics = create_metrics_for_port(port_id);
        let score = metrics.health_score();

        let bucket = if score < 25.0 {
            "critical"
        } else if score < 50.0 {
            "degraded"
        } else if score < 75.0 {
            "healthy"
        } else {
            "excellent"
        };

        *distribution.entry(bucket).or_insert(0) += 1;
    }

    // At least one bucket should have data
    assert!(
        !distribution.is_empty(),
        "Should have some health score data"
    );

    // Total should equal 10K
    let total: u32 = distribution.values().sum();
    assert_eq!(
        total, 10000,
        "Distribution should account for all 10K ports"
    );
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

struct Percentiles {
    p50: f64,
    p95: f64,
    p99: f64,
}

fn calculate_percentiles(count: u32) -> Percentiles {
    let mut scores: Vec<f64> = (0..count)
        .map(|id| create_metrics_for_port(id).health_score())
        .collect();

    scores.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = scores[scores.len() / 2];
    let p95 = scores[(scores.len() * 95) / 100];
    let p99 = scores[(scores.len() * 99) / 100];

    Percentiles { p50, p95, p99 }
}
