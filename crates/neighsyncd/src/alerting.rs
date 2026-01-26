//! Threshold-based alerting engine for neighsyncd
//!
//! Monitors metrics and generates alerts based on configurable thresholds.
//! Implements alert state tracking and severity levels for production monitoring.
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - AU-12: Audit Record Generation - Alert events are logged
//! - SI-4: System Monitoring - Continuous threshold-based monitoring
//! - IR-4: Incident Handling - Alerts triggered on anomalies

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    /// Informational - normal operation
    Info,
    /// Warning - degraded performance or concerning trends
    Warning,
    /// Critical - immediate action required
    Critical,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Info => write!(f, "info"),
            AlertSeverity::Warning => write!(f, "warning"),
            AlertSeverity::Critical => write!(f, "critical"),
        }
    }
}

/// Alert state transitions: None → Pending → Firing → Resolved → None
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertState {
    /// Alert condition not met
    None,
    /// Condition detected but within grace period
    Pending,
    /// Condition met and threshold exceeded - alert is active
    Firing,
    /// Condition resolved but waiting for confirmation period
    Resolved,
}

/// Threshold comparison types
#[derive(Debug, Clone)]
pub enum AlertThreshold {
    /// Metric > value
    Above(f64),
    /// Metric < value
    Below(f64),
    /// value1 < Metric < value2
    Between { min: f64, max: f64 },
    /// Rate of change exceeds threshold
    RateOfChange { threshold: f64, window_secs: u64 },
}

impl AlertThreshold {
    /// Check if value triggers the threshold
    pub fn triggered(&self, value: f64, previous_value: Option<f64>, _time_delta: u64) -> bool {
        match self {
            AlertThreshold::Above(threshold) => value > *threshold,
            AlertThreshold::Below(threshold) => value < *threshold,
            AlertThreshold::Between { min, max } => value > *min && value < *max,
            AlertThreshold::RateOfChange { threshold, .. } => {
                if let Some(prev) = previous_value {
                    let rate = (value - prev).abs();
                    rate > *threshold
                } else {
                    false
                }
            }
        }
    }
}

/// Alert definition with configurable thresholds and grace periods
#[derive(Debug, Clone)]
pub struct Alert {
    /// Alert identifier
    pub name: String,
    /// Alert description
    pub description: String,
    /// Severity level
    pub severity: AlertSeverity,
    /// Trigger threshold
    pub threshold: AlertThreshold,
    /// Seconds to wait before transitioning to Firing state
    pub grace_period_secs: u64,
    /// Seconds to wait after condition clears before resolving
    pub resolve_period_secs: u64,
}

impl Alert {
    /// Create new alert
    pub fn new(
        name: String,
        description: String,
        severity: AlertSeverity,
        threshold: AlertThreshold,
    ) -> Self {
        Self {
            name,
            description,
            severity,
            threshold,
            grace_period_secs: 120,   // 2 minutes default grace period
            resolve_period_secs: 300, // 5 minutes default resolve period
        }
    }

    /// Set grace period (time before alert fires)
    pub fn with_grace_period(mut self, secs: u64) -> Self {
        self.grace_period_secs = secs;
        self
    }

    /// Set resolve period (time after condition clears)
    pub fn with_resolve_period(mut self, secs: u64) -> Self {
        self.resolve_period_secs = secs;
        self
    }
}

/// Alert instance with current state and metadata
#[derive(Debug, Clone)]
struct AlertInstance {
    /// Current state
    state: AlertState,
    /// When state changed
    state_changed_at: u64,
    /// Last metric value
    last_value: f64,
    /// Previous metric value (for rate of change)
    previous_value: Option<f64>,
    /// Last update timestamp
    last_update: u64,
}

/// Alerting engine - monitors metrics and generates alerts
///
/// # NIST Controls
/// - SI-4: Continuously monitor for anomalies
/// - AU-12: Generate audit records for alert events
pub struct AlertingEngine {
    /// Registered alerts
    alerts: HashMap<String, Alert>,
    /// Alert instances with state tracking
    instances: HashMap<String, AlertInstance>,
}

impl AlertingEngine {
    /// Create new alerting engine
    pub fn new() -> Self {
        Self {
            alerts: HashMap::new(),
            instances: HashMap::new(),
        }
    }

    /// Register an alert
    pub fn register_alert(&mut self, alert: Alert) {
        let name = alert.name.clone();
        self.alerts.insert(name.clone(), alert);
        self.instances.insert(
            name,
            AlertInstance {
                state: AlertState::None,
                state_changed_at: current_timestamp(),
                last_value: 0.0,
                previous_value: None,
                last_update: current_timestamp(),
            },
        );
    }

    /// Update metric and check alert conditions
    ///
    /// Returns alerts that transitioned to Firing or Resolved states
    pub fn update_metric(&mut self, alert_name: &str, value: f64) -> Vec<AlertEvent> {
        let mut events = Vec::new();
        let now = current_timestamp();

        if let Some(alert) = self.alerts.get(alert_name) {
            if let Some(instance) = self.instances.get_mut(alert_name) {
                let time_delta = now - instance.last_update;
                let triggered =
                    alert
                        .threshold
                        .triggered(value, instance.previous_value, time_delta);

                let old_state = instance.state;

                // Update values
                instance.previous_value = Some(instance.last_value);
                instance.last_value = value;
                instance.last_update = now;

                // State machine
                match (old_state, triggered) {
                    // None → Pending (or directly to Firing if grace_period is 0)
                    (AlertState::None, true) => {
                        if alert.grace_period_secs == 0 {
                            // Fire immediately with zero grace period
                            instance.state = AlertState::Firing;
                            instance.state_changed_at = now;

                            let event = AlertEvent {
                                alert_name: alert_name.to_string(),
                                severity: alert.severity,
                                state: AlertState::Firing,
                                message: format!("{} - value: {:.2}", alert.description, value),
                                timestamp: now,
                            };

                            match alert.severity {
                                AlertSeverity::Critical => error!("{:?}", event),
                                AlertSeverity::Warning => warn!("{:?}", event),
                                AlertSeverity::Info => info!("{:?}", event),
                            }

                            events.push(event);
                        } else {
                            instance.state = AlertState::Pending;
                            instance.state_changed_at = now;
                            info!(
                                alert_name = alert_name,
                                value = value,
                                "Alert condition detected: {}",
                                alert.description
                            );
                        }
                    }
                    // Pending → Firing when grace period expires
                    (AlertState::Pending, true)
                        if now - instance.state_changed_at >= alert.grace_period_secs =>
                    {
                        instance.state = AlertState::Firing;
                        instance.state_changed_at = now;

                        let event = AlertEvent {
                            alert_name: alert_name.to_string(),
                            severity: alert.severity,
                            state: AlertState::Firing,
                            message: format!("{} - value: {:.2}", alert.description, value),
                            timestamp: now,
                        };

                        match alert.severity {
                            AlertSeverity::Critical => error!("{:?}", event),
                            AlertSeverity::Warning => warn!("{:?}", event),
                            AlertSeverity::Info => info!("{:?}", event),
                        }

                        events.push(event);
                    }
                    // Pending → None when condition clears
                    (AlertState::Pending, false) => {
                        instance.state = AlertState::None;
                        instance.state_changed_at = now;
                        info!(
                            alert_name = alert_name,
                            value = value,
                            "Alert condition resolved: {}",
                            alert.description
                        );
                    }
                    // Firing → Resolved (or directly to None if resolve_period is 0)
                    (AlertState::Firing, false) => {
                        if alert.resolve_period_secs == 0 {
                            // Resolve immediately with zero resolve period
                            instance.state = AlertState::None;
                            instance.state_changed_at = now;

                            let event = AlertEvent {
                                alert_name: alert_name.to_string(),
                                severity: alert.severity,
                                state: AlertState::Resolved,
                                message: format!("{} - resolved", alert.description),
                                timestamp: now,
                            };

                            info!("{:?}", event);
                            events.push(event);
                        } else {
                            instance.state = AlertState::Resolved;
                            instance.state_changed_at = now;
                        }
                    }
                    // Resolved → None when resolve period expires
                    (AlertState::Resolved, false)
                        if now - instance.state_changed_at >= alert.resolve_period_secs =>
                    {
                        instance.state = AlertState::None;
                        instance.state_changed_at = now;

                        let event = AlertEvent {
                            alert_name: alert_name.to_string(),
                            severity: alert.severity,
                            state: AlertState::Resolved,
                            message: format!("{} - resolved", alert.description),
                            timestamp: now,
                        };

                        info!("{:?}", event);
                        events.push(event);
                    }
                    // All other transitions - no action
                    _ => {}
                }
            }
        }

        events
    }

    /// Get current state of an alert
    pub fn get_alert_state(&self, alert_name: &str) -> Option<AlertState> {
        self.instances.get(alert_name).map(|i| i.state)
    }

    /// Get all currently firing alerts
    pub fn get_firing_alerts(&self) -> Vec<String> {
        self.instances
            .iter()
            .filter_map(|(name, instance)| {
                if instance.state == AlertState::Firing {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get count of firing alerts
    pub fn firing_count(&self) -> usize {
        self.instances
            .iter()
            .filter(|(_, i)| i.state == AlertState::Firing)
            .count()
    }
}

impl Default for AlertingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Alert event - emitted when alert state changes
#[derive(Debug, Clone)]
pub struct AlertEvent {
    /// Alert name
    pub alert_name: String,
    /// Severity level
    pub severity: AlertSeverity,
    /// New state
    pub state: AlertState,
    /// Event message
    pub message: String,
    /// Event timestamp
    pub timestamp: u64,
}

/// Helper to get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_severity_ordering() {
        assert!(AlertSeverity::Info < AlertSeverity::Warning);
        assert!(AlertSeverity::Warning < AlertSeverity::Critical);
    }

    #[test]
    fn test_alert_threshold_above() {
        let threshold = AlertThreshold::Above(100.0);
        assert!(threshold.triggered(101.0, None, 0));
        assert!(!threshold.triggered(99.0, None, 0));
        assert!(!threshold.triggered(100.0, None, 0));
    }

    #[test]
    fn test_alert_threshold_below() {
        let threshold = AlertThreshold::Below(50.0);
        assert!(threshold.triggered(49.0, None, 0));
        assert!(!threshold.triggered(51.0, None, 0));
        assert!(!threshold.triggered(50.0, None, 0));
    }

    #[test]
    fn test_alert_threshold_between() {
        let threshold = AlertThreshold::Between {
            min: 10.0,
            max: 90.0,
        };
        assert!(threshold.triggered(50.0, None, 0));
        assert!(!threshold.triggered(9.0, None, 0));
        assert!(!threshold.triggered(91.0, None, 0));
    }

    #[test]
    fn test_alert_threshold_rate_of_change() {
        let threshold = AlertThreshold::RateOfChange {
            threshold: 10.0,
            window_secs: 60,
        };
        assert!(threshold.triggered(115.0, Some(100.0), 60));
        assert!(!threshold.triggered(105.0, Some(100.0), 60));
    }

    #[test]
    fn test_alert_creation() {
        let alert = Alert::new(
            "test_alert".to_string(),
            "Test alert".to_string(),
            AlertSeverity::Warning,
            AlertThreshold::Above(100.0),
        );

        assert_eq!(alert.name, "test_alert");
        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert_eq!(alert.grace_period_secs, 120);
    }

    #[test]
    fn test_alerting_engine_registration() {
        let mut engine = AlertingEngine::new();
        let alert = Alert::new(
            "test".to_string(),
            "Test alert".to_string(),
            AlertSeverity::Warning,
            AlertThreshold::Above(100.0),
        );

        engine.register_alert(alert);
        assert_eq!(engine.get_alert_state("test"), Some(AlertState::None));
    }

    #[test]
    fn test_alerting_engine_state_transition_none_to_pending() {
        let mut engine = AlertingEngine::new();
        let alert = Alert::new(
            "test".to_string(),
            "Test alert".to_string(),
            AlertSeverity::Warning,
            AlertThreshold::Above(100.0),
        );

        engine.register_alert(alert);

        // Trigger alert - should transition to Pending
        let events = engine.update_metric("test", 150.0);
        assert!(events.is_empty()); // No events yet (still in Pending)
        assert_eq!(engine.get_alert_state("test"), Some(AlertState::Pending));
    }

    #[test]
    fn test_alerting_engine_firing() {
        let mut engine = AlertingEngine::new();
        let mut alert = Alert::new(
            "test".to_string(),
            "Test alert".to_string(),
            AlertSeverity::Critical,
            AlertThreshold::Above(100.0),
        );

        // Set very short grace period for testing
        alert = alert.with_grace_period(0);
        engine.register_alert(alert);

        // Trigger alert with zero grace period - should fire immediately
        let events = engine.update_metric("test", 150.0);
        assert!(!events.is_empty());
        assert_eq!(events[0].state, AlertState::Firing);
        assert_eq!(engine.get_alert_state("test"), Some(AlertState::Firing));
    }

    #[test]
    fn test_alerting_engine_resolve() {
        let mut engine = AlertingEngine::new();
        let mut alert = Alert::new(
            "test".to_string(),
            "Test alert".to_string(),
            AlertSeverity::Warning,
            AlertThreshold::Above(100.0),
        );

        alert = alert.with_grace_period(0).with_resolve_period(0);
        engine.register_alert(alert);

        // Fire alert
        engine.update_metric("test", 150.0);
        assert_eq!(engine.get_alert_state("test"), Some(AlertState::Firing));

        // Clear condition - with zero resolve period, goes straight to None
        let events = engine.update_metric("test", 50.0);
        assert!(!events.is_empty());
        assert_eq!(events[0].state, AlertState::Resolved);
        assert_eq!(engine.get_alert_state("test"), Some(AlertState::None));
    }

    #[test]
    fn test_alerting_engine_firing_count() {
        let mut engine = AlertingEngine::new();

        for i in 0..3 {
            let mut alert = Alert::new(
                format!("alert_{}", i),
                format!("Alert {}", i),
                AlertSeverity::Warning,
                AlertThreshold::Above(100.0),
            );
            alert = alert.with_grace_period(0);
            engine.register_alert(alert);
        }

        // Fire first two alerts
        engine.update_metric("alert_0", 150.0);
        engine.update_metric("alert_1", 150.0);
        engine.update_metric("alert_2", 50.0);

        assert_eq!(engine.firing_count(), 2);
    }

    #[test]
    fn test_alert_event_creation() {
        let event = AlertEvent {
            alert_name: "test".to_string(),
            severity: AlertSeverity::Critical,
            state: AlertState::Firing,
            message: "Test fired".to_string(),
            timestamp: 1234567890,
        };

        assert_eq!(event.alert_name, "test");
        assert_eq!(event.severity, AlertSeverity::Critical);
        assert_eq!(event.state, AlertState::Firing);
    }
}
