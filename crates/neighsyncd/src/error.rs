//! Error types for neighsyncd
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - SI-11: Error Handling - Structured error types with contextual information
//! - AU-3: Content of Audit Records - Errors include sufficient detail for audit

use std::net::IpAddr;
use thiserror::Error;

/// Errors that can occur in neighsyncd
///
/// # NIST Controls
/// - SI-11(a): Generate error messages providing information necessary for corrective actions
/// - SI-11(b): Reveal only information necessary for error handling (no sensitive data exposure)
#[derive(Debug, Error)]
pub enum NeighsyncError {
    /// Redis connection or operation failed
    /// NIST: SC-8 (Transmission Confidentiality) - Database communication errors
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    /// Netlink socket error
    /// NIST: SC-7 (Boundary Protection) - Kernel interface errors
    #[error("Netlink error: {0}")]
    Netlink(String),

    /// Interface lookup failed
    /// NIST: CM-8 (System Component Inventory) - Interface tracking
    #[error("Interface not found: index {0}")]
    InterfaceNotFound(u32),

    /// Invalid neighbor state
    /// NIST: SI-10 (Information Input Validation) - State validation
    #[error("Invalid neighbor state for {ip}: {reason}")]
    InvalidNeighborState { ip: IpAddr, reason: String },

    /// Configuration error
    /// NIST: CM-6 (Configuration Settings) - Configuration validation
    #[error("Configuration error: {0}")]
    Config(String),

    /// Warm restart timeout
    /// NIST: CP-10 (System Recovery and Reconstitution) - Recovery timeout
    #[error("Warm restart timeout after {0} seconds")]
    WarmRestartTimeout(u64),

    /// IO error
    /// NIST: SI-11 (Error Handling) - System-level errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// State replication error
    /// NIST: SC-8 (Transmission Confidentiality) - Distributed state coordination
    #[error("State replication error: {0}")]
    Replication(String),

    /// Distributed lock acquisition failed
    /// NIST: AC-3 (Access Enforcement) - Lock-based access control
    #[error("Failed to acquire distributed lock: {0}")]
    LockAcquisitionFailed(String),
}

/// Result type alias for neighsyncd operations
pub type Result<T> = std::result::Result<T, NeighsyncError>;
