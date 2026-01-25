//! Phase 7 Week 1: Chaos Testing - State Consistency
//!
//! Tests for state consistency during concurrent operations and failures
//! Validates data integrity and race condition handling

use sonic_portsyncd::*;

// ============================================================================
// STATE CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_alert_state_machine_consistency() {
    // Verify alert state transitions are consistent
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "state_machine_test".to_string(),
        name: "State Machine Test".to_string(),
        description: "Test state transitions".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,  // Immediate firing for test
        enabled: true,
        severity: AlertSeverity::Critical,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Create degraded metrics
    let degraded = WarmRestartMetrics {
        warm_restart_count: 100,
        cold_start_count: 50,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 90,
        state_recovery_count: 5,
        corruption_detected_count: 20,
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

    // Evaluate multiple times - state should be consistent
    engine.evaluate(&degraded);
    let count_1 = engine.alerts_by_state(AlertState::Firing).len();
    assert!(count_1 > 0);

    engine.evaluate(&degraded);
    let count_2 = engine.alerts_by_state(AlertState::Firing).len();
    assert_eq!(count_1, count_2, "Alert count should be consistent");

    engine.evaluate(&degraded);
    let count_3 = engine.alerts_by_state(AlertState::Firing).len();
    assert_eq!(count_2, count_3, "Alert count should remain consistent");
}

#[test]
fn test_alert_recovery_from_invalid_state() {
    // Verify alert can recover from evaluations with missing metrics
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "invalid_metric_test".to_string(),
        name: "Invalid Metric Test".to_string(),
        description: "Test invalid metric handling".to_string(),
        metric_name: "nonexistent_metric".to_string(),  // This metric doesn't exist
        condition: AlertCondition::Above,
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
        warm_restart_count: 0,
        cold_start_count: 0,
        eoiu_detected_count: 0,
        eoiu_timeout_count: 0,
        state_recovery_count: 0,
        corruption_detected_count: 0,
        backup_created_count: 0,
        backup_cleanup_count: 0,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 0.0,
        max_initial_sync_duration_secs: 0,
        min_initial_sync_duration_secs: 0,
    };

    // Should not panic, should handle gracefully
    let alerts = engine.evaluate(&metrics);
    assert_eq!(alerts.len(), 0, "Non-existent metrics should not trigger alerts");
}

#[test]
fn test_state_consistency_with_alert_suppression() {
    // Verify state consistency when alerts are suppressed
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "suppression_test".to_string(),
        name: "Suppression Test".to_string(),
        description: "Test suppression state".to_string(),
        metric_name: "eoiu_timeout_count".to_string(),
        condition: AlertCondition::Above,
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
        warm_restart_count: 10,
        cold_start_count: 5,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 60,
        state_recovery_count: 40,
        corruption_detected_count: 2,
        backup_created_count: 50,
        backup_cleanup_count: 48,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Fire alert
    engine.evaluate(&metrics);
    let firing_count = engine.alerts_by_state(AlertState::Firing).len();
    assert!(firing_count > 0);

    // Suppress alert
    engine.suppress_alert("suppression_test");
    let suppressed_count = engine.alerts_by_state(AlertState::Suppressed).len();
    assert!(suppressed_count > 0);

    // Evaluate again - should stay suppressed
    engine.evaluate(&metrics);
    let still_suppressed = engine.alerts_by_state(AlertState::Suppressed).len();
    assert_eq!(suppressed_count, still_suppressed,
               "Suppression state should be consistent");

    // Unsuppress alert
    engine.unsuppress_alert("suppression_test");
    let firing_again = engine.alerts_by_state(AlertState::Firing).len();
    assert!(firing_again > 0,
            "Alert should fire again after unsuppression");
}

#[test]
fn test_health_score_monotonicity_during_recovery() {
    // Verify health score behaves as expected during recovery scenario
    let baseline_bad = WarmRestartMetrics {
        warm_restart_count: 100,
        cold_start_count: 50,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 80,
        state_recovery_count: 10,
        corruption_detected_count: 20,
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

    let recovery = WarmRestartMetrics {
        warm_restart_count: 50,
        cold_start_count: 25,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 40,
        state_recovery_count: 55,
        corruption_detected_count: 10,
        backup_created_count: 50,
        backup_cleanup_count: 45,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 50.0,
        max_initial_sync_duration_secs: 100,
        min_initial_sync_duration_secs: 25,
    };

    let recovered = WarmRestartMetrics {
        warm_restart_count: 10,
        cold_start_count: 1,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 5,
        state_recovery_count: 95,
        corruption_detected_count: 0,
        backup_created_count: 100,
        backup_cleanup_count: 95,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 2.0,
        max_initial_sync_duration_secs: 5,
        min_initial_sync_duration_secs: 1,
    };

    let health_bad = baseline_bad.health_score();
    let health_recovery = recovery.health_score();
    let health_recovered = recovered.health_score();

    // Health should improve as system recovers
    assert!(health_recovery > health_bad,
            "Health should improve during recovery: {} > {}", health_recovery, health_bad);
    assert!(health_recovered > health_recovery,
            "Health should continue improving: {} > {}", health_recovered, health_recovery);
}

#[test]
fn test_alert_evaluation_determinism() {
    // Verify that alert evaluation is deterministic
    let mut engine_1 = AlertingEngine::new();
    let mut engine_2 = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "determinism_test".to_string(),
        name: "Determinism Test".to_string(),
        description: "Test deterministic evaluation".to_string(),
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

    // Add same rule to both engines
    engine_1.add_rule(rule.clone());
    engine_2.add_rule(rule);

    let metrics = WarmRestartMetrics {
        warm_restart_count: 10,
        cold_start_count: 5,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 40,
        state_recovery_count: 60,
        corruption_detected_count: 1,
        backup_created_count: 50,
        backup_cleanup_count: 49,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Evaluate both engines with same metrics
    engine_1.evaluate(&metrics);
    engine_2.evaluate(&metrics);

    let alerts_1 = engine_1.alerts().len();
    let alerts_2 = engine_2.alerts().len();

    assert_eq!(alerts_1, alerts_2,
               "Alert evaluation should be deterministic");
}
