//! Error types for cfgmgr operations.
//!
//! This module defines the error types used throughout the cfgmgr crates.
//! All errors implement `std::error::Error` via `thiserror`.

use std::io;
use thiserror::Error;

/// Result type alias for cfgmgr operations.
pub type CfgMgrResult<T> = Result<T, CfgMgrError>;

/// Errors that can occur during cfgmgr operations.
#[derive(Debug, Error)]
pub enum CfgMgrError {
    /// Failed to execute a shell command (spawn error).
    #[error("Failed to execute shell command '{command}': {source}")]
    ShellExec {
        /// The command that failed to execute.
        command: String,
        /// The underlying IO error.
        #[source]
        source: io::Error,
    },

    /// Shell command returned non-zero exit code.
    #[error("Shell command failed: '{command}' (exit code {exit_code}): {output}")]
    ShellCommandFailed {
        /// The command that failed.
        command: String,
        /// The exit code.
        exit_code: i32,
        /// Combined stdout/stderr output.
        output: String,
    },

    /// Redis/database operation failed.
    #[error("Database operation failed: {operation}: {message}")]
    Database {
        /// The operation that failed (e.g., "get", "set", "subscribe").
        operation: String,
        /// Error message.
        message: String,
    },

    /// Configuration validation error.
    #[error("Invalid configuration for {field}: {message}")]
    InvalidConfig {
        /// The field that failed validation.
        field: String,
        /// Error message.
        message: String,
    },

    /// Port/interface not found or not ready.
    #[error("Port '{port}' not found or not ready")]
    PortNotReady {
        /// The port alias.
        port: String,
    },

    /// VLAN not found or invalid.
    #[error("VLAN '{vlan}' not found or invalid")]
    VlanNotFound {
        /// The VLAN identifier.
        vlan: String,
    },

    /// Table entry not found.
    #[error("Table entry not found: {table}:{key}")]
    EntryNotFound {
        /// The table name.
        table: String,
        /// The key.
        key: String,
    },

    /// Warm restart operation failed.
    #[error("Warm restart failed: {message}")]
    WarmRestart {
        /// Error message.
        message: String,
    },

    /// Netlink socket operation failed.
    #[error("Netlink operation failed: {operation}: {message}")]
    Netlink {
        /// The operation that failed.
        operation: String,
        /// Error message.
        message: String,
    },

    /// Internal error (unexpected state).
    #[error("Internal error: {message}")]
    Internal {
        /// Error message.
        message: String,
    },
}

impl CfgMgrError {
    /// Creates a database error.
    pub fn database(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Database {
            operation: operation.into(),
            message: message.into(),
        }
    }

    /// Creates an invalid configuration error.
    pub fn invalid_config(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidConfig {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Creates a port not ready error.
    pub fn port_not_ready(port: impl Into<String>) -> Self {
        Self::PortNotReady { port: port.into() }
    }

    /// Creates an entry not found error.
    pub fn entry_not_found(table: impl Into<String>, key: impl Into<String>) -> Self {
        Self::EntryNotFound {
            table: table.into(),
            key: key.into(),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Returns true if this error indicates a transient condition
    /// that may succeed on retry.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CfgMgrError::PortNotReady { .. }
                | CfgMgrError::Database { .. }
                | CfgMgrError::ShellCommandFailed { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CfgMgrError::port_not_ready("Ethernet0");
        assert_eq!(err.to_string(), "Port 'Ethernet0' not found or not ready");
    }

    #[test]
    fn test_database_error() {
        let err = CfgMgrError::database("hget", "Connection refused");
        assert_eq!(
            err.to_string(),
            "Database operation failed: hget: Connection refused"
        );
    }

    #[test]
    fn test_shell_command_failed() {
        let err = CfgMgrError::ShellCommandFailed {
            command: "ip link set dev eth0 mtu 9100".to_string(),
            exit_code: 2,
            output: "Cannot find device".to_string(),
        };
        assert!(err.to_string().contains("ip link set dev"));
        assert!(err.to_string().contains("exit code 2"));
    }

    #[test]
    fn test_is_retryable() {
        assert!(CfgMgrError::port_not_ready("Ethernet0").is_retryable());
        assert!(CfgMgrError::database("get", "timeout").is_retryable());
        assert!(!CfgMgrError::internal("bug").is_retryable());
    }
}
