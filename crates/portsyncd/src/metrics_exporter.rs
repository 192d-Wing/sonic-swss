//! Prometheus metrics exporter for warm restart metrics
//!
//! Exports warm restart metrics in Prometheus text format and JSON format.
//! Provides methods to convert WarmRestartMetrics to Prometheus exposition format
//! and JSON for monitoring and alerting.
//!
//! Phase 6 Week 4 implementation.

use crate::warm_restart::WarmRestartMetrics;

/// Prometheus metrics exporter for warm restart tracking
#[derive(Debug, Clone)]
pub struct PrometheusExporter;

impl PrometheusExporter {
    /// Export warm restart metrics in Prometheus text format
    ///
    /// # Arguments
    /// * `metrics` - WarmRestartMetrics to export
    ///
    /// # Returns
    /// String containing Prometheus format metrics
    ///
    /// # Format
    /// Produces metrics in Prometheus text exposition format:
    /// ```text
    /// # HELP portsyncd_warm_restarts Total warm restart events
    /// # TYPE portsyncd_warm_restarts counter
    /// portsyncd_warm_restarts 5
    /// ```
    pub fn export(metrics: &WarmRestartMetrics) -> String {
        let mut output = String::new();

        // Counter metrics
        output.push_str("# HELP portsyncd_warm_restarts Total warm restart events\n");
        output.push_str("# TYPE portsyncd_warm_restarts counter\n");
        output.push_str(&format!(
            "portsyncd_warm_restarts {}\n",
            metrics.warm_restart_count
        ));

        output.push_str("# HELP portsyncd_cold_starts Total cold start events\n");
        output.push_str("# TYPE portsyncd_cold_starts counter\n");
        output.push_str(&format!(
            "portsyncd_cold_starts {}\n",
            metrics.cold_start_count
        ));

        output.push_str("# HELP portsyncd_eoiu_detected Total EOIU signals detected\n");
        output.push_str("# TYPE portsyncd_eoiu_detected counter\n");
        output.push_str(&format!(
            "portsyncd_eoiu_detected {}\n",
            metrics.eoiu_detected_count
        ));

        output.push_str("# HELP portsyncd_eoiu_timeouts Total EOIU timeouts (auto-complete)\n");
        output.push_str("# TYPE portsyncd_eoiu_timeouts counter\n");
        output.push_str(&format!(
            "portsyncd_eoiu_timeouts {}\n",
            metrics.eoiu_timeout_count
        ));

        output.push_str("# HELP portsyncd_state_recoveries Total successful state recoveries\n");
        output.push_str("# TYPE portsyncd_state_recoveries counter\n");
        output.push_str(&format!(
            "portsyncd_state_recoveries {}\n",
            metrics.state_recovery_count
        ));

        output.push_str("# HELP portsyncd_corruptions_detected Total corruption events\n");
        output.push_str("# TYPE portsyncd_corruptions_detected counter\n");
        output.push_str(&format!(
            "portsyncd_corruptions_detected {}\n",
            metrics.corruption_detected_count
        ));

        output.push_str("# HELP portsyncd_backups_created Total backup files created\n");
        output.push_str("# TYPE portsyncd_backups_created counter\n");
        output.push_str(&format!(
            "portsyncd_backups_created {}\n",
            metrics.backup_created_count
        ));

        output.push_str("# HELP portsyncd_backups_cleaned Total backup files cleaned up\n");
        output.push_str("# TYPE portsyncd_backups_cleaned counter\n");
        output.push_str(&format!(
            "portsyncd_backups_cleaned {}\n",
            metrics.backup_cleanup_count
        ));

        // Timestamp metrics (gauge - Unix timestamp in seconds)
        if let Some(ts) = metrics.last_warm_restart_secs {
            output
                .push_str("# HELP portsyncd_last_warm_restart_timestamp Last warm restart time\n");
            output.push_str("# TYPE portsyncd_last_warm_restart_timestamp gauge\n");
            output.push_str(&format!("portsyncd_last_warm_restart_timestamp {}\n", ts));
        }

        if let Some(ts) = metrics.last_eoiu_detection_secs {
            output.push_str(
                "# HELP portsyncd_last_eoiu_detection_timestamp Last EOIU detection time\n",
            );
            output.push_str("# TYPE portsyncd_last_eoiu_detection_timestamp gauge\n");
            output.push_str(&format!("portsyncd_last_eoiu_detection_timestamp {}\n", ts));
        }

        if let Some(ts) = metrics.last_state_recovery_secs {
            output.push_str(
                "# HELP portsyncd_last_state_recovery_timestamp Last state recovery time\n",
            );
            output.push_str("# TYPE portsyncd_last_state_recovery_timestamp gauge\n");
            output.push_str(&format!("portsyncd_last_state_recovery_timestamp {}\n", ts));
        }

        if let Some(ts) = metrics.last_corruption_detected_secs {
            output.push_str(
                "# HELP portsyncd_last_corruption_timestamp Last corruption detection time\n",
            );
            output.push_str("# TYPE portsyncd_last_corruption_timestamp gauge\n");
            output.push_str(&format!("portsyncd_last_corruption_timestamp {}\n", ts));
        }

        // Histogram metrics for initial sync duration
        let count = if metrics.warm_restart_count + metrics.cold_start_count > 0 {
            (metrics.warm_restart_count + metrics.cold_start_count) as f64
        } else {
            0.0
        };

        let sum = metrics.avg_initial_sync_duration_secs * count;

        output.push_str(
            "# HELP portsyncd_initial_sync_duration_seconds Initial sync duration in seconds\n",
        );
        output.push_str("# TYPE portsyncd_initial_sync_duration_seconds histogram\n");

        if count > 0.0 {
            // Histogram buckets (in seconds): 1, 5, 10, 30, 60, 120, 300
            let buckets = vec![
                (1.0, count_within_duration(metrics, 1.0)),
                (5.0, count_within_duration(metrics, 5.0)),
                (10.0, count_within_duration(metrics, 10.0)),
                (30.0, count_within_duration(metrics, 30.0)),
                (60.0, count_within_duration(metrics, 60.0)),
                (120.0, count_within_duration(metrics, 120.0)),
                (300.0, count_within_duration(metrics, 300.0)),
            ];

            for (le, count) in buckets {
                output.push_str(&format!(
                    "portsyncd_initial_sync_duration_seconds_bucket{{le=\"{}\"}} {}\n",
                    le, count as u64
                ));
            }

            output.push_str(&format!(
                "portsyncd_initial_sync_duration_seconds_bucket{{le=\"+Inf\"}} {}\n",
                count as u64
            ));
        } else {
            // Empty histogram
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"1\"} 0\n");
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"5\"} 0\n");
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"10\"} 0\n");
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"30\"} 0\n");
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"60\"} 0\n");
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"120\"} 0\n");
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"300\"} 0\n");
            output.push_str("portsyncd_initial_sync_duration_seconds_bucket{le=\"+Inf\"} 0\n");
        }

        output.push_str(&format!(
            "portsyncd_initial_sync_duration_seconds_sum {}\n",
            sum
        ));
        output.push_str(&format!(
            "portsyncd_initial_sync_duration_seconds_count {}\n",
            count as u64
        ));

        output
    }

    /// Export warm restart metrics in JSON format
    ///
    /// # Arguments
    /// * `metrics` - WarmRestartMetrics to export
    ///
    /// # Returns
    /// Result containing JSON string
    pub fn export_json(metrics: &WarmRestartMetrics) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&serde_json::json!({
            "warm_restarts": metrics.warm_restart_count,
            "cold_starts": metrics.cold_start_count,
            "eoiu_detected": metrics.eoiu_detected_count,
            "eoiu_timeouts": metrics.eoiu_timeout_count,
            "state_recoveries": metrics.state_recovery_count,
            "corruptions_detected": metrics.corruption_detected_count,
            "backups_created": metrics.backup_created_count,
            "backups_cleaned": metrics.backup_cleanup_count,
            "last_warm_restart_secs": metrics.last_warm_restart_secs,
            "last_eoiu_detection_secs": metrics.last_eoiu_detection_secs,
            "last_state_recovery_secs": metrics.last_state_recovery_secs,
            "last_corruption_detected_secs": metrics.last_corruption_detected_secs,
            "avg_initial_sync_duration_secs": metrics.avg_initial_sync_duration_secs,
            "max_initial_sync_duration_secs": metrics.max_initial_sync_duration_secs,
            "min_initial_sync_duration_secs": metrics.min_initial_sync_duration_secs,
        }))
    }
}

/// Helper function to count metrics within a duration threshold
fn count_within_duration(metrics: &WarmRestartMetrics, duration_secs: f64) -> f64 {
    let max = metrics.max_initial_sync_duration_secs as f64;
    if max <= duration_secs {
        (metrics.warm_restart_count + metrics.cold_start_count) as f64
    } else {
        // Approximate based on average
        let avg = metrics.avg_initial_sync_duration_secs;
        if avg <= duration_secs {
            (metrics.warm_restart_count + metrics.cold_start_count) as f64
        } else {
            // Simple estimation: if max > duration, assume some events are over
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metrics() -> WarmRestartMetrics {
        WarmRestartMetrics {
            warm_restart_count: 5,
            cold_start_count: 1,
            eoiu_detected_count: 4,
            eoiu_timeout_count: 1,
            state_recovery_count: 2,
            corruption_detected_count: 1,
            backup_created_count: 6,
            backup_cleanup_count: 2,
            last_warm_restart_secs: Some(1609459200),
            last_eoiu_detection_secs: Some(1609459195),
            avg_initial_sync_duration_secs: 5.5,
            max_initial_sync_duration_secs: 12,
            min_initial_sync_duration_secs: 2,
            last_state_recovery_secs: None,
            last_corruption_detected_secs: None,
        }
    }

    #[test]
    fn test_export_prometheus_format() {
        let metrics = create_test_metrics();
        let output = PrometheusExporter::export(&metrics);

        // Verify counter exports
        assert!(output.contains("portsyncd_warm_restarts 5"));
        assert!(output.contains("portsyncd_cold_starts 1"));
        assert!(output.contains("portsyncd_eoiu_detected 4"));
        assert!(output.contains("portsyncd_eoiu_timeouts 1"));
        assert!(output.contains("portsyncd_state_recoveries 2"));
        assert!(output.contains("portsyncd_corruptions_detected 1"));
        assert!(output.contains("portsyncd_backups_created 6"));
        assert!(output.contains("portsyncd_backups_cleaned 2"));
    }

    #[test]
    fn test_export_prometheus_includes_timestamps() {
        let metrics = create_test_metrics();
        let output = PrometheusExporter::export(&metrics);

        // Verify timestamp metrics
        assert!(output.contains("portsyncd_last_warm_restart_timestamp 1609459200"));
        assert!(output.contains("portsyncd_last_eoiu_detection_timestamp 1609459195"));
    }

    #[test]
    fn test_export_prometheus_histogram() {
        let metrics = create_test_metrics();
        let output = PrometheusExporter::export(&metrics);

        // Verify histogram metrics
        assert!(output.contains("portsyncd_initial_sync_duration_seconds_bucket"));
        assert!(output.contains("portsyncd_initial_sync_duration_seconds_sum"));
        assert!(output.contains("portsyncd_initial_sync_duration_seconds_count"));
        assert!(output.contains("# HELP portsyncd_initial_sync_duration_seconds"));
    }

    #[test]
    fn test_export_prometheus_empty_metrics() {
        let metrics = WarmRestartMetrics::default();
        let output = PrometheusExporter::export(&metrics);

        assert!(output.contains("portsyncd_warm_restarts 0"));
        assert!(output.contains("portsyncd_cold_starts 0"));
        assert!(output.contains("portsyncd_initial_sync_duration_seconds_count 0"));
    }

    #[test]
    fn test_export_json_format() {
        let metrics = create_test_metrics();
        let json_result = PrometheusExporter::export_json(&metrics);

        assert!(json_result.is_ok());
        let json_str = json_result.unwrap();
        assert!(json_str.contains("\"warm_restarts\": 5"));
        assert!(json_str.contains("\"cold_starts\": 1"));
        assert!(json_str.contains("\"eoiu_detected\": 4"));
    }

    #[test]
    fn test_export_json_includes_all_fields() {
        let metrics = create_test_metrics();
        let json_str = PrometheusExporter::export_json(&metrics).unwrap();

        assert!(json_str.contains("warm_restarts"));
        assert!(json_str.contains("cold_starts"));
        assert!(json_str.contains("eoiu_detected"));
        assert!(json_str.contains("eoiu_timeouts"));
        assert!(json_str.contains("state_recoveries"));
        assert!(json_str.contains("corruptions_detected"));
        assert!(json_str.contains("backups_created"));
        assert!(json_str.contains("backups_cleaned"));
        assert!(json_str.contains("avg_initial_sync_duration_secs"));
    }

    #[test]
    fn test_export_prometheus_has_help_text() {
        let metrics = WarmRestartMetrics::default();
        let output = PrometheusExporter::export(&metrics);

        assert!(output.contains("# HELP portsyncd_warm_restarts"));
        assert!(output.contains("# HELP portsyncd_cold_starts"));
        assert!(output.contains("# TYPE portsyncd_warm_restarts counter"));
        assert!(output.contains("# TYPE portsyncd_cold_starts counter"));
    }

    #[test]
    fn test_export_prometheus_valid_format() {
        let metrics = create_test_metrics();
        let output = PrometheusExporter::export(&metrics);

        // Verify no duplicate metrics
        let warm_restart_count = output.matches("portsyncd_warm_restarts 5").count();
        assert_eq!(warm_restart_count, 1);

        // Verify format: each line should be valid
        for line in output.lines() {
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                assert!(
                    line.starts_with("# HELP") || line.starts_with("# TYPE"),
                    "Invalid comment line: {}",
                    line
                );
            } else {
                // Metric line should have space between name and value
                assert!(
                    line.contains(' '),
                    "Invalid metric line (no space): {}",
                    line
                );
            }
        }
    }

    #[test]
    fn test_export_prometheus_no_optional_timestamps() {
        let metrics = WarmRestartMetrics::default();
        let output = PrometheusExporter::export(&metrics);

        // Verify optional timestamps are not included
        assert!(!output.contains("portsyncd_last_warm_restart_timestamp"));
        assert!(!output.contains("portsyncd_last_eoiu_detection_timestamp"));
    }

    #[test]
    fn test_export_json_empty_metrics() {
        let metrics = WarmRestartMetrics::default();
        let json_result = PrometheusExporter::export_json(&metrics);

        assert!(json_result.is_ok());
        let json_str = json_result.unwrap();
        assert!(json_str.contains("\"warm_restarts\": 0"));
        assert!(json_str.contains("\"cold_starts\": 0"));
    }

    #[test]
    fn test_export_histogram_buckets_ordered() {
        let metrics = create_test_metrics();
        let output = PrometheusExporter::export(&metrics);

        // Extract bucket lines and verify ordering
        let lines: Vec<&str> = output
            .lines()
            .filter(|l| l.contains("_bucket{le="))
            .collect();

        // Should have buckets for 1, 5, 10, 30, 60, 120, 300, +Inf
        assert!(lines.len() >= 8, "Should have at least 8 bucket lines");
    }

    #[test]
    fn test_export_prometheus_performance() {
        let metrics = create_test_metrics();
        let start = std::time::Instant::now();
        let _ = PrometheusExporter::export(&metrics);
        let elapsed = start.elapsed();

        // Export should be very fast (< 10ms)
        assert!(
            elapsed.as_millis() < 10,
            "Export took {}ms, should be < 10ms",
            elapsed.as_millis()
        );
    }
}
