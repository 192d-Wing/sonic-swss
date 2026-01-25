//! Trend analysis and anomaly detection for portsyncd metrics
//!
//! Analyzes historical metrics to detect trends, patterns, and anomalies.
//! Provides predictive insights for proactive monitoring and capacity planning.
//!
//! Features:
//! - Historical metrics collection with circular buffer
//! - Trend detection (increasing, decreasing, stable)
//! - Seasonality and pattern analysis
//! - Anomaly detection using statistical methods
//! - Predictive scoring for capacity planning
//!
//! Phase 6 Week 5 implementation.

use crate::warm_restart::WarmRestartMetrics;
use std::collections::VecDeque;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Single metric observation at a point in time
#[derive(Debug, Clone)]
pub struct MetricObservation {
    pub timestamp_secs: u64,
    pub metric_name: String,
    pub value: f64,
}

/// Trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    /// Metric is increasing over time
    Increasing,
    /// Metric is decreasing over time
    Decreasing,
    /// Metric is stable/flat
    Stable,
}

/// Anomaly severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalySeverity {
    /// Minor deviation from normal
    Minor,
    /// Moderate deviation
    Moderate,
    /// Severe deviation
    Severe,
}

/// Detected anomaly
#[derive(Debug, Clone)]
pub struct Anomaly {
    pub metric_name: String,
    pub timestamp_secs: u64,
    pub value: f64,
    pub expected_value: f64,
    pub severity: AnomalySeverity,
    pub description: String,
}

/// Trend analysis result
#[derive(Debug, Clone)]
pub struct TrendAnalysis {
    pub metric_name: String,
    pub direction: TrendDirection,
    pub slope: f64,      // Rate of change
    pub confidence: f64, // 0-1, higher means more confident
    pub duration_secs: u64,
    pub start_value: f64,
    pub end_value: f64,
}

/// Seasonality pattern
#[derive(Debug, Clone)]
pub struct SeasonalPattern {
    pub metric_name: String,
    pub period_secs: u64,
    pub amplitude: f64,
    pub offset: f64,
    pub confidence: f64,
}

/// Historical metrics storage
pub struct HistoricalMetrics {
    observations: VecDeque<MetricObservation>,
    max_observations: usize,
    metric_windows: std::collections::HashMap<String, usize>,
}

impl HistoricalMetrics {
    /// Create new historical metrics storage
    pub fn new(max_observations: usize) -> Self {
        Self {
            observations: VecDeque::with_capacity(max_observations),
            max_observations,
            metric_windows: std::collections::HashMap::new(),
        }
    }

    /// Add a metric observation
    pub fn add_observation(&mut self, metric_name: String, value: f64) {
        let observation = MetricObservation {
            timestamp_secs: current_timestamp_secs(),
            metric_name: metric_name.clone(),
            value,
        };

        self.observations.push_back(observation);
        if self.observations.len() > self.max_observations {
            self.observations.pop_front();
        }

        self.metric_windows
            .insert(metric_name, self.observations.len());
    }

    /// Get observations for a specific metric
    pub fn get_observations(&self, metric_name: &str) -> Vec<&MetricObservation> {
        self.observations
            .iter()
            .filter(|o| o.metric_name == metric_name)
            .collect()
    }

    /// Get observations within time window
    pub fn get_observations_in_window(
        &self,
        metric_name: &str,
        window_secs: u64,
    ) -> Vec<&MetricObservation> {
        let now = current_timestamp_secs();
        let cutoff = now.saturating_sub(window_secs);

        self.observations
            .iter()
            .filter(|o| o.metric_name == metric_name && o.timestamp_secs >= cutoff)
            .collect()
    }

    /// Clear all observations
    pub fn clear(&mut self) {
        self.observations.clear();
        self.metric_windows.clear();
    }

    /// Get total observations count
    pub fn observation_count(&self) -> usize {
        self.observations.len()
    }
}

/// Trend analyzer
pub struct TrendAnalyzer;

impl TrendAnalyzer {
    /// Detect trend in historical data
    pub fn detect_trend(observations: &[&MetricObservation]) -> Option<TrendAnalysis> {
        if observations.len() < 2 {
            return None;
        }

        let metric_name = observations[0].metric_name.clone();
        let first = observations[0];
        let last = observations[observations.len() - 1];

        let duration_secs = if last.timestamp_secs > first.timestamp_secs {
            last.timestamp_secs - first.timestamp_secs
        } else {
            1
        };

        let value_change = last.value - first.value;
        let slope = value_change / duration_secs as f64;

        // Calculate confidence based on consistency
        let confidence = Self::calculate_trend_confidence(observations);

        // Determine direction
        let direction = if slope.abs() < 0.001 {
            TrendDirection::Stable
        } else if slope > 0.0 {
            TrendDirection::Increasing
        } else {
            TrendDirection::Decreasing
        };

        Some(TrendAnalysis {
            metric_name,
            direction,
            slope,
            confidence,
            duration_secs,
            start_value: first.value,
            end_value: last.value,
        })
    }

    /// Detect anomalies using statistical methods
    pub fn detect_anomalies(observations: &[&MetricObservation]) -> Vec<Anomaly> {
        if observations.len() < 3 {
            return Vec::new();
        }

        let mut anomalies = Vec::new();
        let mean = Self::calculate_mean(observations);
        let stddev = Self::calculate_stddev(observations, mean);
        let metric_name = observations[0].metric_name.clone();

        for obs in observations {
            let z_score = (obs.value - mean).abs() / (stddev + 0.001);

            if z_score > 3.0 {
                // Severe: > 3 sigma
                anomalies.push(Anomaly {
                    metric_name: metric_name.clone(),
                    timestamp_secs: obs.timestamp_secs,
                    value: obs.value,
                    expected_value: mean,
                    severity: AnomalySeverity::Severe,
                    description: format!(
                        "Severe anomaly: value {} is {:.2} standard deviations from mean",
                        obs.value, z_score
                    ),
                });
            } else if z_score > 2.0 {
                // Moderate: 2-3 sigma
                anomalies.push(Anomaly {
                    metric_name: metric_name.clone(),
                    timestamp_secs: obs.timestamp_secs,
                    value: obs.value,
                    expected_value: mean,
                    severity: AnomalySeverity::Moderate,
                    description: format!(
                        "Moderate anomaly: value {} is {:.2} standard deviations from mean",
                        obs.value, z_score
                    ),
                });
            } else if z_score > 1.5 {
                // Minor: 1.5-2 sigma
                anomalies.push(Anomaly {
                    metric_name: metric_name.clone(),
                    timestamp_secs: obs.timestamp_secs,
                    value: obs.value,
                    expected_value: mean,
                    severity: AnomalySeverity::Minor,
                    description: format!(
                        "Minor anomaly: value {} is {:.2} standard deviations from mean",
                        obs.value, z_score
                    ),
                });
            }
        }

        anomalies
    }

    /// Calculate trend confidence
    fn calculate_trend_confidence(observations: &[&MetricObservation]) -> f64 {
        if observations.len() < 2 {
            return 0.0;
        }

        // Simple confidence based on monotonicity
        let mut direction_changes = 0;
        for i in 1..observations.len() - 1 {
            let prev_change = observations[i].value - observations[i - 1].value;
            let next_change = observations[i + 1].value - observations[i].value;

            if (prev_change > 0.0) != (next_change > 0.0) {
                direction_changes += 1;
            }
        }

        let total_comparisons = (observations.len() - 2).max(1);
        let consistency = 1.0 - (direction_changes as f64 / total_comparisons as f64);

        // Clamp between 0 and 1
        consistency.clamp(0.0, 1.0)
    }

    /// Calculate mean value
    fn calculate_mean(observations: &[&MetricObservation]) -> f64 {
        if observations.is_empty() {
            return 0.0;
        }

        let sum: f64 = observations.iter().map(|o| o.value).sum();
        sum / observations.len() as f64
    }

    /// Calculate standard deviation
    fn calculate_stddev(observations: &[&MetricObservation], mean: f64) -> f64 {
        if observations.len() < 2 {
            return 0.0;
        }

        let variance: f64 = observations
            .iter()
            .map(|o| (o.value - mean).powi(2))
            .sum::<f64>()
            / (observations.len() - 1) as f64;

        variance.sqrt()
    }

    /// Detect seasonality pattern (simplified)
    pub fn detect_seasonality(observations: &[&MetricObservation]) -> Option<SeasonalPattern> {
        if observations.len() < 24 {
            // Need at least 24 samples to detect patterns
            return None;
        }

        let metric_name = observations[0].metric_name.clone();

        // Simplified: assume period of roughly 1/3 of observation window
        let suggested_period = (observations.len() / 3).max(2) as u64;

        // Calculate amplitude as difference between max and min
        let max_val = observations
            .iter()
            .map(|o| o.value)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_val = observations
            .iter()
            .map(|o| o.value)
            .fold(f64::INFINITY, f64::min);

        let amplitude = (max_val - min_val) / 2.0;
        let offset = min_val + amplitude;
        let mean = (max_val + min_val) / 2.0;

        // Confidence based on amplitude relative to mean
        let confidence = if mean > 0.0 {
            (amplitude / mean).min(1.0)
        } else {
            0.0
        };

        if amplitude > 0.0 && confidence > 0.1 {
            Some(SeasonalPattern {
                metric_name,
                period_secs: suggested_period,
                amplitude,
                offset,
                confidence,
            })
        } else {
            None
        }
    }
}

/// Predictive scorer for capacity planning
pub struct PredictiveScorer;

impl PredictiveScorer {
    /// Predict future health based on trends
    /// Returns score 0-100, where 100 is optimal
    pub fn predict_health_score(
        current_metrics: &WarmRestartMetrics,
        trends: &[TrendAnalysis],
    ) -> f64 {
        let mut base_score = current_metrics.health_score();

        // Adjust based on trends
        for trend in trends {
            match trend.metric_name.as_str() {
                // Negative trends for these metrics
                "corruption_detected_count" | "eoiu_timeout_count" | "cold_start_count" => {
                    if trend.direction == TrendDirection::Increasing && trend.confidence > 0.7 {
                        base_score *= 0.9; // Penalize increasing bad metrics
                    }
                }
                // Positive trends for these metrics
                "warm_restart_count" | "state_recovery_count" | "health_score" => {
                    if trend.direction == TrendDirection::Decreasing && trend.confidence > 0.7 {
                        base_score *= 0.95; // Penalize decreasing good metrics
                    }
                }
                _ => {}
            }
        }

        base_score.clamp(0.0, 100.0)
    }

    /// Predict recovery success based on historical performance
    pub fn predict_recovery_rate(recovery_history: &[&MetricObservation]) -> f64 {
        if recovery_history.is_empty() {
            return 50.0; // Default to 50% if no history
        }

        let successful = recovery_history.iter().filter(|o| o.value > 0.0).count();
        let rate = (successful as f64 / recovery_history.len() as f64) * 100.0;

        rate.clamp(0.0, 100.0)
    }

    /// Estimate time to degrade (when health will drop below threshold)
    pub fn estimate_time_to_degrade(
        current_metrics: &WarmRestartMetrics,
        trend: &TrendAnalysis,
        threshold: f64,
    ) -> Option<u64> {
        let current_score = current_metrics.health_score();

        if current_score <= threshold {
            return Some(0);
        }

        if trend.slope >= 0.0 {
            // Not degrading
            return None;
        }

        let decline_needed = current_score - threshold;
        let time_to_degrade = (decline_needed / trend.slope.abs()) as u64;

        Some(time_to_degrade)
    }
}

/// Get current Unix timestamp in seconds
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_observations() -> Vec<MetricObservation> {
        vec![
            MetricObservation {
                timestamp_secs: 1000,
                metric_name: "test_metric".to_string(),
                value: 10.0,
            },
            MetricObservation {
                timestamp_secs: 1060,
                metric_name: "test_metric".to_string(),
                value: 12.0,
            },
            MetricObservation {
                timestamp_secs: 1120,
                metric_name: "test_metric".to_string(),
                value: 14.0,
            },
            MetricObservation {
                timestamp_secs: 1180,
                metric_name: "test_metric".to_string(),
                value: 16.0,
            },
            MetricObservation {
                timestamp_secs: 1240,
                metric_name: "test_metric".to_string(),
                value: 18.0,
            },
        ]
    }

    #[test]
    fn test_historical_metrics_add_observation() {
        let mut history = HistoricalMetrics::new(100);
        history.add_observation("test_metric".to_string(), 10.0);
        history.add_observation("test_metric".to_string(), 20.0);

        assert_eq!(history.observation_count(), 2);
    }

    #[test]
    fn test_historical_metrics_max_size() {
        let mut history = HistoricalMetrics::new(3);
        for i in 0..5 {
            history.add_observation("test_metric".to_string(), i as f64);
        }

        assert_eq!(history.observation_count(), 3);
    }

    #[test]
    fn test_historical_metrics_get_observations() {
        let mut history = HistoricalMetrics::new(100);
        history.add_observation("metric1".to_string(), 10.0);
        history.add_observation("metric2".to_string(), 20.0);
        history.add_observation("metric1".to_string(), 30.0);

        let metric1_obs = history.get_observations("metric1");
        assert_eq!(metric1_obs.len(), 2);
    }

    #[test]
    fn test_historical_metrics_clear() {
        let mut history = HistoricalMetrics::new(100);
        history.add_observation("metric1".to_string(), 10.0);
        history.clear();

        assert_eq!(history.observation_count(), 0);
    }

    #[test]
    fn test_trend_detection_increasing() {
        let observations = create_test_observations();
        let refs: Vec<_> = observations.iter().collect();

        let trend = TrendAnalyzer::detect_trend(&refs).unwrap();

        assert_eq!(trend.direction, TrendDirection::Increasing);
        assert!(trend.slope > 0.0);
        assert!(trend.confidence > 0.5);
    }

    #[test]
    fn test_trend_detection_decreasing() {
        let observations = [
            MetricObservation {
                timestamp_secs: 1000,
                metric_name: "test_metric".to_string(),
                value: 20.0,
            },
            MetricObservation {
                timestamp_secs: 1060,
                metric_name: "test_metric".to_string(),
                value: 18.0,
            },
            MetricObservation {
                timestamp_secs: 1120,
                metric_name: "test_metric".to_string(),
                value: 16.0,
            },
            MetricObservation {
                timestamp_secs: 1180,
                metric_name: "test_metric".to_string(),
                value: 14.0,
            },
            MetricObservation {
                timestamp_secs: 1240,
                metric_name: "test_metric".to_string(),
                value: 12.0,
            },
        ];
        let refs: Vec<_> = observations.iter().collect();

        let trend = TrendAnalyzer::detect_trend(&refs).unwrap();

        assert_eq!(trend.direction, TrendDirection::Decreasing);
        assert!(trend.slope < 0.0);
    }

    #[test]
    fn test_trend_detection_stable() {
        let observations = [
            MetricObservation {
                timestamp_secs: 1000,
                metric_name: "test_metric".to_string(),
                value: 10.0,
            },
            MetricObservation {
                timestamp_secs: 1060,
                metric_name: "test_metric".to_string(),
                value: 10.1,
            },
            MetricObservation {
                timestamp_secs: 1120,
                metric_name: "test_metric".to_string(),
                value: 10.0,
            },
        ];
        let refs: Vec<_> = observations.iter().collect();

        let trend = TrendAnalyzer::detect_trend(&refs).unwrap();

        assert_eq!(trend.direction, TrendDirection::Stable);
    }

    #[test]
    fn test_anomaly_detection() {
        let observations = vec![
            MetricObservation {
                timestamp_secs: 1000,
                metric_name: "test".to_string(),
                value: 1.0,
            },
            MetricObservation {
                timestamp_secs: 1060,
                metric_name: "test".to_string(),
                value: 1.0,
            },
            MetricObservation {
                timestamp_secs: 1120,
                metric_name: "test".to_string(),
                value: 1.0,
            },
            MetricObservation {
                timestamp_secs: 1180,
                metric_name: "test".to_string(),
                value: 1.0,
            },
            MetricObservation {
                timestamp_secs: 1240,
                metric_name: "test".to_string(),
                value: 50.0, // Extreme anomaly
            },
        ];
        let refs: Vec<_> = observations.iter().collect();

        let anomalies = TrendAnalyzer::detect_anomalies(&refs);

        assert!(!anomalies.is_empty(), "Should detect anomalies");
    }

    #[test]
    fn test_seasonality_detection() {
        let mut observations = Vec::new();
        for i in 0..30 {
            observations.push(MetricObservation {
                timestamp_secs: (i * 60) as u64,
                metric_name: "test".to_string(),
                value: 50.0 + 20.0 * ((i as f64 * 0.2).sin()),
            });
        }
        let refs: Vec<_> = observations.iter().collect();

        let pattern = TrendAnalyzer::detect_seasonality(&refs);

        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert!(p.amplitude > 0.0);
        assert!(p.confidence > 0.0);
    }

    #[test]
    fn test_trend_confidence_calculation() {
        let observations = create_test_observations();
        let refs: Vec<_> = observations.iter().collect();

        let confidence = TrendAnalyzer::calculate_trend_confidence(&refs);

        assert!((0.0..=1.0).contains(&confidence));
        assert!(confidence > 0.8); // High confidence for monotonic data
    }

    #[test]
    fn test_mean_calculation() {
        let observations = [
            MetricObservation {
                timestamp_secs: 1000,
                metric_name: "test".to_string(),
                value: 10.0,
            },
            MetricObservation {
                timestamp_secs: 1060,
                metric_name: "test".to_string(),
                value: 20.0,
            },
        ];
        let refs: Vec<_> = observations.iter().collect();

        let mean = TrendAnalyzer::calculate_mean(&refs);

        assert_eq!(mean, 15.0);
    }

    #[test]
    fn test_stddev_calculation() {
        let observations = [
            MetricObservation {
                timestamp_secs: 1000,
                metric_name: "test".to_string(),
                value: 10.0,
            },
            MetricObservation {
                timestamp_secs: 1060,
                metric_name: "test".to_string(),
                value: 20.0,
            },
        ];
        let refs: Vec<_> = observations.iter().collect();

        let mean = TrendAnalyzer::calculate_mean(&refs);
        let stddev = TrendAnalyzer::calculate_stddev(&refs, mean);

        assert!(stddev > 0.0);
    }

    #[test]
    fn test_predict_health_score_with_positive_trend() {
        let metrics = WarmRestartMetrics {
            warm_restart_count: 100,
            cold_start_count: 5,
            eoiu_detected_count: 100,
            eoiu_timeout_count: 5,
            state_recovery_count: 95,
            corruption_detected_count: 0,
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

        let trend = TrendAnalysis {
            metric_name: "warm_restart_count".to_string(),
            direction: TrendDirection::Increasing,
            slope: 0.1,
            confidence: 0.8,
            duration_secs: 1000,
            start_value: 90.0,
            end_value: 100.0,
        };

        let predicted = PredictiveScorer::predict_health_score(&metrics, &[trend]);

        assert!(predicted > 0.0);
        assert!(predicted <= 100.0);
    }

    #[test]
    fn test_estimate_time_to_degrade() {
        let metrics = WarmRestartMetrics {
            warm_restart_count: 100,
            cold_start_count: 5,
            eoiu_detected_count: 100,
            eoiu_timeout_count: 5,
            state_recovery_count: 95,
            corruption_detected_count: 0,
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

        let trend = TrendAnalysis {
            metric_name: "health_score".to_string(),
            direction: TrendDirection::Decreasing,
            slope: -1.0,
            confidence: 0.9,
            duration_secs: 1000,
            start_value: 100.0,
            end_value: 90.0,
        };

        let time = PredictiveScorer::estimate_time_to_degrade(&metrics, &trend, 50.0);

        assert!(time.is_some());
        assert!(time.unwrap() > 0);
    }

    #[test]
    fn test_recovery_rate_prediction() {
        let observations = [
            MetricObservation {
                timestamp_secs: 1000,
                metric_name: "recovery".to_string(),
                value: 1.0,
            },
            MetricObservation {
                timestamp_secs: 1060,
                metric_name: "recovery".to_string(),
                value: 1.0,
            },
            MetricObservation {
                timestamp_secs: 1120,
                metric_name: "recovery".to_string(),
                value: 0.0,
            },
        ];
        let refs: Vec<_> = observations.iter().collect();

        let rate = PredictiveScorer::predict_recovery_rate(&refs);

        assert!((0.0..=100.0).contains(&rate));
        assert_eq!(rate, 66.66666666666666);
    }
}
