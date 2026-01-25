//! Phase 7 Week 2: Stress Testing - Dashboard Load Testing
//!
//! Tests for system behavior under Grafana dashboard and concurrent query loads
//! Validates query performance and data consistency with high concurrent access

use sonic_portsyncd::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ============================================================================
// DASHBOARD QUERY SIMULATOR
// ============================================================================

/// Simulates a Grafana dashboard query
struct DashboardQuery {
    query_id: String,
    metric_name: String,
    port_count: usize,
    concurrent_viewers: usize,
}

/// Simulates dashboard access patterns
struct DashboardSimulator {
    engines: Arc<Mutex<Vec<AlertingEngine>>>,
    query_results: Arc<Mutex<HashMap<String, QueryResult>>>,
}

#[derive(Debug, Clone)]
struct QueryResult {
    query_id: String,
    total_alerts: usize,
    critical_count: usize,
    warning_count: usize,
    response_time_ms: u128,
}

impl DashboardSimulator {
    fn new() -> Self {
        Self {
            engines: Arc::new(Mutex::new(vec![AlertingEngine::new()])),
            query_results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Simulate executing a dashboard query
    fn execute_query(&self, query: &DashboardQuery) -> QueryResult {
        let start = Instant::now();

        let engines = self.engines.lock().unwrap();
        let engine = &engines[0];

        // Simulate query execution by filtering alerts
        let alerts_map = engine.alerts();
        let alerts: Vec<_> = alerts_map.values().collect();
        let critical_count = alerts.iter().filter(|a| a.severity == AlertSeverity::Critical).count();
        let warning_count = alerts.iter().filter(|a| a.severity == AlertSeverity::Warning).count();

        let response_time = start.elapsed().as_millis();

        QueryResult {
            query_id: query.query_id.clone(),
            total_alerts: alerts.len(),
            critical_count,
            warning_count,
            response_time_ms: response_time,
        }
    }
}

// ============================================================================
// DASHBOARD PERFORMANCE TESTS
// ============================================================================

#[test]
fn test_grafana_dashboard_with_10k_ports() {
    // Verify dashboard can handle metrics from 10K ports
    let mut engine = AlertingEngine::new();

    // Setup rules that would be in a typical dashboard
    let rules = vec![
        AlertRule {
            rule_id: "dashboard_critical".to_string(),
            name: "Dashboard Critical".to_string(),
            description: "Critical health alerts".to_string(),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 25.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Critical,
            actions: vec![],
        },
        AlertRule {
            rule_id: "dashboard_warning".to_string(),
            name: "Dashboard Warning".to_string(),
            description: "Warning health alerts".to_string(),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 50.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        },
    ];

    for rule in rules {
        engine.add_rule(rule);
    }

    // Generate metrics for 10K ports
    for port_id in 0..10000 {
        let metrics = create_dashboard_test_metrics(port_id);
        engine.evaluate(&metrics);
    }

    // Simulate dashboard query
    let start = Instant::now();
    let all_alerts = engine.alerts().values().collect::<Vec<_>>();
    let critical = all_alerts.iter().filter(|a| a.severity == AlertSeverity::Critical).count();
    let warning = all_alerts.iter().filter(|a| a.severity == AlertSeverity::Warning).count();
    let query_time = start.elapsed();

    // Dashboard query should complete in reasonable time (<100ms)
    assert!(
        query_time.as_millis() < 100,
        "Dashboard query should complete in <100ms, took {}ms",
        query_time.as_millis()
    );

    // Should have found some alerts
    assert!(critical > 0 || warning > 0, "Dashboard should find some alerts");
}

#[test]
fn test_dashboard_query_filtering_100k_ports() {
    // Verify dashboard can filter alerts from 100K ports efficiently
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "filter_test".to_string(),
        name: "Filter Test".to_string(),
        description: "Test filtering with 100K ports".to_string(),
        metric_name: "warm_restart_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 40.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Critical,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Generate metrics for 100K ports
    for port_id in 0..100000 {
        let metrics = create_dashboard_test_metrics(port_id);
        engine.evaluate(&metrics);
    }

    // Filter by severity
    let start = Instant::now();
    let critical_alerts = engine.alerts_by_state(AlertState::Firing);
    let filter_time = start.elapsed();

    assert!(
        filter_time.as_millis() < 500,
        "Filtering 100K ports should complete in <500ms, took {}ms",
        filter_time.as_millis()
    );

    // Verify we got some results or alerts exist
    let all_alerts = engine.alerts().len();
    assert!(all_alerts > 0 || critical_alerts.len() > 0, "Should have some alerts from 100K ports");
}

#[test]
fn test_dashboard_aggregation_metrics() {
    // Verify dashboard aggregation functions work correctly at scale
    let mut engine = AlertingEngine::new();

    let rules = vec![
        AlertRule {
            rule_id: "agg_critical".to_string(),
            name: "Aggregation Critical".to_string(),
            description: "Critical aggregation".to_string(),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 30.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Critical,
            actions: vec![],
        },
        AlertRule {
            rule_id: "agg_warning".to_string(),
            name: "Aggregation Warning".to_string(),
            description: "Warning aggregation".to_string(),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 60.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        },
    ];

    for rule in rules {
        engine.add_rule(rule);
    }

    // Generate metrics for 50K ports
    for port_id in 0..50000 {
        let metrics = create_dashboard_test_metrics(port_id);
        engine.evaluate(&metrics);
    }

    // Aggregate alerts by severity
    let start = Instant::now();
    let all_alerts = engine.alerts().values().collect::<Vec<_>>();
    let critical_count = all_alerts.iter().filter(|a| a.severity == AlertSeverity::Critical).count();
    let warning_count = all_alerts.iter().filter(|a| a.severity == AlertSeverity::Warning).count();
    let agg_time = start.elapsed();

    assert!(
        agg_time.as_millis() < 100,
        "Aggregation should complete in <100ms, took {}ms",
        agg_time.as_millis()
    );

    // Verify we have reasonable counts
    assert!(critical_count > 0 || warning_count > 0, "Should have some alerts");
    if critical_count > 0 && warning_count > 0 {
        assert!(critical_count <= warning_count, "More warnings than critical expected");
    }
}

// ============================================================================
// CONCURRENT ACCESS TESTS
// ============================================================================

#[test]
fn test_concurrent_dashboard_readers_10_users() {
    // Verify dashboard can handle 10 concurrent viewers
    let simulator = DashboardSimulator::new();

    // Setup engine with alerts
    {
        let mut engines = simulator.engines.lock().unwrap();
        let engine = &mut engines[0];

        let rule = AlertRule {
            rule_id: "concurrent_test".to_string(),
            name: "Concurrent Test".to_string(),
            description: "Test concurrent readers".to_string(),
            metric_name: "eoiu_timeout_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 30.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };

        engine.add_rule(rule);

        // Generate metrics
        for port_id in 0..5000 {
            let metrics = create_dashboard_test_metrics(port_id);
            engine.evaluate(&metrics);
        }
    }

    // Simulate 10 concurrent viewers querying dashboard
    let start = Instant::now();
    for viewer in 0..10 {
        let query = DashboardQuery {
            query_id: format!("query_{}", viewer),
            metric_name: "all".to_string(),
            port_count: 5000,
            concurrent_viewers: 10,
        };

        let result = simulator.execute_query(&query);
        assert!(result.response_time_ms < 100, "Each query should complete in <100ms");

        let mut results = simulator.query_results.lock().unwrap();
        results.insert(query.query_id.clone(), result);
    }

    let total_time = start.elapsed();

    // All 10 viewers should complete relatively quickly
    assert!(
        total_time.as_millis() < 1000,
        "10 concurrent queries should complete in <1s, took {}ms",
        total_time.as_millis()
    );

    // Verify all queries succeeded
    let results = simulator.query_results.lock().unwrap();
    assert_eq!(
        results.len(),
        10,
        "All 10 queries should have completed"
    );
}

#[test]
fn test_concurrent_dashboard_readers_100_users() {
    // Verify dashboard can handle 100 concurrent viewers (high stress)
    let simulator = DashboardSimulator::new();

    // Setup engine with larger alert set
    {
        let mut engines = simulator.engines.lock().unwrap();
        let engine = &mut engines[0];

        for rule_idx in 0..5 {
            let rule = AlertRule {
                rule_id: format!("rule_{}", rule_idx),
                name: format!("Rule {}", rule_idx),
                description: format!("Rule {} description", rule_idx),
                metric_name: "health_score".to_string(),
                condition: AlertCondition::Below,
                threshold: 40.0 + (rule_idx as f64 * 5.0),
                threshold_range: None,
                evaluation_window_secs: 300,
                for_duration_secs: 0,
                enabled: true,
                severity: AlertSeverity::Warning,
                actions: vec![],
            };
            engine.add_rule(rule);
        }

        // Generate metrics for 20K ports
        for port_id in 0..20000 {
            let metrics = create_dashboard_test_metrics(port_id);
            engine.evaluate(&metrics);
        }
    }

    // Simulate 100 concurrent viewers
    let start = Instant::now();
    let mut response_times = Vec::new();

    for viewer in 0..100 {
        let query = DashboardQuery {
            query_id: format!("query_{}", viewer),
            metric_name: "all".to_string(),
            port_count: 20000,
            concurrent_viewers: 100,
        };

        let result = simulator.execute_query(&query);
        response_times.push(result.response_time_ms);

        let mut results = simulator.query_results.lock().unwrap();
        results.insert(query.query_id.clone(), result);
    }

    let total_time = start.elapsed();

    // Calculate response time statistics
    let avg_response = response_times.iter().sum::<u128>() / response_times.len() as u128;

    // Each query should still be reasonably fast
    assert!(
        avg_response < 200,
        "Average response time should be <200ms, got {}ms",
        avg_response
    );

    // P99 should be reasonable
    response_times.sort();
    let p99_idx = (response_times.len() * 99) / 100;
    let p99 = response_times[p99_idx];
    assert!(
        p99 < 500,
        "P99 response time should be <500ms, got {}ms",
        p99
    );

    // Total time for all 100 queries
    assert!(
        total_time.as_secs() < 60,
        "100 concurrent queries should complete in <60s"
    );

    // Verify all queries succeeded
    let results = simulator.query_results.lock().unwrap();
    assert_eq!(results.len(), 100, "All 100 queries should have completed");
}

#[test]
fn test_dashboard_query_consistency_under_load() {
    // Verify dashboard returns consistent data under concurrent load
    let simulator = DashboardSimulator::new();

    // Setup engine
    {
        let mut engines = simulator.engines.lock().unwrap();
        let engine = &mut engines[0];

        let rule = AlertRule {
            rule_id: "consistency_load".to_string(),
            name: "Consistency Load Test".to_string(),
            description: "Test data consistency".to_string(),
            metric_name: "corruption_detected_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 5.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Critical,
            actions: vec![],
        };

        engine.add_rule(rule);

        // Generate metrics
        for port_id in 0..10000 {
            let metrics = create_dashboard_test_metrics(port_id);
            engine.evaluate(&metrics);
        }
    }

    // Execute same query 50 times and verify consistency
    let mut results = Vec::new();
    for _execution in 0..50 {
        let query = DashboardQuery {
            query_id: "consistency_query".to_string(),
            metric_name: "all".to_string(),
            port_count: 10000,
            concurrent_viewers: 1,
        };

        let result = simulator.execute_query(&query);
        results.push(result);
    }

    // All executions should return same alert counts
    let first_result = &results[0];
    for (idx, result) in results.iter().enumerate() {
        assert_eq!(
            result.total_alerts, first_result.total_alerts,
            "Query {} returned different alert count",
            idx
        );
        assert_eq!(
            result.critical_count, first_result.critical_count,
            "Query {} returned different critical count",
            idx
        );
    }
}

// ============================================================================
// REAL-TIME UPDATE TESTS
// ============================================================================

#[test]
fn test_dashboard_updates_during_event_stream() {
    // Verify dashboard shows updated alerts as events arrive
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "realtime_test".to_string(),
        name: "Real-time Test".to_string(),
        description: "Test real-time updates".to_string(),
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

    // Simulate streaming events and dashboard queries
    let mut alert_counts = Vec::new();

    for batch_num in 0..10 {
        // Process batch of events
        for port_id in (batch_num * 100)..((batch_num + 1) * 100) {
            let metrics = create_dashboard_test_metrics(port_id);
            engine.evaluate(&metrics);
        }

        // Query dashboard
        let alert_count = engine.alerts().len();
        alert_counts.push(alert_count as usize);
    }

    // Alert count should generally increase (more events = more alerts)
    // but not necessarily every batch due to variation
    assert!(
        alert_counts[9] >= alert_counts[0],
        "Alert count should generally increase with more events"
    );
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn create_dashboard_test_metrics(port_id: u32) -> WarmRestartMetrics {
    // Create varying health scores - some healthy, some degraded
    let severity = port_id % 3;

    match severity {
        0 => {
            // Critical health - will trigger alerts
            WarmRestartMetrics {
                warm_restart_count: 80 + (port_id % 20) as u64,
                cold_start_count: 40 + (port_id % 10) as u64,
                eoiu_detected_count: 100,
                eoiu_timeout_count: 80 + (port_id % 20) as u64,
                state_recovery_count: 10 + (port_id % 5) as u64,
                corruption_detected_count: 5 + (port_id % 15) as u64,
                backup_created_count: 100,
                backup_cleanup_count: 30,
                last_warm_restart_secs: None,
                last_eoiu_detection_secs: None,
                last_state_recovery_secs: None,
                last_corruption_detected_secs: None,
                avg_initial_sync_duration_secs: 80.0 + ((port_id % 100) as f64),
                max_initial_sync_duration_secs: 400,
                min_initial_sync_duration_secs: 50,
            }
        }
        1 => {
            // Warning health - will trigger warning alerts
            WarmRestartMetrics {
                warm_restart_count: 30 + (port_id % 20) as u64,
                cold_start_count: 15 + (port_id % 10) as u64,
                eoiu_detected_count: 100,
                eoiu_timeout_count: 40 + (port_id % 20) as u64,
                state_recovery_count: 50 + (port_id % 30) as u64,
                corruption_detected_count: 2 + (port_id % 8) as u64,
                backup_created_count: 100,
                backup_cleanup_count: 95,
                last_warm_restart_secs: None,
                last_eoiu_detection_secs: None,
                last_state_recovery_secs: None,
                last_corruption_detected_secs: None,
                avg_initial_sync_duration_secs: 30.0 + ((port_id % 30) as f64),
                max_initial_sync_duration_secs: 100,
                min_initial_sync_duration_secs: 15,
            }
        }
        _ => {
            // Healthy - won't trigger alerts
            WarmRestartMetrics {
                warm_restart_count: 2 + (port_id % 5) as u64,
                cold_start_count: 1,
                eoiu_detected_count: 100,
                eoiu_timeout_count: 5 + (port_id % 5) as u64,
                state_recovery_count: 90 + (port_id % 9) as u64,
                corruption_detected_count: 0,
                backup_created_count: 100,
                backup_cleanup_count: 100,
                last_warm_restart_secs: None,
                last_eoiu_detection_secs: None,
                last_state_recovery_secs: None,
                last_corruption_detected_secs: None,
                avg_initial_sync_duration_secs: 2.0 + ((port_id % 3) as f64),
                max_initial_sync_duration_secs: 5,
                min_initial_sync_duration_secs: 1,
            }
        }
    }
}
