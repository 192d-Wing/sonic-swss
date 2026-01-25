//! Phase 6 Week 5 Integration Tests
//!
//! Comprehensive integration tests for alerting, trend analysis, and metrics monitoring.
//! Tests the complete workflow of:
//! 1. Metrics collection and observation
//! 2. Trend analysis and anomaly detection
//! 3. Alert rule evaluation and state management
//! 4. PromQL query generation and execution
//! 5. Health scoring and predictive analysis
//!
//! Phase 6 Week 5 Implementation Tests

use sonic_portsyncd::*;

// ============================================================================
// ALERTING ENGINE INTEGRATION TESTS
// ============================================================================

#[test]
fn test_alerting_engine_complete_lifecycle() {
    let mut engine = AlertingEngine::new();

    // Load default alert rules
    let default_rules = create_default_alert_rules();
    assert_eq!(default_rules.len(), 10);

    for rule in default_rules {
        engine.add_rule(rule);
    }

    assert_eq!(engine.rules().len(), 10);

    // Create metrics that should trigger some alerts
    let metrics = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25, // 50% cold start rate - should trigger anomaly alert
        eoiu_detected_count: 100,
        eoiu_timeout_count: 60, // 60% timeout rate - should trigger high timeout alert
        state_recovery_count: 20, // Low recovery - may trigger
        corruption_detected_count: 10,
        backup_created_count: 50,
        backup_cleanup_count: 30,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Create a simple override rule with no duration requirement for testing
    let test_rule = AlertRule {
        rule_id: "eoiu_timeout_high".to_string(), // Override the default rule
        name: "EOIU Timeout High".to_string(),
        description: "Alert when EOIU timeout rate is high".to_string(),
        metric_name: "eoiu_timeout_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0, // No duration - fires immediately
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![AlertAction::Log],
    };

    // Replace the default rule with our zero-duration version
    engine.add_rule(test_rule);

    // Single evaluation creates Firing alert (no duration requirement)
    let alerts = engine.evaluate(&metrics);

    // Should have some alerts in the engine
    assert!(!alerts.is_empty(), "Should have triggered some alerts");

    // Check for firing alerts
    let firing = engine.alerts_by_state(AlertState::Firing);
    assert!(
        !firing.is_empty(),
        "Should have firing alerts before suppression"
    );

    // Suppress a firing alert
    let high_timeout_rule = engine.suppress_alert("eoiu_timeout_high");
    assert!(high_timeout_rule, "Should suppress high timeout alert");

    // Verify suppressed alert
    let suppressed = engine.alerts_by_state(AlertState::Suppressed);
    assert!(!suppressed.is_empty(), "Should have suppressed alerts");

    // Condition is still met, so keep metrics the same and re-evaluate
    // The suppressed alert should stay suppressed even while condition is met
    engine.evaluate(&metrics);

    // Verify alert is still suppressed
    let still_suppressed = engine.alerts_by_state(AlertState::Suppressed);
    assert!(
        !still_suppressed.is_empty(),
        "Alert should remain suppressed"
    );

    // Unsuppress alert while suppressed
    let restored = engine.unsuppress_alert("eoiu_timeout_high");
    assert!(restored, "Should unsuppress alert");

    // After unsuppressing, should go back to Firing
    let firing_again = engine.alerts_by_state(AlertState::Firing);
    assert!(
        !firing_again.is_empty(),
        "Alert should be firing again after unsuppressing"
    );
}

#[test]
fn test_alerting_with_custom_rules() {
    let mut engine = AlertingEngine::new();

    // Create custom rule: trigger if health score drops below 50
    let health_rule = AlertRule {
        rule_id: "custom_health_check".to_string(),
        name: "Custom Health Check".to_string(),
        description: "Alert when health drops below 50".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0, // No duration - fires immediately for testing
        enabled: true,
        severity: AlertSeverity::Critical,
        actions: vec![AlertAction::Log, AlertAction::Notify],
    };

    engine.add_rule(health_rule);

    // Create degraded metrics (low health)
    let metrics = WarmRestartMetrics {
        warm_restart_count: 10,
        cold_start_count: 8, // High cold start rate
        eoiu_detected_count: 100,
        eoiu_timeout_count: 70,  // Very high timeout rate
        state_recovery_count: 5, // Low recovery
        corruption_detected_count: 15,
        backup_created_count: 10,
        backup_cleanup_count: 5,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 100.0,
        max_initial_sync_duration_secs: 300,
        min_initial_sync_duration_secs: 50,
    };

    let health_score = metrics.health_score();
    assert!(
        health_score < 50.0,
        "Health should be low: {}",
        health_score
    );

    // Single evaluation should create Firing alert (for_duration_secs: 0)
    engine.evaluate(&metrics);

    let firing = engine.alerts_by_state(AlertState::Firing);
    assert!(
        !firing.is_empty(),
        "Should have critical alert firing, found {} firing alerts",
        firing.len()
    );
}

// ============================================================================
// TREND ANALYSIS INTEGRATION TESTS
// ============================================================================

#[test]
fn test_trend_analysis_complete_workflow() {
    let mut history = HistoricalMetrics::new(100);

    // Add metrics over time showing degradation
    let timestamps: Vec<u64> = (1000..1500).step_by(100).collect();
    let values = [10.0, 12.0, 15.0, 20.0, 30.0];

    for (_ts, &val) in timestamps.iter().zip(values.iter()) {
        history.add_observation("corruption_rate".to_string(), val);
    }

    let observations = history.get_observations("corruption_rate");
    assert_eq!(observations.len(), 5);

    // Analyze trend
    let trend = TrendAnalyzer::detect_trend(&observations).unwrap();
    assert_eq!(trend.direction, TrendDirection::Increasing);
    assert!(
        trend.slope > 0.0,
        "Slope should be positive for increasing trend"
    );
    assert!(trend.confidence > 0.5, "Should have high confidence");

    // Detect anomalies
    history.add_observation("corruption_rate".to_string(), 200.0); // Extreme value
    let updated_obs = history.get_observations("corruption_rate");
    let anomalies = TrendAnalyzer::detect_anomalies(&updated_obs);
    assert!(!anomalies.is_empty(), "Should detect extreme value anomaly");
}

#[test]
fn test_predictive_scorer_with_trends() {
    let metrics = WarmRestartMetrics {
        warm_restart_count: 100,
        cold_start_count: 10,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 20,
        state_recovery_count: 95,
        corruption_detected_count: 5,
        backup_created_count: 100,
        backup_cleanup_count: 50,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Create a degrading trend
    let degrading_trend = TrendAnalysis {
        metric_name: "health_score".to_string(),
        direction: TrendDirection::Decreasing,
        slope: -0.5,
        confidence: 0.9,
        duration_secs: 3600,
        start_value: 90.0,
        end_value: 80.0,
    };

    // Predict health
    let predicted =
        PredictiveScorer::predict_health_score(&metrics, std::slice::from_ref(&degrading_trend));
    let current = metrics.health_score();
    assert!(predicted <= current, "Predicted should be lower or equal");

    // Estimate time to degrade
    let time_to_degrade =
        PredictiveScorer::estimate_time_to_degrade(&metrics, &degrading_trend, 50.0);
    assert!(time_to_degrade.is_some());
    assert!(time_to_degrade.unwrap() > 0);
}

// ============================================================================
// PROMQL QUERY INTEGRATION TESTS
// ============================================================================

#[test]
fn test_promql_query_categories_coverage() {
    let categories = vec![
        QueryCategory::RecoveryRates,
        QueryCategory::SyncDuration,
        QueryCategory::ErrorRates,
        QueryCategory::HealthMetrics,
        QueryCategory::TrendAnalysis,
        QueryCategory::Throughput,
        QueryCategory::Latency,
        QueryCategory::Reliability,
    ];

    let all_queries = PromQLBuilder::all_queries();
    assert!(all_queries.len() >= 23, "Should have at least 23 queries");

    for category in categories {
        let category_queries = PromQLBuilder::queries_for_category(category);
        assert!(
            !category_queries.is_empty(),
            "Category {:?} should have queries",
            category
        );
    }
}

#[test]
fn test_promql_recovery_rate_queries() {
    let query1 = PromQLBuilder::recovery_success_rate();
    assert!(query1.query.contains("portsyncd_state_recoveries"));
    assert!(query1.query.contains("portsyncd_corruptions_detected"));

    let query2 = PromQLBuilder::unrecovered_corruption_ratio();
    assert!(query2.query.contains("portsyncd_corruptions_detected"));

    let query3 = PromQLBuilder::corruption_rate(TimeWindow::FiveMinutes);
    assert!(query3.query.contains("rate"));
    assert!(query3.query.contains("5m"));
}

#[test]
fn test_promql_sync_duration_queries() {
    let avg = PromQLBuilder::avg_sync_duration();
    assert_eq!(avg.category, QueryCategory::SyncDuration);

    let max = PromQLBuilder::max_sync_duration();
    assert_eq!(max.category, QueryCategory::SyncDuration);

    let trend = PromQLBuilder::sync_duration_trend(TimeWindow::OneHour);
    assert!(trend.query.contains("1h"));
}

#[test]
fn test_promql_health_metric_queries() {
    let health = PromQLBuilder::health_score();
    assert_eq!(health.category, QueryCategory::HealthMetrics);
    assert!(!health.query.is_empty());

    let reliability = PromQLBuilder::reliability_score();
    assert_eq!(reliability.category, QueryCategory::HealthMetrics);

    let warm_restart = PromQLBuilder::warm_restart_success_rate();
    assert!(warm_restart.query.contains("portsyncd_warm_restarts"));
}

#[test]
fn test_promql_latency_percentile_queries() {
    let p50 = PromQLBuilder::p50_sync_duration();
    assert_eq!(p50.category, QueryCategory::Latency);

    let p95 = PromQLBuilder::sync_duration_percentile(95);
    assert!(p95.query.contains("histogram_quantile"));
    assert!(p95.query.contains("0.95"));

    let p99 = PromQLBuilder::sync_duration_percentile(99);
    assert!(p99.query.contains("histogram_quantile"));
    assert!(p99.query.contains("0.99"));
}

// ============================================================================
// METRICS-TO-ALERTS INTEGRATION TESTS
// ============================================================================

#[test]
fn test_metrics_to_alerts_workflow() {
    // Create metrics
    let metrics = WarmRestartMetrics {
        warm_restart_count: 200,
        cold_start_count: 5,
        eoiu_detected_count: 200,
        eoiu_timeout_count: 120, // 60% timeout rate - high!
        state_recovery_count: 180,
        corruption_detected_count: 20,
        backup_created_count: 200,
        backup_cleanup_count: 100,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 8.0,
        max_initial_sync_duration_secs: 25,
        min_initial_sync_duration_secs: 2,
    };

    // Create alerting engine
    let mut engine = AlertingEngine::new();
    let default_rules = create_default_alert_rules();
    for rule in default_rules {
        engine.add_rule(rule);
    }

    // Evaluate
    let alerts = engine.evaluate(&metrics);

    // High timeout rate should trigger alert
    let timeout_alerts: Vec<_> = alerts
        .iter()
        .filter(|a| a.name.contains("EOIU Timeout"))
        .collect();
    assert!(!timeout_alerts.is_empty(), "Should have EOIU timeout alert");

    // Get relevant PromQL queries for investigation
    let error_queries = PromQLBuilder::queries_for_category(QueryCategory::ErrorRates);
    assert!(!error_queries.is_empty(), "Should have error rate queries");
}

#[test]
fn test_health_score_reflects_metrics_changes() {
    // Healthy metrics
    let healthy = WarmRestartMetrics {
        warm_restart_count: 1000,
        cold_start_count: 5,
        eoiu_detected_count: 1000,
        eoiu_timeout_count: 10,
        state_recovery_count: 995,
        corruption_detected_count: 2,
        backup_created_count: 1000,
        backup_cleanup_count: 500,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 4.0,
        max_initial_sync_duration_secs: 10,
        min_initial_sync_duration_secs: 1,
    };

    // Degraded metrics
    let degraded = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 40,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 60,
        state_recovery_count: 20,
        corruption_detected_count: 50,
        backup_created_count: 50,
        backup_cleanup_count: 10,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 150.0,
        max_initial_sync_duration_secs: 400,
        min_initial_sync_duration_secs: 50,
    };

    let healthy_score = healthy.health_score();
    let degraded_score = degraded.health_score();

    assert!(
        healthy_score > degraded_score,
        "Healthy should have higher score"
    );
    assert!(healthy_score > 70.0, "Healthy should have good score");
    assert!(degraded_score < 50.0, "Degraded should have poor score");
}

#[test]
fn test_anomaly_detection_integration() {
    let mut history = HistoricalMetrics::new(100);

    // Normal pattern
    for i in 0..10 {
        history.add_observation(
            "metric".to_string(),
            10.0 + (i as f64 * 0.5), // Gradual increase
        );
    }

    let observations = history.get_observations("metric");
    let anomalies = TrendAnalyzer::detect_anomalies(&observations);
    assert!(
        anomalies.is_empty(),
        "Normal pattern should have no anomalies"
    );

    // Add extreme value
    history.add_observation("metric".to_string(), 500.0);

    let updated_obs = history.get_observations("metric");
    let anomalies = TrendAnalyzer::detect_anomalies(&updated_obs);
    assert!(!anomalies.is_empty(), "Should detect extreme value");
    assert!(anomalies[0].severity >= AnomalySeverity::Minor);
}

// ============================================================================
// ALERT SEVERITY AND PRIORITY TESTS
// ============================================================================

#[test]
fn test_alert_severity_comparison() {
    assert!(AlertSeverity::Critical > AlertSeverity::Warning);
    assert!(AlertSeverity::Warning > AlertSeverity::Info);
    assert!(AlertSeverity::Critical > AlertSeverity::Info);
}

#[test]
fn test_multiple_alerts_with_different_severities() {
    let mut engine = AlertingEngine::new();

    // Add rules with different severities
    let critical_rule = AlertRule {
        rule_id: "critical".to_string(),
        name: "Critical".to_string(),
        description: "Critical rule".to_string(),
        metric_name: "corruption_detected_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 100.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Critical,
        actions: vec![],
    };

    let warning_rule = AlertRule {
        rule_id: "warning".to_string(),
        name: "Warning".to_string(),
        description: "Warning rule".to_string(),
        metric_name: "cold_start_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(critical_rule);
    engine.add_rule(warning_rule);

    let metrics = WarmRestartMetrics {
        warm_restart_count: 100,
        cold_start_count: 100, // Triggers warning
        eoiu_detected_count: 100,
        eoiu_timeout_count: 30,
        state_recovery_count: 90,
        corruption_detected_count: 150, // Triggers critical
        backup_created_count: 100,
        backup_cleanup_count: 50,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    engine.evaluate(&metrics);

    let critical = engine.alerts_by_severity(AlertSeverity::Critical);
    let warning = engine.alerts_by_severity(AlertSeverity::Warning);

    assert!(!critical.is_empty(), "Should have critical alert");
    assert!(!warning.is_empty(), "Should have warning alert");
}

// ============================================================================
// TIME WINDOW AND TEMPORAL TESTS
// ============================================================================

#[test]
fn test_time_windows_in_queries() {
    let windows = [
        TimeWindow::OneMinute,
        TimeWindow::FiveMinutes,
        TimeWindow::FifteenMinutes,
        TimeWindow::OneHour,
        TimeWindow::SixHours,
        TimeWindow::OneDay,
    ];

    let expected_durations = ["1m", "5m", "15m", "1h", "6h", "1d"];

    for (window, expected) in windows.iter().zip(expected_durations.iter()) {
        assert_eq!(window.to_promql_duration(), *expected);
    }
}

#[test]
fn test_trend_queries_with_different_windows() {
    let one_min = PromQLBuilder::restart_trend(TimeWindow::OneMinute);
    assert!(one_min.query.contains("1m"));

    let one_hour = PromQLBuilder::corruption_trend(TimeWindow::OneHour);
    assert!(one_hour.query.contains("1h"));

    let one_day = PromQLBuilder::recovery_trend(TimeWindow::OneDay);
    assert!(one_day.query.contains("1d"));
}
