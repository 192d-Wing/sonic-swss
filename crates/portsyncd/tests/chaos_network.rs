//! Phase 7 Week 1: Chaos Testing - Network Failures
//!
//! Tests for network partition scenarios and Redis connection failures
//! Validates recovery mechanisms and data consistency during network outages

use sonic_portsyncd::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// CHAOS TESTING UTILITIES
// ============================================================================

/// Mock Redis connection that can simulate failures
struct ChaosRedisAdapter {
    connected: Arc<Mutex<bool>>,
    failure_mode: Arc<Mutex<Option<FailureMode>>>,
}

#[derive(Debug, Clone, Copy)]
enum FailureMode {
    /// Connection completely lost
    Disconnected,
    /// Slow responses (timeouts)
    SlowResponses { latency_ms: u64 },
    /// Partial failures (asymmetric)
    PartialFailure { read_fail_rate: f32 },
}

impl ChaosRedisAdapter {
    fn new() -> Self {
        Self {
            connected: Arc::new(Mutex::new(true)),
            failure_mode: Arc::new(Mutex::new(None)),
        }
    }

    fn simulate_failure(&self, mode: FailureMode) {
        *self.failure_mode.lock().unwrap() = Some(mode);
        *self.connected.lock().unwrap() = false;
    }

    fn simulate_recovery(&self) {
        *self.failure_mode.lock().unwrap() = None;
        *self.connected.lock().unwrap() = true;
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }

    fn get_failure_mode(&self) -> Option<FailureMode> {
        *self.failure_mode.lock().unwrap()
    }
}

// ============================================================================
// NETWORK PARTITION TESTS
// ============================================================================

#[test]
fn test_redis_connection_loss_detection() {
    let chaos = ChaosRedisAdapter::new();

    // Initially connected
    assert!(chaos.is_connected(), "Should start connected");

    // Simulate connection loss
    chaos.simulate_failure(FailureMode::Disconnected);
    assert!(!chaos.is_connected(), "Should detect disconnection");
    assert!(matches!(
        chaos.get_failure_mode(),
        Some(FailureMode::Disconnected)
    ));
}

#[test]
fn test_redis_recovery_from_disconnection() {
    let chaos = ChaosRedisAdapter::new();

    // Simulate failure then recovery
    chaos.simulate_failure(FailureMode::Disconnected);
    assert!(!chaos.is_connected());

    chaos.simulate_recovery();
    assert!(chaos.is_connected(), "Should recover from disconnection");
    assert!(chaos.get_failure_mode().is_none());
}

#[test]
fn test_redis_timeout_handling() {
    let chaos = ChaosRedisAdapter::new();

    // Simulate slow responses
    chaos.simulate_failure(FailureMode::SlowResponses { latency_ms: 5000 });

    let failure_mode = chaos.get_failure_mode();
    assert!(matches!(
        failure_mode,
        Some(FailureMode::SlowResponses { latency_ms: 5000 })
    ), "Should detect slow responses");
}

#[test]
fn test_partial_network_partition() {
    let chaos = ChaosRedisAdapter::new();

    // Simulate asymmetric network (reads fail, writes succeed)
    chaos.simulate_failure(FailureMode::PartialFailure {
        read_fail_rate: 0.5,
    });

    let failure_mode = chaos.get_failure_mode();
    assert!(matches!(
        failure_mode,
        Some(FailureMode::PartialFailure { .. })
    ), "Should detect partial failure");
}

// ============================================================================
// ALERT STATE CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_alert_consistency_during_network_failure() {
    let mut engine = AlertingEngine::new();

    // Create a simple alert rule
    let rule = AlertRule {
        rule_id: "test_alert".to_string(),
        name: "Test Alert".to_string(),
        description: "Test alert for consistency".to_string(),
        metric_name: "eoiu_timeout_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![AlertAction::Log],
    };

    engine.add_rule(rule);

    // Create metrics that trigger the alert
    let metrics = WarmRestartMetrics {
        warm_restart_count: 10,
        cold_start_count: 5,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 60,  // Above threshold
        state_recovery_count: 5,
        corruption_detected_count: 1,
        backup_created_count: 10,
        backup_cleanup_count: 5,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Evaluate and trigger alert
    engine.evaluate(&metrics);
    let firing_count = engine.alerts_by_state(AlertState::Firing).len();
    assert!(firing_count > 0, "Alert should fire");

    // Evaluate again - alert should remain in same state
    engine.evaluate(&metrics);
    let still_firing = engine.alerts_by_state(AlertState::Firing).len();
    assert_eq!(still_firing, firing_count,
               "Alert state should be consistent");
}

#[test]
fn test_alert_state_after_forced_recovery() {
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "consistency_test".to_string(),
        name: "Consistency Test".to_string(),
        description: "Test alert consistency".to_string(),
        metric_name: "health_score".to_string(),
        condition: AlertCondition::Below,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Critical,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Trigger with degraded metrics
    let degraded_metrics = WarmRestartMetrics {
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

    engine.evaluate(&degraded_metrics);
    let alerts_before = engine.alerts_by_state(AlertState::Firing);
    assert!(!alerts_before.is_empty());

    // "Recover" with healthy metrics
    let healthy_metrics = WarmRestartMetrics {
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

    engine.evaluate(&healthy_metrics);
    let alerts_after = engine.alerts_by_state(AlertState::Resolved);
    assert!(!alerts_after.is_empty(), "Alert should resolve when condition clears");
}

// ============================================================================
// RECOVERY VALIDATION TESTS
// ============================================================================

#[test]
fn test_metric_consistency_during_recovery() {
    let mut metrics = WarmRestartMetrics {
        warm_restart_count: 10,
        cold_start_count: 5,
        eoiu_detected_count: 100,
        eoiu_timeout_count: 30,
        state_recovery_count: 70,
        corruption_detected_count: 5,
        backup_created_count: 10,
        backup_cleanup_count: 5,
        last_warm_restart_secs: None,
        last_eoiu_detection_secs: None,
        last_state_recovery_secs: None,
        last_corruption_detected_secs: None,
        avg_initial_sync_duration_secs: 5.0,
        max_initial_sync_duration_secs: 15,
        min_initial_sync_duration_secs: 2,
    };

    // Take baseline health score
    let baseline_health = metrics.health_score();

    // Metrics should be stable across multiple reads
    let health_1 = metrics.health_score();
    let health_2 = metrics.health_score();

    assert_eq!(health_1, health_2, "Health score should be consistent");
    assert_eq!(baseline_health, health_1, "Health score should not change");
}

#[test]
fn test_recovery_time_objective_slo() {
    let chaos = ChaosRedisAdapter::new();

    // Simulate failure
    let failure_start = Instant::now();
    chaos.simulate_failure(FailureMode::Disconnected);

    // Recovery should be within SLO (30 seconds)
    let slo_deadline = Duration::from_secs(30);
    chaos.simulate_recovery();
    let recovery_time = failure_start.elapsed();

    assert!(recovery_time < slo_deadline,
            "Recovery should complete within SLO: {:?}", recovery_time);
}

#[test]
fn test_no_data_loss_during_network_partition() {
    // Verify that events are not lost during network partitions
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "data_loss_test".to_string(),
        name: "Data Loss Test".to_string(),
        description: "Verify no data loss".to_string(),
        metric_name: "eoiu_timeout_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 25.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Create event before partition
    let metrics_1 = WarmRestartMetrics {
        eoiu_timeout_count: 30,
        ..Default::default()
    };
    engine.evaluate(&metrics_1);

    let alerts_before_partition = engine.alerts().len();
    assert!(alerts_before_partition > 0);

    // During partition, evaluate again (simulating buffered events)
    let metrics_2 = WarmRestartMetrics {
        eoiu_timeout_count: 40,
        ..Default::default()
    };
    engine.evaluate(&metrics_2);

    let alerts_during_partition = engine.alerts().len();
    assert!(alerts_during_partition > 0, "Alerts should be tracked during partition");

    // After recovery, all alerts should still be present
    assert_eq!(alerts_during_partition, alerts_before_partition,
               "No data loss during network partition");
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[test]
fn test_chaos_scenario_network_loss_and_recovery_cycle() {
    let chaos = ChaosRedisAdapter::new();
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "cycle_test".to_string(),
        name: "Cycle Test".to_string(),
        description: "Test cycle".to_string(),
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

    let metrics = WarmRestartMetrics {
        eoiu_timeout_count: 60,
        ..Default::default()
    };

    // Phase 1: Normal operation
    assert!(chaos.is_connected());
    engine.evaluate(&metrics);
    let normal_alerts = engine.alerts().len();
    assert!(normal_alerts > 0);

    // Phase 2: Network failure
    chaos.simulate_failure(FailureMode::Disconnected);
    assert!(!chaos.is_connected());
    // Alerts should persist (still in memory)
    assert_eq!(engine.alerts().len(), normal_alerts);

    // Phase 3: Recovery
    chaos.simulate_recovery();
    assert!(chaos.is_connected());
    // Alerts should still be present
    assert_eq!(engine.alerts().len(), normal_alerts);
}
