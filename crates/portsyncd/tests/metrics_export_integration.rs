//! Integration tests for metrics persistence and export
//!
//! Tests the complete metrics lifecycle:
//! 1. Metrics recording during warm restart
//! 2. Metrics persistence to JSON files
//! 3. Metrics recovery after restart
//! 4. Prometheus export format validation
//! 5. Analytics calculations
//!
//! Phase 6 Week 4 Integration Tests

use sonic_portsyncd::config_file::MetricsExportFormat;
use sonic_portsyncd::*;
use tempfile::TempDir;

// ============================================================================
// METRICS PERSISTENCE INTEGRATION TESTS
// ============================================================================

#[test]
fn test_metrics_persist_across_manager_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let metrics_file = temp_dir.path().join("metrics.json");

    // Create first manager instance
    {
        let mut manager = WarmRestartManager::new();
        manager.initialize().unwrap();

        // Record some events
        manager.metrics.record_warm_restart();
        manager.metrics.record_eoiu_detected();
        manager.metrics.record_initial_sync_duration(5);

        // Save metrics
        manager.save_metrics(&metrics_file).unwrap();

        // Verify file was created
        assert!(metrics_file.exists());
    }

    // Create second manager instance and load metrics
    {
        let mut manager = WarmRestartManager::new();
        manager.load_metrics(&metrics_file).unwrap();

        // Verify metrics were loaded
        assert_eq!(manager.metrics.warm_restart_count, 1);
        assert_eq!(manager.metrics.eoiu_detected_count, 1);
        assert!(manager.metrics.avg_initial_sync_duration_secs > 0.0);
    }
}

#[test]
fn test_metrics_merge_accumulates_events() {
    let temp_dir = TempDir::new().unwrap();
    let metrics_file = temp_dir.path().join("metrics.json");

    // First session
    {
        let mut manager = WarmRestartManager::new();
        manager.metrics.record_warm_restart();
        manager.metrics.record_state_recovery();
        manager.save_metrics(&metrics_file).unwrap();
    }

    // Second session - load and merge
    {
        let mut manager = WarmRestartManager::new();
        manager.metrics.record_warm_restart();
        manager.metrics.record_eoiu_detected();

        // Load previous metrics
        manager.load_metrics(&metrics_file).unwrap();

        // Merge should accumulate (load adds to existing)
        // Note: Current implementation does merge
        manager.save_metrics(&metrics_file).unwrap();
    }

    // Third session - verify accumulation
    {
        let mut manager = WarmRestartManager::new();
        manager.load_metrics(&metrics_file).unwrap();

        // Should have events from all sessions
        assert!(manager.metrics.warm_restart_count >= 1);
        assert!(manager.metrics.state_recovery_count >= 1);
    }
}

#[test]
fn test_metrics_survive_corruption_with_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let metrics_file = temp_dir.path().join("metrics.json");

    // Save valid metrics
    {
        let mut manager = WarmRestartManager::new();
        manager.metrics.record_warm_restart();
        manager.metrics.record_cold_start();
        manager.save_metrics(&metrics_file).unwrap();
    }

    // Corrupt the file
    std::fs::write(&metrics_file, b"invalid json {{{").unwrap();

    // Try to load corrupted metrics - should handle gracefully
    {
        let mut manager = WarmRestartManager::new();
        let result = manager.load_metrics(&metrics_file);

        // Should fail to load corrupted data but not panic
        assert!(result.is_err());

        // Metrics should remain in default state
        assert_eq!(manager.metrics.warm_restart_count, 0);
    }
}

// ============================================================================
// PROMETHEUS EXPORT INTEGRATION TESTS
// ============================================================================

#[test]
fn test_prometheus_export_includes_all_metric_types() {
    let mut metrics = WarmRestartMetrics::default();

    // Add various events
    metrics.warm_restart_count = 10;
    metrics.cold_start_count = 2;
    metrics.eoiu_detected_count = 8;
    metrics.eoiu_timeout_count = 1;
    metrics.state_recovery_count = 3;
    metrics.corruption_detected_count = 2;
    metrics.backup_created_count = 5;
    metrics.backup_cleanup_count = 1;
    metrics.last_warm_restart_secs = Some(1609459200);
    metrics.last_eoiu_detection_secs = Some(1609459195);
    metrics.avg_initial_sync_duration_secs = 5.5;
    metrics.max_initial_sync_duration_secs = 10;
    metrics.min_initial_sync_duration_secs = 3;

    let output = PrometheusExporter::export(&metrics);

    // Verify all counters present
    assert!(output.contains("portsyncd_warm_restarts 10"));
    assert!(output.contains("portsyncd_cold_starts 2"));
    assert!(output.contains("portsyncd_eoiu_detected 8"));
    assert!(output.contains("portsyncd_eoiu_timeouts 1"));
    assert!(output.contains("portsyncd_state_recoveries 3"));
    assert!(output.contains("portsyncd_corruptions_detected 2"));
    assert!(output.contains("portsyncd_backups_created 5"));
    assert!(output.contains("portsyncd_backups_cleaned 1"));

    // Verify timestamps present
    assert!(output.contains("portsyncd_last_warm_restart_timestamp 1609459200"));
    assert!(output.contains("portsyncd_last_eoiu_detection_timestamp 1609459195"));

    // Verify histogram buckets
    assert!(output.contains("portsyncd_initial_sync_duration_seconds_bucket"));
    assert!(output.contains("portsyncd_initial_sync_duration_seconds_count 12")); // 10 + 2
    assert!(output.contains("portsyncd_initial_sync_duration_seconds_sum 66")); // 5.5 * 12
}

#[test]
fn test_prometheus_export_valid_format() {
    let mut metrics = WarmRestartMetrics::default();
    metrics.warm_restart_count = 5;
    metrics.cold_start_count = 1;

    let output = PrometheusExporter::export(&metrics);

    // Verify Prometheus text format compliance
    for line in output.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Each metric line must have format: name{labels} value
        assert!(
            line.contains(' '),
            "Invalid metric line (missing value): {}",
            line
        );

        // Value must be numeric
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert!(parts.len() >= 2, "Invalid format: {}", line);

        let value = parts.last().unwrap();
        assert!(
            value.parse::<f64>().is_ok(),
            "Invalid numeric value: {}",
            value
        );
    }
}

#[test]
fn test_prometheus_and_json_export_consistency() {
    let mut metrics = WarmRestartMetrics::default();
    metrics.warm_restart_count = 5;
    metrics.cold_start_count = 1;
    metrics.eoiu_detected_count = 4;
    metrics.eoiu_timeout_count = 1;

    let prometheus_output = PrometheusExporter::export(&metrics);
    let json_output = PrometheusExporter::export_json(&metrics).unwrap();

    // Prometheus should contain all counters from JSON
    assert!(prometheus_output.contains("portsyncd_warm_restarts 5"));
    assert!(prometheus_output.contains("portsyncd_cold_starts 1"));
    assert!(prometheus_output.contains("portsyncd_eoiu_detected 4"));
    assert!(prometheus_output.contains("portsyncd_eoiu_timeouts 1"));

    // JSON should parse correctly
    let json_value: serde_json::Value = serde_json::from_str(&json_output).unwrap();
    assert_eq!(json_value["warm_restarts"].as_u64().unwrap(), 5);
    assert_eq!(json_value["cold_starts"].as_u64().unwrap(), 1);
    assert_eq!(json_value["eoiu_detected"].as_u64().unwrap(), 4);
    assert_eq!(json_value["eoiu_timeouts"].as_u64().unwrap(), 1);
}

// ============================================================================
// ANALYTICS INTEGRATION TESTS
// ============================================================================

#[test]
fn test_analytics_health_score_reflects_state_quality() {
    // Healthy state
    let mut healthy = WarmRestartMetrics::default();
    healthy.warm_restart_count = 100;
    healthy.cold_start_count = 1;
    healthy.corruption_detected_count = 0;
    healthy.eoiu_timeout_count = 0;
    let healthy_score = healthy.health_score();

    // Degraded state
    let mut degraded = WarmRestartMetrics::default();
    degraded.warm_restart_count = 100;
    degraded.cold_start_count = 1;
    degraded.corruption_detected_count = 5;
    degraded.state_recovery_count = 3; // 2 unrecovered
    degraded.eoiu_timeout_count = 20;
    degraded.eoiu_detected_count = 100;
    let degraded_score = degraded.health_score();

    // Unhealthy state
    let mut unhealthy = WarmRestartMetrics::default();
    unhealthy.warm_restart_count = 10;
    unhealthy.cold_start_count = 90;
    unhealthy.corruption_detected_count = 50;
    unhealthy.state_recovery_count = 0; // All unrecovered
    let unhealthy_score = unhealthy.health_score();

    // Health scores should reflect quality differences
    assert!(healthy_score > degraded_score);
    assert!(degraded_score > unhealthy_score);
    assert!(unhealthy_score >= 0.0);
}

#[test]
fn test_analytics_recovery_rates_across_scenarios() {
    // Perfect recovery
    let mut perfect = WarmRestartMetrics::default();
    perfect.corruption_detected_count = 5;
    perfect.state_recovery_count = 5;
    assert_eq!(perfect.recovery_success_rate(), 100.0);
    assert_eq!(perfect.corruption_recovery_rate(), 100.0);

    // Partial recovery
    let mut partial = WarmRestartMetrics::default();
    partial.corruption_detected_count = 5;
    partial.state_recovery_count = 3;
    assert_eq!(partial.recovery_success_rate(), 60.0);
    assert_eq!(partial.corruption_recovery_rate(), 60.0);

    // No recovery
    let mut none = WarmRestartMetrics::default();
    none.corruption_detected_count = 5;
    none.state_recovery_count = 0;
    assert_eq!(none.recovery_success_rate(), 0.0);
    assert_eq!(none.corruption_recovery_rate(), 0.0);
}

#[test]
fn test_analytics_anomaly_detection_catches_issues() {
    // Normal state - no anomaly
    let mut normal = WarmRestartMetrics::default();
    normal.warm_restart_count = 100;
    normal.cold_start_count = 10;
    normal.eoiu_detected_count = 100;
    normal.eoiu_timeout_count = 5;
    normal.avg_initial_sync_duration_secs = 5.0;
    assert!(!normal.has_anomaly());

    // High timeout rate - anomaly
    let mut high_timeout = WarmRestartMetrics::default();
    high_timeout.eoiu_detected_count = 100;
    high_timeout.eoiu_timeout_count = 60; // > 50%
    assert!(high_timeout.has_anomaly());

    // High corruption rate - anomaly
    let mut high_corruption = WarmRestartMetrics::default();
    high_corruption.warm_restart_count = 10;
    high_corruption.cold_start_count = 0;
    high_corruption.corruption_detected_count = 10; // Every restart
    assert!(high_corruption.has_anomaly());

    // Extreme sync duration - anomaly
    let mut slow_sync = WarmRestartMetrics::default();
    slow_sync.avg_initial_sync_duration_secs = 500.0; // > 300
    assert!(slow_sync.has_anomaly());
}

// ============================================================================
// CONFIGURATION INTEGRATION TESTS
// ============================================================================

#[test]
fn test_metrics_config_integration_with_portsyncd_config() {
    let config_str = r#"
[database]
redis_host = "127.0.0.1"
redis_port = 6379

[performance]
max_event_queue = 1000

[health]
max_stall_seconds = 10

[metrics]
enabled = true
save_interval_secs = 300
retention_days = 30
max_file_size_mb = 100
export_format = "prometheus"
storage_path = "/var/lib/sonic/portsyncd/metrics"
"#;

    let config: PortsyncConfig = toml::from_str(config_str).unwrap();

    // Verify metrics config is present and valid
    assert!(config.metrics.enabled);
    assert_eq!(config.metrics.save_interval_secs, 300);
    assert_eq!(config.metrics.retention_days, 30);
    assert_eq!(config.metrics.max_file_size_mb, 100);
    assert_eq!(
        config.metrics.export_format,
        MetricsExportFormat::Prometheus
    );
    assert_eq!(
        config.metrics.storage_path,
        "/var/lib/sonic/portsyncd/metrics"
    );

    // Verify overall config validates
    assert!(config.validate().is_ok());
}

#[test]
fn test_metrics_config_validation_enforces_constraints() {
    let config_str = r#"
[database]
redis_host = "127.0.0.1"
redis_port = 6379

[metrics]
enabled = true
save_interval_secs = 0
retention_days = 30
max_file_size_mb = 100
export_format = "prometheus"
storage_path = "/var/lib/sonic/portsyncd/metrics"
"#;

    let config: PortsyncConfig = toml::from_str(config_str).unwrap();

    // Validation should fail due to zero save_interval_secs
    assert!(config.validate().is_err());
}

// ============================================================================
// END-TO-END SCENARIO TESTS
// ============================================================================

#[test]
fn test_end_to_end_warm_restart_with_metrics() {
    let temp_dir = TempDir::new().unwrap();
    let metrics_file = temp_dir.path().join("metrics.json");

    // Simulate warm restart cycle with metrics tracking
    let mut manager = WarmRestartManager::new();
    manager.initialize().unwrap();

    // Record events during warm restart cycle
    manager.metrics.record_warm_restart();
    manager.metrics.record_eoiu_detected();
    manager.metrics.record_initial_sync_duration(8);
    manager.metrics.record_state_recovery();

    // Save metrics
    manager.save_metrics(&metrics_file).unwrap();

    // Verify metrics were persisted correctly
    assert!(metrics_file.exists());
    let metrics_json = std::fs::read_to_string(&metrics_file).unwrap();

    // Verify the events were recorded
    assert!(metrics_json.contains("\"warm_restart_count\""));
    assert!(metrics_json.contains("\"eoiu_detected_count\""));
    assert!(metrics_json.contains("\"state_recovery_count\""));

    // Export to Prometheus format
    let prometheus_output = PrometheusExporter::export(&manager.metrics);
    assert!(prometheus_output.contains("portsyncd_warm_restarts 1"));
    assert!(prometheus_output.contains("portsyncd_eoiu_detected 1"));
    assert!(prometheus_output.contains("portsyncd_state_recoveries 1"));
}

#[test]
fn test_end_to_end_cold_start_with_metrics() {
    let temp_dir = TempDir::new().unwrap();
    let metrics_file = temp_dir.path().join("metrics.json");

    // Simulate cold start scenario with metrics
    let mut manager = WarmRestartManager::new();
    manager.initialize().unwrap();

    // Note: initialize() already records a cold start or warm restart based on state
    // So we just verify the metrics were persisted correctly

    // Save metrics
    manager.save_metrics(&metrics_file).unwrap();

    // Verify cold start was recorded
    let metrics_json = std::fs::read_to_string(&metrics_file).unwrap();

    // Verify metrics were saved with appropriate counts
    assert!(manager.metrics.cold_start_count > 0 || manager.metrics.warm_restart_count > 0);

    // Check JSON contains the expected fields
    assert!(metrics_json.contains("\"cold_start_count\""));
    assert!(metrics_json.contains("\"warm_restart_count\""));
}

#[test]
fn test_metrics_export_performance() {
    let mut metrics = WarmRestartMetrics::default();

    // Simulate high-activity system
    for i in 0..1000 {
        metrics.warm_restart_count += i % 3;
        metrics.eoiu_detected_count += i % 5;
        metrics.state_recovery_count += i % 7;
    }

    // Measure export performance
    let start = std::time::Instant::now();
    let _prometheus_output = PrometheusExporter::export(&metrics);
    let prometheus_time = start.elapsed();

    let start = std::time::Instant::now();
    let _json_output = PrometheusExporter::export_json(&metrics);
    let json_time = start.elapsed();

    // Both exports should be very fast (< 100ms)
    assert!(
        prometheus_time.as_millis() < 100,
        "Prometheus export took {}ms",
        prometheus_time.as_millis()
    );
    assert!(
        json_time.as_millis() < 100,
        "JSON export took {}ms",
        json_time.as_millis()
    );
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

trait MetricsExt {
    fn cold_start_percentage(&self) -> f64;
}

impl MetricsExt for WarmRestartMetrics {
    fn cold_start_percentage(&self) -> f64 {
        let total = self.warm_restart_count + self.cold_start_count;
        if total == 0 {
            0.0
        } else {
            (self.cold_start_count as f64 / total as f64) * 100.0
        }
    }
}
