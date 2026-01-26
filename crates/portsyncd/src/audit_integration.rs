//! NIST SP 800-53 Rev5 audit logging integration for portsyncd
//!
//! This module provides structured audit logging for port synchronization daemon
//! operations with full NIST compliance, RFC 5424 syslog formatting, and
//! multi-backend support (syslog, Redis, SIEM).
//!
//! # NIST Controls Mapping
//!
//! - **AU-2**: Event Logging - Port state changes, initialization, shutdowns
//! - **AU-3**: Content of Audit Records - Structured records with outcomes
//! - **AU-12**: Audit Generation - Macro-based logging for all events
//! - **SI-4**: System Monitoring - Real-time port status monitoring
//! - **SC-7**: Boundary Protection - Port configuration updates

use sonic_audit::{
    AuditCategory, AuditOutcome, AuditRecord, AuditorConfig, Facility, Severity,
    backends::SyslogBackend, init_global_auditor,
};
use std::sync::Arc;

/// Initialize NIST-compliant audit logging for portsyncd
///
/// This sets up the global auditor with syslog backend for RFC 5424 compliance
/// and integration with system monitoring/SIEM infrastructure.
///
/// # NIST Controls
/// - AU-12: Audit Generation - Initialize audit infrastructure
/// - AU-4: Audit Storage Capacity - Syslog backend for persistence
///
/// # Example
/// ```ignore
/// init_portsyncd_auditing().expect("Failed to initialize audit logging");
///
/// // Now use audit macros for logging
/// sonic_audit::audit_log!(
///     AuditRecord::new(AuditCategory::SystemInformationIntegrity, "portsyncd", "startup")
/// );
/// ```
pub fn init_portsyncd_auditing() -> Result<(), Box<dyn std::error::Error>> {
    // Create syslog backend for RFC 5424 compliance
    // NIST: AU-9 - Protection of Audit Information
    let syslog_backend = Arc::new(SyslogBackend::new(
        Facility::Local0, // Local use facility (Local0-Local7 available)
        "portsyncd",      // Application identifier for syslog
    )?);

    // Initialize global auditor with syslog backend
    // NIST: AU-3 - Content of Audit Records
    let config = AuditorConfig::new("portsyncd");

    init_global_auditor(config, syslog_backend)?;

    eprintln!("portsyncd: NIST SP 800-53 Rev5 audit logging initialized");

    Ok(())
}

/// Log port initialization event
///
/// # NIST Controls
/// - AU-12: Audit Generation - Log initialization events
/// - SI-4: System Monitoring - Track port availability
pub fn audit_port_init(port_count: usize) {
    let record = AuditRecord::new(
        AuditCategory::SystemInformationIntegrity,
        "portsyncd",
        "port_initialization_start",
    )
    .with_severity(Severity::Notice) // Significant operational event
    .with_outcome(AuditOutcome::InProgress)
    .with_details(serde_json::json!({
        "port_count": port_count,
        "event_type": "port_sync_initialization"
    }));

    sonic_audit::audit_log!(record);
}

/// Log port initialization completion
///
/// # NIST Controls
/// - AU-12: Audit Generation - Log successful completion
pub fn audit_port_init_done() {
    let record = AuditRecord::new(
        AuditCategory::SystemInformationIntegrity,
        "portsyncd",
        "port_initialization_complete",
    )
    .with_severity(Severity::Notice)
    .with_outcome(AuditOutcome::Success)
    .with_details(serde_json::json!({
        "event_type": "port_sync_complete"
    }));

    sonic_audit::audit_log!(record);
}

/// Log port configuration change
///
/// # NIST Controls
/// - CM-3: Configuration Change Control - Track configuration modifications
/// - AU-12: Audit Generation - Log all configuration changes
pub fn audit_port_config_change(port_name: &str, operation: &str, success: bool) {
    let outcome = if success {
        AuditOutcome::Success
    } else {
        AuditOutcome::Failure
    };

    let record = AuditRecord::new(
        AuditCategory::ConfigurationManagement,
        "portsyncd",
        format!("port_config_change: {}", operation),
    )
    .with_severity(if success {
        Severity::Notice
    } else {
        Severity::Warning
    })
    .with_outcome(outcome)
    .with_object_id(port_name)
    .with_object_type("port")
    .with_details(serde_json::json!({
        "operation": operation,
        "port_name": port_name,
    }));

    sonic_audit::audit_log!(record);
}

/// Log port state change (e.g., admin status, speed)
///
/// # NIST Controls
/// - SI-4: System Monitoring - Track real-time port state changes
/// - AU-12: Audit Generation - Log all state transitions
pub fn audit_port_state_change(
    port_name: &str,
    state_type: &str,
    old_value: &str,
    new_value: &str,
) {
    let record = AuditRecord::new(
        AuditCategory::SystemInformationIntegrity,
        "portsyncd",
        "port_state_change",
    )
    .with_severity(Severity::Informational)
    .with_outcome(AuditOutcome::Success)
    .with_object_id(port_name)
    .with_object_type("port")
    .with_details(serde_json::json!({
        "state_type": state_type,
        "old_value": old_value,
        "new_value": new_value,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }));

    sonic_audit::audit_log!(record);
}

/// Log database operation (read/write to STATE_DB, CONFIG_DB)
///
/// # NIST Controls
/// - AU-4: Audit Storage Capacity - Track database operations
/// - AU-9: Protection of Audit Information - Database protection
pub fn audit_database_operation(db_name: &str, operation: &str, key: &str, success: bool) {
    let outcome = if success {
        AuditOutcome::Success
    } else {
        AuditOutcome::Failure
    };

    let record = AuditRecord::new(
        AuditCategory::DatabaseOperations,
        "portsyncd",
        format!("database_operation: {}", operation),
    )
    .with_severity(if success {
        Severity::Informational
    } else {
        Severity::Error
    })
    .with_outcome(outcome)
    .with_details(serde_json::json!({
        "database": db_name,
        "operation": operation,
        "key": key,
    }));

    sonic_audit::audit_log!(record);
}

/// Log system error or critical event
///
/// # NIST Controls
/// - IR-4: Incident Handling - Track incident-relevant events
/// - SI-11: Error Handling - Log error conditions
pub fn audit_error(error_msg: &str, error_type: &str) {
    let record = AuditRecord::new(
        AuditCategory::SystemInformationIntegrity,
        "portsyncd",
        "system_error",
    )
    .with_severity(Severity::Error)
    .with_outcome(AuditOutcome::Failure)
    .with_error(error_msg.to_string())
    .with_details(serde_json::json!({
        "error_type": error_type,
        "error_message": error_msg,
    }));

    sonic_audit::audit_log!(record);
}

/// Log graceful shutdown
///
/// # NIST Controls
/// - CP-10: System Recovery - Track system state transitions
/// - AU-12: Audit Generation - Log system lifecycle events
pub fn audit_shutdown(reason: &str) {
    let record = AuditRecord::new(
        AuditCategory::ContingencyPlanning,
        "portsyncd",
        "daemon_shutdown",
    )
    .with_severity(Severity::Notice)
    .with_outcome(AuditOutcome::Success)
    .with_details(serde_json::json!({
        "shutdown_reason": reason,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }));

    sonic_audit::audit_log!(record);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_categories_mapped() {
        // Verify audit categories map correctly to NIST controls
        let config_cat = AuditCategory::ConfigurationManagement;
        assert_eq!(config_cat.family_code(), "CM");

        let db_cat = AuditCategory::DatabaseOperations;
        assert_eq!(db_cat.family_code(), "AU");

        let sys_cat = AuditCategory::SystemInformationIntegrity;
        assert_eq!(sys_cat.family_code(), "SI");
    }

    #[test]
    fn test_outcomes() {
        assert!(AuditOutcome::Success.is_success());
        assert!(!AuditOutcome::Failure.is_success());
        assert!(AuditOutcome::Failure.is_failure());
        assert!(AuditOutcome::Denied.is_failure());
    }
}
