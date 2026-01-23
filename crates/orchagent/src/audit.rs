//! Audit logging module for NIST security compliance.
//!
//! This module provides structured audit logging that complies with NIST SP 800-53
//! security controls for auditing (AU family). It supports:
//!
//! - AU-2: Audit Events - Configurable event types for security-relevant actions
//! - AU-3: Content of Audit Records - Structured records with timestamp, source, action, outcome
//! - AU-6: Audit Review, Analysis, and Reporting - JSON-structured logs for SIEM integration
//! - AU-8: Time Stamps - UTC timestamps with microsecond precision
//! - AU-9: Protection of Audit Information - Immutable log records
//!
//! # Syslog Severity Levels (RFC 5424)
//!
//! | Level | Severity | Description |
//! |-------|----------|-------------|
//! | 0 | Emergency | System is unusable |
//! | 1 | Alert | Action must be taken immediately |
//! | 2 | Critical | Critical conditions |
//! | 3 | Error | Error conditions |
//! | 4 | Warning | Warning conditions |
//! | 5 | Notice | Normal but significant condition |
//! | 6 | Info | Informational messages |
//! | 7 | Debug | Debug-level messages |

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Audit event categories aligned with NIST SP 800-53 AU-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditCategory {
    /// Authentication and authorization events
    Authentication,
    /// Configuration changes to the system
    ConfigurationChange,
    /// Resource creation events
    ResourceCreate,
    /// Resource modification events
    ResourceModify,
    /// Resource deletion events
    ResourceDelete,
    /// System startup and shutdown
    SystemLifecycle,
    /// Security policy changes
    SecurityPolicy,
    /// Network configuration changes
    NetworkConfig,
    /// SAI (Switch Abstraction Interface) operations
    SaiOperation,
    /// Error and failure events
    ErrorCondition,
    /// Administrative actions
    AdminAction,
    /// Warm restart events
    WarmRestart,
}

impl fmt::Display for AuditCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditCategory::Authentication => write!(f, "AUTHENTICATION"),
            AuditCategory::ConfigurationChange => write!(f, "CONFIGURATION_CHANGE"),
            AuditCategory::ResourceCreate => write!(f, "RESOURCE_CREATE"),
            AuditCategory::ResourceModify => write!(f, "RESOURCE_MODIFY"),
            AuditCategory::ResourceDelete => write!(f, "RESOURCE_DELETE"),
            AuditCategory::SystemLifecycle => write!(f, "SYSTEM_LIFECYCLE"),
            AuditCategory::SecurityPolicy => write!(f, "SECURITY_POLICY"),
            AuditCategory::NetworkConfig => write!(f, "NETWORK_CONFIG"),
            AuditCategory::SaiOperation => write!(f, "SAI_OPERATION"),
            AuditCategory::ErrorCondition => write!(f, "ERROR_CONDITION"),
            AuditCategory::AdminAction => write!(f, "ADMIN_ACTION"),
            AuditCategory::WarmRestart => write!(f, "WARM_RESTART"),
        }
    }
}

/// Outcome of an audited action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditOutcome {
    /// Action completed successfully
    Success,
    /// Action failed
    Failure,
    /// Action is in progress
    InProgress,
    /// Action was denied due to policy
    Denied,
}

impl fmt::Display for AuditOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditOutcome::Success => write!(f, "success"),
            AuditOutcome::Failure => write!(f, "failure"),
            AuditOutcome::InProgress => write!(f, "in_progress"),
            AuditOutcome::Denied => write!(f, "denied"),
        }
    }
}

/// Structured audit record compliant with NIST SP 800-53 AU-3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// UTC timestamp with microsecond precision (AU-8)
    pub timestamp: DateTime<Utc>,
    /// Event category for filtering and analysis
    pub category: AuditCategory,
    /// Source module/component generating the event
    pub source: String,
    /// Human-readable action description
    pub action: String,
    /// Outcome of the action
    pub outcome: AuditOutcome,
    /// Object identifier affected by the action (e.g., OID, port name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    /// Object type (e.g., "trap", "mirror_session", "fdb_entry")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
    /// Additional context as key-value pairs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Error message if outcome is failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Correlation ID for tracking related events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl AuditRecord {
    /// Create a new audit record with the current timestamp.
    pub fn new(category: AuditCategory, source: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            category,
            source: source.into(),
            action: action.into(),
            outcome: AuditOutcome::InProgress,
            object_id: None,
            object_type: None,
            details: None,
            error: None,
            correlation_id: None,
        }
    }

    /// Set the outcome of the action.
    pub fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Set the object identifier.
    pub fn with_object_id(mut self, id: impl Into<String>) -> Self {
        self.object_id = Some(id.into());
        self
    }

    /// Set the object type.
    pub fn with_object_type(mut self, obj_type: impl Into<String>) -> Self {
        self.object_type = Some(obj_type.into());
        self
    }

    /// Add additional details as JSON.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Set the error message.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self.outcome = AuditOutcome::Failure;
        self
    }

    /// Set the correlation ID for tracking related events.
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Convert to JSON string for logging.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!(
                r#"{{"error":"serialization_failed","message":"{}"}}"#,
                e
            )
        })
    }
}

/// Macro for debug-level logging with structured context.
///
/// Debug messages are only emitted when debug logging is enabled.
/// Use for detailed troubleshooting information.
#[macro_export]
macro_rules! debug_log {
    ($source:expr, $($arg:tt)*) => {
        tracing::debug!(
            source = $source,
            $($arg)*
        )
    };
}

/// Macro for info-level logging with structured context.
///
/// Info messages indicate normal operational events.
#[macro_export]
macro_rules! info_log {
    ($source:expr, $($arg:tt)*) => {
        tracing::info!(
            source = $source,
            $($arg)*
        )
    };
}

/// Macro for warning-level logging with structured context.
///
/// Warning messages indicate potential issues that don't prevent operation.
#[macro_export]
macro_rules! warn_log {
    ($source:expr, $($arg:tt)*) => {
        tracing::warn!(
            source = $source,
            $($arg)*
        )
    };
}

/// Macro for error-level logging with structured context.
///
/// Error messages indicate failures that affect operation.
#[macro_export]
macro_rules! error_log {
    ($source:expr, $($arg:tt)*) => {
        tracing::error!(
            source = $source,
            $($arg)*
        )
    };
}

/// Macro for security audit logging (NIST AU-2 compliant).
///
/// Emits a structured JSON audit record for security-relevant events.
/// These events should be forwarded to a SIEM for analysis.
#[macro_export]
macro_rules! audit_log {
    ($record:expr) => {
        let record = $record;
        match record.outcome {
            $crate::audit::AuditOutcome::Success => {
                tracing::info!(
                    target: "audit",
                    category = %record.category,
                    source = %record.source,
                    action = %record.action,
                    outcome = %record.outcome,
                    audit_json = %record.to_json(),
                    "AUDIT: {} - {} - {}",
                    record.category,
                    record.action,
                    record.outcome
                );
            }
            $crate::audit::AuditOutcome::InProgress => {
                tracing::debug!(
                    target: "audit",
                    category = %record.category,
                    source = %record.source,
                    action = %record.action,
                    outcome = %record.outcome,
                    audit_json = %record.to_json(),
                    "AUDIT: {} - {} - {}",
                    record.category,
                    record.action,
                    record.outcome
                );
            }
            $crate::audit::AuditOutcome::Failure | $crate::audit::AuditOutcome::Denied => {
                tracing::warn!(
                    target: "audit",
                    category = %record.category,
                    source = %record.source,
                    action = %record.action,
                    outcome = %record.outcome,
                    error = record.error.as_deref().unwrap_or(""),
                    audit_json = %record.to_json(),
                    "AUDIT: {} - {} - {}",
                    record.category,
                    record.action,
                    record.outcome
                );
            }
        }
    };
}

/// Macro for security-critical audit events (NIST AU-2 high-impact).
///
/// Use for authentication failures, policy violations, and critical errors.
#[macro_export]
macro_rules! security_audit {
    ($record:expr) => {
        let record = $record;
        tracing::warn!(
            target: "security_audit",
            category = %record.category,
            source = %record.source,
            action = %record.action,
            outcome = %record.outcome,
            error = record.error.as_deref().unwrap_or(""),
            audit_json = %record.to_json(),
            "SECURITY_AUDIT: {} - {} - {}",
            record.category,
            record.action,
            record.outcome
        );
    };
}

/// Initialize the tracing subscriber for structured logging.
///
/// This should be called once at application startup.
pub fn init_logging(log_level: &str) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .json()
        )
        .init();
}

/// Initialize logging for development with human-readable output.
pub fn init_logging_pretty(log_level: &str) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
                .pretty()
        )
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_record_creation() {
        let record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "CoppOrch",
            "create_trap",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id("0x1000")
        .with_object_type("copp_trap");

        assert_eq!(record.category, AuditCategory::ResourceCreate);
        assert_eq!(record.source, "CoppOrch");
        assert_eq!(record.action, "create_trap");
        assert_eq!(record.outcome, AuditOutcome::Success);
        assert_eq!(record.object_id, Some("0x1000".to_string()));
        assert_eq!(record.object_type, Some("copp_trap".to_string()));
    }

    #[test]
    fn test_audit_record_with_error() {
        let record = AuditRecord::new(
            AuditCategory::ErrorCondition,
            "MirrorOrch",
            "create_session",
        )
        .with_error("SAI operation failed: invalid port");

        assert_eq!(record.outcome, AuditOutcome::Failure);
        assert_eq!(record.error, Some("SAI operation failed: invalid port".to_string()));
    }

    #[test]
    fn test_audit_record_json_serialization() {
        let record = AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "SwitchOrch",
            "set_ecmp_hash",
        )
        .with_outcome(AuditOutcome::Success)
        .with_details(serde_json::json!({
            "algorithm": "crc",
            "seed": 42
        }));

        let json = record.to_json();
        assert!(json.contains("CONFIGURATION_CHANGE"));
        assert!(json.contains("SwitchOrch"));
        assert!(json.contains("set_ecmp_hash"));
        assert!(json.contains("\"algorithm\":\"crc\""));
    }

    #[test]
    fn test_audit_category_display() {
        assert_eq!(AuditCategory::Authentication.to_string(), "AUTHENTICATION");
        assert_eq!(AuditCategory::ResourceCreate.to_string(), "RESOURCE_CREATE");
        assert_eq!(AuditCategory::SaiOperation.to_string(), "SAI_OPERATION");
    }

    #[test]
    fn test_audit_outcome_display() {
        assert_eq!(AuditOutcome::Success.to_string(), "success");
        assert_eq!(AuditOutcome::Failure.to_string(), "failure");
        assert_eq!(AuditOutcome::Denied.to_string(), "denied");
    }

    #[test]
    fn test_audit_record_with_correlation_id() {
        let record = AuditRecord::new(
            AuditCategory::WarmRestart,
            "SwitchOrch",
            "begin_warm_restart",
        )
        .with_correlation_id("wr-12345")
        .with_outcome(AuditOutcome::InProgress);

        assert_eq!(record.correlation_id, Some("wr-12345".to_string()));
    }
}
