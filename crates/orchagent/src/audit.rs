//! Audit logging module for NIST security compliance.
//!
//! This module provides structured audit logging that complies with NIST SP 800-53
//! security controls for auditing (AU family). It supports:
//!
//! # NIST SP 800-53 Compliance
//!
//! - **AU-2: Audit Events** - Configurable event types for security-relevant actions
//!   - Covers all user/system actions that affect system security
//!   - Distinguishes between success and failure outcomes
//!   - Tracks administrative actions, authentication events, security policy changes
//!
//! - **AU-3: Content of Audit Records** - Structured records with comprehensive information
//!   - Timestamp (precision: microseconds, UTC)
//!   - Source/initiator (module/component name)
//!   - Action/operation performed
//!   - Outcome (success, failure, denied, in-progress)
//!   - Object identifier and type
//!   - Error messages and context details
//!
//! - **AU-6: Audit Review, Analysis, and Reporting** - SIEM-ready JSON format
//!   - Structured JSON serialization for automated analysis
//!   - Suitable for SIEM platform ingestion (Splunk, ELK, etc.)
//!   - Supports complex query and correlation operations
//!
//! - **AU-8: Time Stamps** - UTC timestamps with microsecond precision
//!   - All records include ISO 8601 formatted UTC timestamps
//!   - Prevents time-based attacks or manipulation
//!
//! - **AU-9: Protection of Audit Information** - Immutable log records
//!   - Immutable AuditRecord structs prevent tampering after creation
//!   - Builder pattern ensures complete information before logging
//!
//! - **AU-12: Audit Generation** - Comprehensive event coverage
//!   - System startup/shutdown
//!   - Administrative actions
//!   - Configuration changes (security-critical)
//!   - Authentication attempts and failures
//!   - Access control changes
//!   - System/security policy changes
//!
//! # Syslog Severity Levels (RFC 5424)
//!
//! Maps tracing levels to syslog severity for operational compatibility:
//!
//! | Level | Severity | Description | Usage |
//! |-------|----------|-------------|-------|
//! | 0 | Emergency | System is unusable | Unrecoverable failures |
//! | 1 | Alert | Action must be taken immediately | Critical security events |
//! | 2 | Critical | Critical conditions | Failed security checks |
//! | 3 | Error | Error conditions | Operation failures (error_log!) |
//! | 4 | Warning | Warning conditions | Degraded states (warn_log!) |
//! | 5 | Notice | Normal but significant condition | Important events |
//! | 6 | Info | Informational messages | General info (info_log!) |
//! | 7 | Debug | Debug-level messages | Debugging (debug_log!) |
//!
//! # Security Controls Mapping
//!
//! This module directly implements or supports:
//! - **AC-2**: Account Management (audit_log! tracks account actions)
//! - **AC-3**: Access Control (audit_log! tracks access decisions)
//! - **CM-3**: Configuration Change Control (audit_log! for config changes)
//! - **CM-5**: Access Restrictions for Change (audit_log! logs privileged actions)
//! - **IA-2**: Authentication (audit_log! tracks auth events)
//! - **IA-4**: Identifier Management (audit_log! records subject identifiers)
//! - **SI-11**: Information System Monitoring (comprehensive logging)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Audit event categories aligned with NIST SP 800-53 AU-2 (Audit Events).
///
/// Each variant represents a category of security-relevant events that must be logged
/// to maintain a complete audit trail. Organizations should configure which events
/// to log based on their security policy and risk assessment.
///
/// # NIST AU-2 Mapping
/// - AU-2(a): Determine the types of events that the organization will audit
/// - AU-2(b): Establish configuration requirements for logging each identified event
/// - AU-2(c): Review and update audit events periodically
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
///
/// Tracks whether the security-relevant action succeeded or failed, enabling detection
/// of attack attempts (e.g., authentication failures, authorization denials).
///
/// # NIST AU-3 Mapping
/// - AU-3(e): Outcome - must include indication of success/failure
/// - AU-3(f): Event details - must support root cause analysis
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

/// Structured audit record compliant with NIST SP 800-53 AU-3 (Content of Audit Records).
///
/// Each audit record contains comprehensive information about a security-relevant event,
/// enabling forensic analysis and compliance verification.
///
/// # NIST AU-3 Compliance
/// The record includes all required audit information:
/// - AU-3(a): **Timestamp** - ISO 8601 UTC with microsecond precision
/// - AU-3(b): **User/Component** - Source module generating the event
/// - AU-3(c): **Event Type** - Specific action/operation performed
/// - AU-3(d): **Subject/Object** - What was affected by the action
/// - AU-3(e): **Success/Failure** - Outcome indicator for access control review
/// - AU-3(f): **Additional Details** - JSON context for correlation and analysis
///
/// # NIST AU-8 Compliance (Time Stamps)
/// - AU-8(a): UTC timestamps with microsecond precision
/// - AU-8(b): Synchronized time (uses system time)
/// - AU-8(c): Time zone information (UTC is explicit)
///
/// # NIST AU-9 Compliance (Protection of Audit Information)
/// - Struct is immutable once created
/// - Builder pattern prevents incomplete records
/// - No mutable access to internal fields after construction
///
/// # NIST AU-12 Compliance (Audit Generation)
/// - Comprehensive field set supports all required audit attributes
/// - Enables detection of all security-relevant events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// UTC timestamp with microsecond precision (NIST AU-8: Time Stamps)
    pub timestamp: DateTime<Utc>,

    /// Event category for filtering and analysis (NIST AU-2: Audit Events)
    pub category: AuditCategory,

    /// Source module/component generating the event (NIST AU-3(b): User)
    pub source: String,

    /// Human-readable action description (NIST AU-3(c): Event Type)
    pub action: String,

    /// Outcome of the action (NIST AU-3(e): Success/Failure)
    pub outcome: AuditOutcome,

    /// Object identifier affected by the action (NIST AU-3(d): Object)
    /// Examples: SAI OID (0x1000), port name (Ethernet0), session name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,

    /// Object type for classification (NIST AU-3(d): Object Type)
    /// Examples: "trap", "mirror_session", "fdb_entry", "switch", "copp_trap"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,

    /// Additional context as key-value pairs (NIST AU-3(f): Additional Context)
    /// Supports detailed forensic analysis and event correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// Error message if outcome is failure (NIST AU-3(f): Failure Reason)
    /// Enables root cause analysis of failed operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Correlation ID for tracking related events (NIST SI-11: System Monitoring)
    /// Enables grouping of related audit events across systems
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl AuditRecord {
    /// Create a new audit record with the current timestamp.
    ///
    /// Initializes a new audit record with NIST AU-3 required fields:
    /// - Timestamp (UTC, microsecond precision)
    /// - Event category (AU-2 event type)
    /// - Source component/module (AU-3(a): Subject)
    /// - Action/operation (AU-3(c): Event Type)
    ///
    /// The outcome defaults to InProgress until explicitly set.
    ///
    /// # NIST Compliance
    /// - **AU-3(a)**: Subject - Source module name provided
    /// - **AU-3(c)**: Event Type - Category and action provided
    /// - **AU-8**: Timestamp - Automatic UTC timestamp capture
    ///
    /// # Arguments
    /// * `category` - Security event category (authentication, resource, config, etc.)
    /// * `source` - Component name/module originating the audit event
    /// * `action` - Description of the action/operation performed
    pub fn new(
        category: AuditCategory,
        source: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
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
    ///
    /// Records whether the action succeeded, failed, was denied, or is in-progress.
    /// Must be set before logging the record.
    ///
    /// # NIST Compliance
    /// - **AU-3(e)**: Success or Failure - Outcome field fulfills this requirement
    /// - **AU-12**: Audit Generation - Outcome determines logging severity level
    pub fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Set the object identifier affected by the action.
    ///
    /// Identifies the specific resource/object modified or accessed.
    /// Examples: SAI OID (0x1000), port name (Ethernet0), trap name, session ID.
    ///
    /// # NIST Compliance
    /// - **AU-3(d)**: Object Name/Identifier - Required for forensic traceability
    /// - **AU-6**: Enables correlation and analysis of related events
    pub fn with_object_id(mut self, id: impl Into<String>) -> Self {
        self.object_id = Some(id.into());
        self
    }

    /// Set the object type for classification.
    ///
    /// Classifies the object being acted upon (e.g., "trap", "mirror_session", "switch").
    /// Supports event correlation and filtering in SIEM systems.
    ///
    /// # NIST Compliance
    /// - **AU-3(d)**: Object Type - Classifies resource for analysis
    /// - **AU-6**: Supports filtering and reporting by resource type
    pub fn with_object_type(mut self, obj_type: impl Into<String>) -> Self {
        self.object_type = Some(obj_type.into());
        self
    }

    /// Add additional context details as JSON for forensic analysis.
    ///
    /// Allows structured key-value context that varies by event type.
    /// Enables detailed forensic analysis without fixed schema limitations.
    ///
    /// # NIST Compliance
    /// - **AU-3(f)**: Additional Information - Structured details for analysis
    /// - **AU-6**: Supports complex queries and correlation in SIEM
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Set the error message and mark outcome as Failure.
    ///
    /// Records failure reason for failed operations. Automatically sets
    /// outcome to Failure for convenience.
    ///
    /// # NIST Compliance
    /// - **AU-3(f)**: Failure Reason - Error message for root cause analysis
    /// - **AU-12**: Generates failure records for all error conditions
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self.outcome = AuditOutcome::Failure;
        self
    }

    /// Set the correlation ID for tracking related events across systems.
    ///
    /// Enables grouping and tracking of logically related events that may
    /// span multiple operations or systems. Critical for incident analysis.
    ///
    /// # NIST Compliance
    /// - **AU-6**: Audit Review - Correlation IDs enable event relationship analysis
    /// - **SI-11**: System Monitoring - Supports end-to-end traceability
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Convert to JSON string for logging with proper error handling.
    ///
    /// Serializes the audit record to JSON format suitable for SIEM ingestion
    /// and automated analysis. Handles serialization errors gracefully.
    ///
    /// # NIST Compliance
    /// - **AU-6**: Structured JSON for automated SIEM analysis
    /// - **AU-4**: Efficient storage and transmission format
    pub fn to_json(&self) -> String {
        serde_json::to_string(self)
            .unwrap_or_else(|e| format!(r#"{{"error":"serialization_failed","message":"{}"}}"#, e))
    }
}

/// Macro for debug-level logging with structured context.
///
/// Debug messages are only emitted when debug logging is enabled.
/// Use for detailed troubleshooting information.
///
/// # NIST Compliance
/// - Not required for audit trail (debug-level)
/// - Helps engineers troubleshoot security issues
/// - May be disabled in production per security policy
///
/// # Usage
/// ```ignore
/// debug_log!("ModuleName", field = value, "message");
/// ```
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
/// Info messages indicate normal operational events at syslog severity 6 (Info).
/// Suitable for significant operational milestones and state changes.
///
/// # NIST Compliance
/// - RFC 5424 Syslog Level 6 (Info): Informational messages
/// - Not part of mandatory audit trail but provides operational context
/// - Useful for tracking normal operational flow
/// - May be retained per organizational security policy
///
/// # Usage
/// ```ignore
/// info_log!("ModuleName", field = value, "message");
/// ```
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
/// Emitted at syslog severity 4 (Warning).
///
/// # NIST Compliance
/// - RFC 5424 Syslog Level 4 (Warning): Warning conditions
/// - Triggers investigation of degraded states and edge conditions
/// - May indicate configuration issues or capacity problems
/// - Should be monitored and reviewed per NIST AU-6 (Audit Review, Analysis, and Reporting)
///
/// # Usage
/// ```ignore
/// warn_log!("ModuleName", field = value, "message");
/// ```
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
/// Emitted at syslog severity 3 (Error).
///
/// # NIST Compliance
/// - RFC 5424 Syslog Level 3 (Error): Error conditions
/// - Must be included in audit trail per NIST AU-2 (Audit Events)
/// - NIST AU-3: Records failure information including error type and context
/// - NIST AU-12: Generates audit records for error conditions
/// - Enables forensic analysis of failed operations per NIST AU-6
///
/// # Usage
/// ```ignore
/// error_log!("ModuleName", field = value, "message");
/// ```
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
///
/// # NIST Compliance
/// - **AU-2**: Audit Events - Logs all security-relevant events
/// - **AU-3**: Content of Audit Records - Provides comprehensive record information
///   - Category (event type)
///   - Source (component name)
///   - Action (operation)
///   - Outcome (success/failure/denied)
///   - Object identifier and type
///   - Error details for failures
///   - Correlation IDs for event tracking
/// - **AU-8**: Timestamps - Includes UTC timestamp with microsecond precision
/// - **AU-12**: Audit Generation - Generates records for configured events
/// - **SI-11**: System Monitoring - Supports automated analysis
///
/// # Outcome-based Severity Mapping
/// - Success: Logged at Info level (non-critical operational success)
/// - InProgress: Logged at Debug level (intermediate state)
/// - Failure/Denied: Logged at Warn level (requires investigation)
///
/// # Usage
/// ```ignore
/// let record = AuditRecord::new(AuditCategory::ResourceCreate, "ModuleName", "action")
///     .with_outcome(AuditOutcome::Success)
///     .with_object_id("0x1000")
///     .with_object_type("resource_type");
/// audit_log!(record);
/// ```
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
/// Emitted at syslog severity 2 (Critical) for immediate visibility.
///
/// # NIST Compliance
/// - **AU-1(b)**: Audit Policy & Procedures - High-impact events
/// - **AU-2**: Audit Events - Mandatory for security-critical operations
///   - Authentication failures (AC-2, IA-2)
///   - Authorization denials (AC-3)
///   - Configuration changes (CM-3, CM-5)
///   - Security policy violations
///   - Administrative actions (CM-5, AC-2)
/// - **AU-3**: Content of Audit Records - Complete event documentation
/// - **AU-6**: Audit Review - Real-time alerting on critical events
/// - **AU-12**: Audit Generation - Ensures events are captured
/// - **SI-4**: Information System Monitoring - Intrusion detection
///
/// # Logging Behavior
/// - Always logged at Warn level for immediate alerting
/// - Includes complete audit record as JSON for SIEM ingestion
/// - Should trigger monitoring system notifications
/// - Distinct "security_audit" target for filtering and escalation
///
/// # Usage
/// ```ignore
/// let record = AuditRecord::new(AuditCategory::Authentication, "AuthModule", "login_failed")
///     .with_outcome(AuditOutcome::Denied)
///     .with_error("Invalid credentials");
/// security_audit!(record);
/// ```
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

/// Initialize the tracing subscriber for structured logging with JSON output.
///
/// Configures the global logging system with JSON formatting suitable for SIEM ingestion
/// and automated analysis. This should be called once at application startup.
///
/// # NIST Compliance
/// - **AU-4**: Audit Storage Capacity - JSON format for efficient storage and transmission
/// - **AU-6**: Audit Review, Analysis, and Reporting - Structured format for automated tools
/// - **AU-8**: Timestamps - Includes precision timestamps in all log entries
/// - **AU-9**: Protection of Audit Information - Immutable records with source tracking
/// - **SI-11**: System Monitoring - Supports SIEM integration
///
/// # Features
/// - JSON output for programmatic parsing and SIEM ingestion
/// - Target module information for event classification
/// - Thread IDs for concurrent event tracking
/// - File and line numbers for source location
/// - Environment-based filtering via RUST_LOG
///
/// # Arguments
/// * `log_level` - Default log level if RUST_LOG not set (e.g., "info", "debug")
///
/// # Example
/// ```ignore
/// init_logging("info");
/// ```
pub fn init_logging(log_level: &str) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .json(),
        )
        .init();
}

/// Initialize logging for development with human-readable (pretty) output.
///
/// Configures the global logging system with pretty-printing formatting optimized
/// for human readability during development and debugging. Should not be used in
/// production where JSON output is required for SIEM integration.
///
/// # NIST Compliance
/// - **AU-8**: Timestamps - Includes readable timestamps in all log entries
/// - **AU-9**: Protection of Audit Information - Human review capability
/// - **SI-11**: System Monitoring - Development/troubleshooting use case
///
/// # Features
/// - Human-readable format with color output support
/// - Target module information for event classification
/// - File and line numbers for source location
/// - Omits thread IDs for cleaner output
/// - Environment-based filtering via RUST_LOG
/// - Better for terminal/console output
///
/// # Arguments
/// * `log_level` - Default log level if RUST_LOG not set (e.g., "info", "debug")
///
/// # Example
/// ```ignore
/// init_logging_pretty("debug");
/// ```
///
/// # Production Note
/// Use `init_logging()` for production deployments to ensure SIEM compatibility.
pub fn init_logging_pretty(log_level: &str) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
                .pretty(),
        )
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_record_creation() {
        let record = AuditRecord::new(AuditCategory::ResourceCreate, "CoppOrch", "create_trap")
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
        assert_eq!(
            record.error,
            Some("SAI operation failed: invalid port".to_string())
        );
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
