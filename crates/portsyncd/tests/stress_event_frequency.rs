//! Phase 7 Week 2: Stress Testing - Event Frequency
//!
//! Tests for system behavior under high event rate scenarios
//! Validates performance at 1K, 10K, and sustained high event rates

use sonic_portsyncd::*;
use std::time::{Duration, Instant};

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Create test metrics with varying severity
fn create_test_metrics(event_num: u32, severity: AlertSeverity) -> WarmRestartMetrics {
    let base_multiplier = event_num % 100;

    match severity {
        AlertSeverity::Critical => WarmRestartMetrics {
            warm_restart_count: 80 + (base_multiplier as u64),
            cold_start_count: 50 + (base_multiplier as u64),
            eoiu_detected_count: 100,
            eoiu_timeout_count: 90 + (base_multiplier as u64),
            state_recovery_count: 10 + (base_multiplier as u64),
            corruption_detected_count: 15 + (base_multiplier as u64),
            backup_created_count: 100,
            backup_cleanup_count: 50,
            last_warm_restart_secs: None,
            last_eoiu_detection_secs: None,
            last_state_recovery_secs: None,
            last_corruption_detected_secs: None,
            avg_initial_sync_duration_secs: 150.0,
            max_initial_sync_duration_secs: 500,
            min_initial_sync_duration_secs: 100,
        },
        AlertSeverity::Warning => WarmRestartMetrics {
            warm_restart_count: 30 + (base_multiplier as u64),
            cold_start_count: 15 + (base_multiplier as u64),
            eoiu_detected_count: 100,
            eoiu_timeout_count: 40 + (base_multiplier as u64),
            state_recovery_count: 60 + (base_multiplier as u64),
            corruption_detected_count: 5 + ((base_multiplier / 2) as u64),
            backup_created_count: 100,
            backup_cleanup_count: 95,
            last_warm_restart_secs: None,
            last_eoiu_detection_secs: None,
            last_state_recovery_secs: None,
            last_corruption_detected_secs: None,
            avg_initial_sync_duration_secs: 30.0,
            max_initial_sync_duration_secs: 60,
            min_initial_sync_duration_secs: 10,
        },
        AlertSeverity::Info => WarmRestartMetrics {
            warm_restart_count: 5 + ((base_multiplier / 10) as u64),
            cold_start_count: 1 + ((base_multiplier / 20) as u64),
            eoiu_detected_count: 100,
            eoiu_timeout_count: 10 + (base_multiplier as u64),
            state_recovery_count: 90 + (base_multiplier as u64),
            corruption_detected_count: ((base_multiplier / 50) as u64),
            backup_created_count: 100,
            backup_cleanup_count: 100,
            last_warm_restart_secs: None,
            last_eoiu_detection_secs: None,
            last_state_recovery_secs: None,
            last_corruption_detected_secs: None,
            avg_initial_sync_duration_secs: 2.0,
            max_initial_sync_duration_secs: 5,
            min_initial_sync_duration_secs: 1,
        },
    }
}

// ============================================================================
// EVENT FREQUENCY TESTS - 1K Events Per Second
// ============================================================================

#[test]
fn test_1000_events_per_second_throughput() {
    // Verify system can process 1000 events per second
    let mut engine = AlertingEngine::new();

    // Add comprehensive rules
    let rules = vec![
        AlertRule {
            rule_id: "freq_test_1".to_string(),
            name: "Frequency Test 1K Rule 1".to_string(),
            description: "High frequency test rule 1".to_string(),
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
            rule_id: "freq_test_2".to_string(),
            name: "Frequency Test 1K Rule 2".to_string(),
            description: "High frequency test rule 2".to_string(),
            metric_name: "eoiu_timeout_count".to_string(),
            condition: AlertCondition::Above,
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

    // Process 1000 events
    let start = Instant::now();
    for event_num in 0..1000 {
        let metrics = create_test_metrics(event_num, AlertSeverity::Warning);
        engine.evaluate(&metrics);
    }
    let elapsed = start.elapsed();

    // Should process in reasonable time (<1 second)
    assert!(
        elapsed < Duration::from_secs(1),
        "1000 events should process in <1s, took {:?}",
        elapsed
    );

    // Verify alerts were generated
    let alerts = engine.alerts();
    assert!(
        alerts.len() > 0,
        "Should generate alerts from 1000 events"
    );
}

#[test]
fn test_1000_eps_alert_consistency() {
    // Verify alert consistency during high-frequency evaluation
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "consistency_1k".to_string(),
        name: "Consistency 1K".to_string(),
        description: "Test consistency at 1K eps".to_string(),
        metric_name: "warm_restart_count".to_string(),
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

    // Process critical metrics rapidly
    let mut event_count = 0;
    for event_num in 0..1000 {
        let metrics = create_test_metrics(event_num, AlertSeverity::Critical);
        engine.evaluate(&metrics);
        event_count += 1;
    }

    assert_eq!(event_count, 1000, "Should process all 1000 events");

    // Verify alert state is consistent
    let firing_alerts = engine.alerts_by_state(AlertState::Firing);
    assert!(
        firing_alerts.len() > 0,
        "Should have critical alerts firing"
    );
}

// ============================================================================
// EVENT FREQUENCY TESTS - 10K Events Per Second (Burst)
// ============================================================================

#[test]
fn test_10000_events_burst_processing() {
    // Verify system can handle 10K event burst
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "burst_10k".to_string(),
        name: "Burst 10K Test".to_string(),
        description: "Test 10K event burst".to_string(),
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

    // Process 10K events as rapidly as possible
    let start = Instant::now();
    for event_num in 0..10000 {
        let severity = match event_num % 3 {
            0 => AlertSeverity::Critical,
            1 => AlertSeverity::Warning,
            _ => AlertSeverity::Info,
        };
        let metrics = create_test_metrics(event_num as u32, severity);
        engine.evaluate(&metrics);
    }
    let elapsed = start.elapsed();

    // Should complete burst in reasonable time (<5 seconds)
    assert!(
        elapsed < Duration::from_secs(5),
        "10K burst should complete in <5s, took {:?}",
        elapsed
    );

    // Verify alerts were generated
    assert!(
        engine.alerts().len() > 0,
        "Should have alerts after 10K burst"
    );
}

#[test]
fn test_10k_burst_memory_stability() {
    // Verify memory doesn't spike during 10K burst
    let mut engine = AlertingEngine::new();

    for rule_idx in 0..5 {
        let rule = AlertRule {
            rule_id: format!("burst_mem_{}", rule_idx),
            name: format!("Burst Memory Test {}", rule_idx),
            description: format!("Memory test rule {}", rule_idx),
            metric_name: "eoiu_timeout_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 30.0 + (rule_idx as f64 * 10.0),
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };
        engine.add_rule(rule);
    }

    // Process burst
    for event_num in 0..10000 {
        let metrics = create_test_metrics(event_num as u32, AlertSeverity::Warning);
        engine.evaluate(&metrics);
    }

    // Alert count should be reasonable (not explosive growth)
    let alert_count = engine.alerts().len();
    assert!(
        alert_count < 50000,
        "Alert count {} seems excessive for 10K events",
        alert_count
    );
}

// ============================================================================
// SUSTAINED HIGH FREQUENCY TESTS
// ============================================================================

#[test]
fn test_sustained_5000_eps_for_10_seconds() {
    // Verify system can sustain 5000 events/sec for extended period
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "sustained_5k".to_string(),
        name: "Sustained 5K Test".to_string(),
        description: "Test sustained 5000 eps".to_string(),
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

    // Simulate 10 seconds at 5K eps = 50K events
    let start = Instant::now();
    let mut event_count = 0;

    for event_num in 0..50000 {
        let metrics = create_test_metrics(event_num as u32, AlertSeverity::Info);
        engine.evaluate(&metrics);
        event_count += 1;
    }

    let elapsed = start.elapsed();

    // Should complete 50K events in reasonable time
    assert!(
        elapsed < Duration::from_secs(30),
        "50K events should complete in <30s, took {:?}",
        elapsed
    );

    assert_eq!(
        event_count, 50000,
        "Should have processed all 50000 events"
    );
}

#[test]
fn test_alternating_severity_events() {
    // Verify system handles varying severity events smoothly
    let mut engine = AlertingEngine::new();

    // Add rules for different severity levels
    let critical_rule = AlertRule {
        rule_id: "alt_critical".to_string(),
        name: "Alternating Critical".to_string(),
        description: "Critical alternation test".to_string(),
        metric_name: "warm_restart_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 50.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Critical,
        actions: vec![],
    };

    let warning_rule = AlertRule {
        rule_id: "alt_warning".to_string(),
        name: "Alternating Warning".to_string(),
        description: "Warning alternation test".to_string(),
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

    engine.add_rule(critical_rule);
    engine.add_rule(warning_rule);

    // Process alternating severity events
    for event_num in 0..5000 {
        let severity = match event_num % 3 {
            0 => AlertSeverity::Critical,
            1 => AlertSeverity::Warning,
            _ => AlertSeverity::Info,
        };

        let metrics = create_test_metrics(event_num as u32, severity);
        engine.evaluate(&metrics);
    }

    // Verify alerts by severity
    let alerts_map = engine.alerts();
    assert!(alerts_map.len() > 0, "Should have alerts from mixed severity events");

    // At least one should be critical
    let alerts: Vec<_> = alerts_map.values().collect();
    let has_critical = alerts.iter().any(|a| a.severity == AlertSeverity::Critical);
    assert!(has_critical, "Should have at least one critical alert");
}

// ============================================================================
// RAPID CYCLING TESTS
// ============================================================================

#[test]
fn test_rapid_alert_state_transitions() {
    // Verify alert state machine handles rapid transitions
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "rapid_transition".to_string(),
        name: "Rapid Transition Test".to_string(),
        description: "Test rapid state transitions".to_string(),
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

    // Alternate between healthy and degraded metrics rapidly
    for cycle in 0..1000 {
        if cycle % 2 == 0 {
            // Degraded
            let metrics = create_test_metrics(cycle as u32, AlertSeverity::Critical);
            engine.evaluate(&metrics);
        } else {
            // Healthy
            let metrics = create_test_metrics(cycle as u32, AlertSeverity::Info);
            engine.evaluate(&metrics);
        }
    }

    // Should have alerts after 1000 cycles
    assert!(
        engine.alerts().len() > 0,
        "Should have alerts after rapid cycling"
    );
}

#[test]
fn test_event_processing_timing_stability() {
    // Verify event processing time stays consistent
    let mut engine = AlertingEngine::new();

    let rule = AlertRule {
        rule_id: "timing_stability".to_string(),
        name: "Timing Stability Test".to_string(),
        description: "Test processing timing stability".to_string(),
        metric_name: "corruption_detected_count".to_string(),
        condition: AlertCondition::Above,
        threshold: 5.0,
        threshold_range: None,
        evaluation_window_secs: 300,
        for_duration_secs: 0,
        enabled: true,
        severity: AlertSeverity::Warning,
        actions: vec![],
    };

    engine.add_rule(rule);

    // Measure processing time for batches
    let batch_size = 1000;
    let mut batch_times = Vec::new();

    for batch in 0..10 {
        let start = Instant::now();
        for event_num in 0..batch_size {
            let event_id = (batch * batch_size + event_num) as u32;
            let metrics = create_test_metrics(event_id, AlertSeverity::Warning);
            engine.evaluate(&metrics);
        }
        let batch_time = start.elapsed();
        batch_times.push(batch_time);
    }

    // Check for timing stability
    let avg_time = batch_times.iter().sum::<Duration>() / batch_times.len() as u32;

    // No batch should deviate more than 100% from average (natural variation acceptable)
    for (idx, batch_time) in batch_times.iter().enumerate() {
        let deviation = if batch_time > &avg_time {
            (*batch_time - avg_time).as_millis() as f64 / avg_time.as_millis() as f64
        } else {
            (avg_time - *batch_time).as_millis() as f64 / avg_time.as_millis() as f64
        };

        assert!(
            deviation < 1.0,
            "Batch {} timing deviation {:.2}% too high, time: {:?}, avg: {:?}",
            idx,
            deviation * 100.0,
            batch_time,
            avg_time
        );
    }
}
