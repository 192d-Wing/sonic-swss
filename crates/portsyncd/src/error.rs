//! Error types for portsyncd

use thiserror::Error;

/// Port synchronization daemon errors
#[derive(Error, Debug)]
pub enum PortsyncError {
    /// Database connection error
    #[error("Database error: {0}")]
    Database(String),

    /// Netlink error
    #[error("Netlink error: {0}")]
    Netlink(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Port validation error
    #[error("Port validation error: {0}")]
    PortValidation(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("Error: {0}")]
    Other(String),
}

/// Result type for portsyncd operations
pub type Result<T> = std::result::Result<T, PortsyncError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PortsyncError::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");
    }

    #[test]
    fn test_error_netlink() {
        let err = PortsyncError::Netlink("subscribe failed".to_string());
        assert_eq!(err.to_string(), "Netlink error: subscribe failed");
    }

    #[test]
    fn test_error_port_validation() {
        let err = PortsyncError::PortValidation("invalid port name".to_string());
        assert_eq!(err.to_string(), "Port validation error: invalid port name");
    }
}
