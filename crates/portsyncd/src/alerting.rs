//! Alerting rule engine for portsyncd metrics monitoring
//!
//! Provides threshold-based alerting for metrics with state management,
//! evaluation windows, and action handling.
//!
//! Features:
//! - Threshold-based alert conditions (Above, Below, Between, Equals, RateOfChange)
//! - Alert severity levels (Critical, Warning, Info)
//! - State tracking (Pending, Firing, Resolved, Suppressed)
//! - Evaluation windows with for_duration (prevents flapping)
//! - Configurable actions (log, notify, webhook)
//! - Default rule templates for common scenarios
//!
//! Phase 6 Week 5 implementation.

use crate::warm_restart::WarmRestartMetrics;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Alert condition types for rule evaluation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertCondition {
    /// Metric value > threshold
    Above,
    /// Metric value < threshold
    Below,
    /// Metric value between min and max
    Between,
    /// Metric value == threshold
    Equals,
    /// Rate of change > threshold (e.g., restarts per minute)
    RateOfChange,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AlertSeverity {
    /// System is operational but degraded
    Info,
    /// Intervention may be required soon
    Warning,
    /// Immediate intervention required
    Critical,
}

/// Alert state tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertState {
    /// Alert condition detected but not yet sustained (pending for_duration)
    Pending,
    /// Alert condition sustained and firing
    Firing,
    /// Alert was firing, now resolved
    Resolved,
    /// Alert silenced by user action
    Suppressed,
}

/// Alert action types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AlertAction {
    /// Log to systemd journal
    Log,
    /// Send notification (systemd notify or similar)
    Notify,
    /// Send to webhook endpoint
    Webhook(String),
}

/// Represents a single metric value with timestamp for trend analysis
#[derive(Debug, Clone)]
pub struct MetricSample {
    pub value: f64,
    pub timestamp_secs: u64,
}

/// Alert rule definition
#[derive(Debug, Clone)]
pub struct AlertRule {
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    /// For Between condition: (min, max)
    pub threshold_range: Option<(f64, f64)>,
    /// Window size for evaluation in seconds
    pub evaluation_window_secs: u64,
    /// Duration the condition must be true before firing in seconds
    pub for_duration_secs: u64,
    pub enabled: bool,
    pub severity: AlertSeverity,
    pub actions: Vec<AlertAction>,
}

/// Active alert instance with state tracking
#[derive(Debug, Clone)]
pub struct Alert {
    pub rule_id: String,
    pub name: String,
    pub metric_name: String,
    pub state: AlertState,
    pub severity: AlertSeverity,
    /// When did the condition start (for pending -> firing transition)
    pub condition_start_secs: Option<u64>,
    /// When did the alert fire
    pub fired_at_secs: Option<u64>,
    /// When was the alert resolved
    pub resolved_at_secs: Option<u64>,
    /// Last value that triggered the condition
    pub metric_value: f64,
    /// Message describing the alert
    pub message: String,
}

/// Alert rule evaluator with state machine
#[derive(Debug)]
pub struct AlertingEngine {
    rules: HashMap<String, AlertRule>,
    alerts: HashMap<String, Alert>,
    /// Historical samples for rate of change calculations
    metric_history: HashMap<String, Vec<MetricSample>>,
    /// Max samples to keep per metric (for memory efficiency)
    max_history_samples: usize,
}

impl AlertingEngine {
    /// Create a new alerting engine
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            alerts: HashMap::new(),
            metric_history: HashMap::new(),
            max_history_samples: 1000,
        }
    }

    /// Add an alert rule
    pub fn add_rule(&mut self, rule: AlertRule) {
        self.rules.insert(rule.rule_id.clone(), rule);
    }

    /// Remove an alert rule
    pub fn remove_rule(&mut self, rule_id: &str) -> Option<AlertRule> {
        self.rules.remove(rule_id)
    }

    /// Enable/disable a rule
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) -> bool {
        if let Some(rule) = self.rules.get_mut(rule_id) {
            rule.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Get all rules
    pub fn rules(&self) -> &HashMap<String, AlertRule> {
        &self.rules
    }

    /// Get all active alerts
    pub fn alerts(&self) -> &HashMap<String, Alert> {
        &self.alerts
    }

    /// Get alerts by state
    pub fn alerts_by_state(&self, state: AlertState) -> Vec<&Alert> {
        self.alerts.values().filter(|a| a.state == state).collect()
    }

    /// Get alerts by severity
    pub fn alerts_by_severity(&self, severity: AlertSeverity) -> Vec<&Alert> {
        self.alerts
            .values()
            .filter(|a| a.severity == severity)
            .collect()
    }

    /// Evaluate all rules against current metrics
    pub fn evaluate(&mut self, metrics: &WarmRestartMetrics) -> Vec<&Alert> {
        let now = current_timestamp_secs();

        let rules_to_eval: Vec<_> = self.rules.values().cloned().collect();
        for rule in rules_to_eval {
            if !rule.enabled {
                continue;
            }

            let metric_value = self.get_metric_value(&rule, metrics);
            if metric_value.is_nan() {
                continue;
            }

            let condition_met = self.check_condition(&rule, metric_value, metrics);

            self.update_alert_state(
                rule.rule_id.clone(),
                rule.name.clone(),
                rule.metric_name.clone(),
                rule.severity,
                condition_met,
                metric_value,
                now,
                rule.for_duration_secs,
            );
        }

        self.alerts.values().collect()
    }

    /// Suppress an alert (silence it)
    pub fn suppress_alert(&mut self, rule_id: &str) -> bool {
        if let Some(alert) = self.alerts.get_mut(rule_id) {
            if alert.state == AlertState::Firing {
                alert.state = AlertState::Suppressed;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Unsuppress an alert
    pub fn unsuppress_alert(&mut self, rule_id: &str) -> bool {
        if let Some(alert) = self.alerts.get_mut(rule_id) {
            if alert.state == AlertState::Suppressed {
                alert.state = AlertState::Firing;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    // Private helper methods

    fn get_metric_value(&self, rule: &AlertRule, metrics: &WarmRestartMetrics) -> f64 {
        match rule.metric_name.as_str() {
            "warm_restart_count" => metrics.warm_restart_count as f64,
            "cold_start_count" => metrics.cold_start_count as f64,
            "eoiu_detected_count" => metrics.eoiu_detected_count as f64,
            "eoiu_timeout_count" => metrics.eoiu_timeout_count as f64,
            "state_recovery_count" => metrics.state_recovery_count as f64,
            "corruption_detected_count" => metrics.corruption_detected_count as f64,
            "backup_created_count" => metrics.backup_created_count as f64,
            "backup_cleanup_count" => metrics.backup_cleanup_count as f64,
            "avg_initial_sync_duration_secs" => metrics.avg_initial_sync_duration_secs,
            "max_initial_sync_duration_secs" => metrics.max_initial_sync_duration_secs as f64,
            "min_initial_sync_duration_secs" => metrics.min_initial_sync_duration_secs as f64,
            "health_score" => metrics.health_score(),
            "recovery_success_rate" => metrics.recovery_success_rate(),
            "eoiu_timeout_rate" => metrics.eoiu_timeout_rate(),
            _ => f64::NAN,
        }
    }

    fn check_condition(
        &mut self,
        rule: &AlertRule,
        metric_value: f64,
        _metrics: &WarmRestartMetrics,
    ) -> bool {
        match rule.condition {
            AlertCondition::Above => metric_value > rule.threshold,
            AlertCondition::Below => metric_value < rule.threshold,
            AlertCondition::Between => {
                if let Some((min, max)) = rule.threshold_range {
                    metric_value >= min && metric_value <= max
                } else {
                    false
                }
            }
            AlertCondition::Equals => (metric_value - rule.threshold).abs() < 0.001,
            AlertCondition::RateOfChange => self.check_rate_of_change(rule, metric_value),
        }
    }

    fn check_rate_of_change(&mut self, rule: &AlertRule, metric_value: f64) -> bool {
        let now = current_timestamp_secs();
        let metric_name = &rule.metric_name;

        // Initialize history if needed
        self.metric_history.entry(metric_name.clone()).or_default();

        let history = self.metric_history.get_mut(metric_name).unwrap();
        history.push(MetricSample {
            value: metric_value,
            timestamp_secs: now,
        });

        // Keep only recent samples within evaluation window
        let cutoff_time = now.saturating_sub(rule.evaluation_window_secs);
        history.retain(|s| s.timestamp_secs >= cutoff_time);

        // Trim to max size
        if history.len() > self.max_history_samples {
            history.remove(0);
        }

        // Need at least 2 samples to calculate rate
        if history.len() < 2 {
            return false;
        }

        let first = &history[0];
        let last = &history[history.len() - 1];

        let time_delta = (last.timestamp_secs - first.timestamp_secs) as f64;
        if time_delta < 1.0 {
            return false;
        }

        let value_delta = last.value - first.value;
        let rate = value_delta / time_delta;

        rate > rule.threshold
    }

    #[allow(clippy::too_many_arguments)]
    fn update_alert_state(
        &mut self,
        rule_id: String,
        name: String,
        metric_name: String,
        severity: AlertSeverity,
        condition_met: bool,
        metric_value: f64,
        now: u64,
        for_duration_secs: u64,
    ) {
        // Only create alert if condition is met (unless alert already exists)
        if !condition_met && !self.alerts.contains_key(&rule_id) {
            return;
        }

        let alert = self.alerts.entry(rule_id.clone()).or_insert_with(|| Alert {
            rule_id: rule_id.clone(),
            name: name.clone(),
            metric_name: metric_name.clone(),
            state: AlertState::Pending,
            severity,
            condition_start_secs: None,
            fired_at_secs: None,
            resolved_at_secs: None,
            metric_value: 0.0,
            message: String::new(),
        });

        alert.metric_value = metric_value;

        match alert.state {
            AlertState::Pending => {
                if condition_met {
                    if alert.condition_start_secs.is_none() {
                        alert.condition_start_secs = Some(now);
                    }

                    let duration_so_far =
                        now.saturating_sub(alert.condition_start_secs.unwrap_or(now));
                    if duration_so_far >= for_duration_secs {
                        alert.state = AlertState::Firing;
                        alert.fired_at_secs = Some(now);
                        alert.message =
                            format!("{}: {} is {}", alert.name, metric_name, metric_value);
                    }
                } else {
                    alert.condition_start_secs = None;
                }
            }
            AlertState::Firing => {
                if !condition_met {
                    alert.state = AlertState::Resolved;
                    alert.resolved_at_secs = Some(now);
                    alert.message = format!(
                        "{}: {} resolved (value: {})",
                        alert.name, metric_name, metric_value
                    );
                } else {
                    alert.message = format!("{}: {} is {}", alert.name, metric_name, metric_value);
                }
            }
            AlertState::Suppressed => {
                if !condition_met {
                    alert.state = AlertState::Resolved;
                    alert.resolved_at_secs = Some(now);
                    alert.message = format!(
                        "{}: {} resolved (value: {})",
                        alert.name, metric_name, metric_value
                    );
                }
            }
            AlertState::Resolved => {
                if condition_met {
                    alert.state = AlertState::Pending;
                    alert.condition_start_secs = Some(now);
                    alert.message = format!(
                        "{}: {} recurred (value: {})",
                        alert.name, metric_name, metric_value
                    );
                }
            }
        }
    }
}

impl Default for AlertingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Create default alert rules for common portsyncd scenarios
pub fn create_default_alert_rules() -> Vec<AlertRule> {
    vec![
        // Rule 1: High EOIU timeout rate (>50%)
        AlertRule {
            rule_id: "eoiu_timeout_high".to_string(),
            name: "High EOIU Timeout Rate".to_string(),
            description: "EOIU signals are not completing normally (>50% timeout rate)".to_string(),
            metric_name: "eoiu_timeout_rate".to_string(),
            condition: AlertCondition::Above,
            threshold: 50.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 60,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![AlertAction::Log, AlertAction::Notify],
        },
        // Rule 2: Corruption detected but not recovered
        AlertRule {
            rule_id: "corruption_unrecovered".to_string(),
            name: "Unrecovered State Corruption".to_string(),
            description: "State corruption events exceed successful recoveries".to_string(),
            metric_name: "corruption_detected_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 5.0,
            threshold_range: None,
            evaluation_window_secs: 600,
            for_duration_secs: 120,
            enabled: true,
            severity: AlertSeverity::Critical,
            actions: vec![AlertAction::Log, AlertAction::Notify],
        },
        // Rule 3: High cold start rate (anomaly)
        AlertRule {
            rule_id: "cold_start_anomaly".to_string(),
            name: "Cold Start Anomaly".to_string(),
            description: "Excessive cold starts detected (>50% of recent events)".to_string(),
            metric_name: "cold_start_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 10.0,
            threshold_range: None,
            evaluation_window_secs: 900,
            for_duration_secs: 180,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![AlertAction::Log, AlertAction::Notify],
        },
        // Rule 4: Sync duration exceeds threshold
        AlertRule {
            rule_id: "sync_duration_slow".to_string(),
            name: "Slow Initial Sync".to_string(),
            description: "Initial sync duration exceeds 30 seconds".to_string(),
            metric_name: "avg_initial_sync_duration_secs".to_string(),
            condition: AlertCondition::Above,
            threshold: 30.0,
            threshold_range: None,
            evaluation_window_secs: 600,
            for_duration_secs: 300,
            enabled: true,
            severity: AlertSeverity::Info,
            actions: vec![AlertAction::Log],
        },
        // Rule 5: System health score degraded
        AlertRule {
            rule_id: "health_score_low".to_string(),
            name: "Low System Health Score".to_string(),
            description: "System health score below 50%".to_string(),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Below,
            threshold: 50.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 60,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![AlertAction::Log, AlertAction::Notify],
        },
        // Rule 6: Recovery success rate too low
        AlertRule {
            rule_id: "recovery_rate_low".to_string(),
            name: "Low Recovery Success Rate".to_string(),
            description: "Corruption recovery success rate below 75%".to_string(),
            metric_name: "recovery_success_rate".to_string(),
            condition: AlertCondition::Below,
            threshold: 75.0,
            threshold_range: None,
            evaluation_window_secs: 600,
            for_duration_secs: 120,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![AlertAction::Log, AlertAction::Notify],
        },
        // Rule 7: Backup creation failures
        AlertRule {
            rule_id: "backup_creation_low".to_string(),
            name: "Low Backup Creation Rate".to_string(),
            description: "Backup files not being created as expected".to_string(),
            metric_name: "backup_created_count".to_string(),
            condition: AlertCondition::Below,
            threshold: 1.0,
            threshold_range: None,
            evaluation_window_secs: 1800,
            for_duration_secs: 300,
            enabled: true,
            severity: AlertSeverity::Info,
            actions: vec![AlertAction::Log],
        },
        // Rule 8: Max sync duration exceeds hard limit
        AlertRule {
            rule_id: "sync_duration_critical".to_string(),
            name: "Critical Sync Duration".to_string(),
            description: "Maximum sync duration exceeds 300 seconds".to_string(),
            metric_name: "max_initial_sync_duration_secs".to_string(),
            condition: AlertCondition::Above,
            threshold: 300.0,
            threshold_range: None,
            evaluation_window_secs: 600,
            for_duration_secs: 60,
            enabled: true,
            severity: AlertSeverity::Critical,
            actions: vec![AlertAction::Log, AlertAction::Notify],
        },
        // Rule 9: Multiple warm restarts in short period (rate of change)
        AlertRule {
            rule_id: "restart_rate_high".to_string(),
            name: "High Restart Rate".to_string(),
            description: "Warm restarts occurring at high rate (>0.5 per second)".to_string(),
            metric_name: "warm_restart_count".to_string(),
            condition: AlertCondition::RateOfChange,
            threshold: 0.5,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 60,
            enabled: true,
            severity: AlertSeverity::Critical,
            actions: vec![AlertAction::Log, AlertAction::Notify],
        },
        // Rule 10: Healthy state maintained
        AlertRule {
            rule_id: "health_score_good".to_string(),
            name: "Excellent System Health".to_string(),
            description: "System health score maintained above 85%".to_string(),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Above,
            threshold: 85.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 300,
            enabled: false, // Informational only
            severity: AlertSeverity::Info,
            actions: vec![AlertAction::Log],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warm_restart::WarmRestartMetrics;

    fn create_test_metrics() -> WarmRestartMetrics {
        WarmRestartMetrics {
            warm_restart_count: 100,
            cold_start_count: 10,
            eoiu_detected_count: 100,
            eoiu_timeout_count: 30,
            state_recovery_count: 95,
            corruption_detected_count: 5,
            backup_created_count: 100,
            backup_cleanup_count: 50,
            last_warm_restart_secs: Some(current_timestamp_secs()),
            last_eoiu_detection_secs: None,
            last_state_recovery_secs: None,
            last_corruption_detected_secs: None,
            avg_initial_sync_duration_secs: 5.5,
            max_initial_sync_duration_secs: 15,
            min_initial_sync_duration_secs: 2,
        }
    }

    #[test]
    fn test_alert_condition_above() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_above".to_string(),
            name: "Test Above".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
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

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let alerts = engine.alerts_by_severity(AlertSeverity::Warning);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].state, AlertState::Firing);
    }

    #[test]
    fn test_alert_condition_below() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_below".to_string(),
            name: "Test Below".to_string(),
            description: "Test".to_string(),
            metric_name: "recovery_success_rate".to_string(),
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

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let alerts = engine.alerts_by_severity(AlertSeverity::Critical);
        assert_eq!(alerts.len(), 0); // Condition not met
    }

    #[test]
    fn test_alert_condition_between() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_between".to_string(),
            name: "Test Between".to_string(),
            description: "Test".to_string(),
            metric_name: "health_score".to_string(),
            condition: AlertCondition::Between,
            threshold: 0.0,
            threshold_range: Some((70.0, 100.0)), // Health score is high for test metrics
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Info,
            actions: vec![],
        };
        engine.add_rule(rule);

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let alerts: Vec<_> = engine.alerts().values().cloned().collect();
        assert!(!alerts.is_empty());
    }

    #[test]
    fn test_alert_pending_state() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_pending".to_string(),
            name: "Test Pending".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 5.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 300, // Require 300 seconds
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };
        engine.add_rule(rule);

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let alerts = engine.alerts_by_state(AlertState::Pending);
        assert_eq!(alerts.len(), 1);
    }

    #[test]
    fn test_alert_firing_after_duration() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_firing".to_string(),
            name: "Test Firing".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 5.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0, // Immediate fire
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };
        engine.add_rule(rule);

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let alerts = engine.alerts_by_state(AlertState::Firing);
        assert_eq!(alerts.len(), 1);
        assert!(alerts[0].fired_at_secs.is_some());
    }

    #[test]
    fn test_alert_resolution() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_resolution".to_string(),
            name: "Test Resolution".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
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

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        // Condition is met, alert fires
        assert_eq!(
            engine.alerts_by_state(AlertState::Firing).len(),
            1,
            "Alert should be firing"
        );

        // Now evaluate with metrics where condition is not met
        let mut resolved_metrics = create_test_metrics();
        resolved_metrics.cold_start_count = 1; // Below threshold
        engine.evaluate(&resolved_metrics);

        let resolved = engine.alerts_by_state(AlertState::Resolved);
        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].resolved_at_secs.is_some());
    }

    #[test]
    fn test_suppress_alert() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_suppress".to_string(),
            name: "Test Suppress".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
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

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let result = engine.suppress_alert("test_suppress");
        assert!(result, "Suppress should succeed");

        let suppressed = engine.alerts_by_state(AlertState::Suppressed);
        assert_eq!(suppressed.len(), 1);
    }

    #[test]
    fn test_unsuppress_alert() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_unsuppress".to_string(),
            name: "Test Unsuppress".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
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

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);
        engine.suppress_alert("test_unsuppress");

        let result = engine.unsuppress_alert("test_unsuppress");
        assert!(result, "Unsuppress should succeed");

        let firing = engine.alerts_by_state(AlertState::Firing);
        assert_eq!(firing.len(), 1);
    }

    #[test]
    fn test_disable_rule() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_disable".to_string(),
            name: "Test Disable".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
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

        engine.set_rule_enabled("test_disable", false);

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let alerts = engine.alerts_by_state(AlertState::Firing);
        assert_eq!(alerts.len(), 0, "Disabled rule should not fire");
    }

    #[test]
    fn test_default_alert_rules() {
        let rules = create_default_alert_rules();
        assert_eq!(rules.len(), 10, "Should have 10 default rules");

        // Verify rule uniqueness
        let rule_ids: Vec<_> = rules.iter().map(|r| &r.rule_id).collect();
        let unique_count = rule_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(unique_count, 10, "All rule IDs should be unique");

        // Verify severity distribution
        let critical_count = rules
            .iter()
            .filter(|r| r.severity == AlertSeverity::Critical)
            .count();
        let warning_count = rules
            .iter()
            .filter(|r| r.severity == AlertSeverity::Warning)
            .count();
        let info_count = rules
            .iter()
            .filter(|r| r.severity == AlertSeverity::Info)
            .count();

        assert!(critical_count > 0, "Should have critical rules");
        assert!(warning_count > 0, "Should have warning rules");
        assert!(info_count > 0, "Should have info rules");
    }

    #[test]
    fn test_alert_equals_condition() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_equals".to_string(),
            name: "Test Equals".to_string(),
            description: "Test".to_string(),
            metric_name: "backup_cleanup_count".to_string(),
            condition: AlertCondition::Equals,
            threshold: 50.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Info,
            actions: vec![],
        };
        engine.add_rule(rule);

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let alerts: Vec<_> = engine.alerts().values().collect();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].state, AlertState::Firing);
    }

    #[test]
    fn test_multiple_rules() {
        let mut engine = AlertingEngine::new();

        let rule1 = AlertRule {
            rule_id: "rule1".to_string(),
            name: "Rule 1".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
            condition: AlertCondition::Above,
            threshold: 5.0,
            threshold_range: None,
            evaluation_window_secs: 300,
            for_duration_secs: 0,
            enabled: true,
            severity: AlertSeverity::Warning,
            actions: vec![],
        };

        let rule2 = AlertRule {
            rule_id: "rule2".to_string(),
            name: "Rule 2".to_string(),
            description: "Test".to_string(),
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

        engine.add_rule(rule1);
        engine.add_rule(rule2);

        let metrics = create_test_metrics();
        engine.evaluate(&metrics);

        let all_alerts: Vec<_> = engine.alerts().values().cloned().collect();
        assert_eq!(all_alerts.len(), 1, "Only rule1 should fire");
        assert_eq!(all_alerts[0].rule_id, "rule1");
    }

    #[test]
    fn test_remove_rule() {
        let mut engine = AlertingEngine::new();
        let rule = AlertRule {
            rule_id: "test_remove".to_string(),
            name: "Test Remove".to_string(),
            description: "Test".to_string(),
            metric_name: "cold_start_count".to_string(),
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

        let result = engine.remove_rule("test_remove");
        assert!(result.is_some(), "Remove should succeed");

        let rules = engine.rules();
        assert_eq!(rules.len(), 0);
    }
}
