//! SAI error types and status handling.
//!
//! This module provides safe error handling for SAI operations, converting
//! raw SAI status codes into Rust's Result type.

use std::fmt;
use thiserror::Error;

/// SAI status codes matching the SAI C API.
///
/// These values correspond to `sai_status_t` in the SAI header files.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaiStatus {
    Success = 0,
    Failure = -1,
    NotSupported = -2,
    NoMemory = -3,
    InsufficientResources = -4,
    InvalidParameter = -5,
    ItemAlreadyExists = -6,
    ItemNotFound = -7,
    BufferOverflow = -8,
    InvalidPortNumber = -9,
    InvalidPortMember = -10,
    InvalidVlanId = -11,
    Uninitialized = -12,
    TableFull = -13,
    MandatoryAttributeMissing = -14,
    NotImplemented = -15,
    AddrNotFound = -16,
    ObjectInUse = -17,
    InvalidObjectType = -18,
    InvalidObjectId = -19,
    InvalidNifId = -20,
    NifTableFull = -21,
    HwTableFull = -22,
    NotExecuted = -23,
    InvalidAttribute = -24,
    // Add more as needed based on SAI headers
}

impl SaiStatus {
    /// Creates a SaiStatus from a raw i32 value.
    pub fn from_raw(status: i32) -> Self {
        match status {
            0 => SaiStatus::Success,
            -1 => SaiStatus::Failure,
            -2 => SaiStatus::NotSupported,
            -3 => SaiStatus::NoMemory,
            -4 => SaiStatus::InsufficientResources,
            -5 => SaiStatus::InvalidParameter,
            -6 => SaiStatus::ItemAlreadyExists,
            -7 => SaiStatus::ItemNotFound,
            -8 => SaiStatus::BufferOverflow,
            -9 => SaiStatus::InvalidPortNumber,
            -10 => SaiStatus::InvalidPortMember,
            -11 => SaiStatus::InvalidVlanId,
            -12 => SaiStatus::Uninitialized,
            -13 => SaiStatus::TableFull,
            -14 => SaiStatus::MandatoryAttributeMissing,
            -15 => SaiStatus::NotImplemented,
            -16 => SaiStatus::AddrNotFound,
            -17 => SaiStatus::ObjectInUse,
            -18 => SaiStatus::InvalidObjectType,
            -19 => SaiStatus::InvalidObjectId,
            -20 => SaiStatus::InvalidNifId,
            -21 => SaiStatus::NifTableFull,
            -22 => SaiStatus::HwTableFull,
            -23 => SaiStatus::NotExecuted,
            -24 => SaiStatus::InvalidAttribute,
            _ => SaiStatus::Failure,
        }
    }

    /// Returns true if the status indicates success.
    pub fn is_success(&self) -> bool {
        *self == SaiStatus::Success
    }

    /// Returns true if the status indicates an error.
    pub fn is_error(&self) -> bool {
        *self != SaiStatus::Success
    }

    /// Converts to a Result, returning Ok(()) for success.
    pub fn into_result(self) -> SaiResult<()> {
        if self.is_success() {
            Ok(())
        } else {
            Err(SaiError::from_status(self))
        }
    }
}

impl fmt::Display for SaiStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SaiStatus::Success => "SAI_STATUS_SUCCESS",
            SaiStatus::Failure => "SAI_STATUS_FAILURE",
            SaiStatus::NotSupported => "SAI_STATUS_NOT_SUPPORTED",
            SaiStatus::NoMemory => "SAI_STATUS_NO_MEMORY",
            SaiStatus::InsufficientResources => "SAI_STATUS_INSUFFICIENT_RESOURCES",
            SaiStatus::InvalidParameter => "SAI_STATUS_INVALID_PARAMETER",
            SaiStatus::ItemAlreadyExists => "SAI_STATUS_ITEM_ALREADY_EXISTS",
            SaiStatus::ItemNotFound => "SAI_STATUS_ITEM_NOT_FOUND",
            SaiStatus::BufferOverflow => "SAI_STATUS_BUFFER_OVERFLOW",
            SaiStatus::InvalidPortNumber => "SAI_STATUS_INVALID_PORT_NUMBER",
            SaiStatus::InvalidPortMember => "SAI_STATUS_INVALID_PORT_MEMBER",
            SaiStatus::InvalidVlanId => "SAI_STATUS_INVALID_VLAN_ID",
            SaiStatus::Uninitialized => "SAI_STATUS_UNINITIALIZED",
            SaiStatus::TableFull => "SAI_STATUS_TABLE_FULL",
            SaiStatus::MandatoryAttributeMissing => "SAI_STATUS_MANDATORY_ATTRIBUTE_MISSING",
            SaiStatus::NotImplemented => "SAI_STATUS_NOT_IMPLEMENTED",
            SaiStatus::AddrNotFound => "SAI_STATUS_ADDR_NOT_FOUND",
            SaiStatus::ObjectInUse => "SAI_STATUS_OBJECT_IN_USE",
            SaiStatus::InvalidObjectType => "SAI_STATUS_INVALID_OBJECT_TYPE",
            SaiStatus::InvalidObjectId => "SAI_STATUS_INVALID_OBJECT_ID",
            SaiStatus::InvalidNifId => "SAI_STATUS_INVALID_NIF_ID",
            SaiStatus::NifTableFull => "SAI_STATUS_NIF_TABLE_FULL",
            SaiStatus::HwTableFull => "SAI_STATUS_HW_TABLE_FULL",
            SaiStatus::NotExecuted => "SAI_STATUS_NOT_EXECUTED",
            SaiStatus::InvalidAttribute => "SAI_STATUS_INVALID_ATTRIBUTE",
        };
        write!(f, "{}", s)
    }
}

/// Error type for SAI operations.
#[derive(Debug, Clone, Error)]
pub enum SaiError {
    /// SAI API returned an error status.
    #[error("SAI operation failed: {status}")]
    Status { status: SaiStatus },

    /// The requested feature is not supported by the SAI implementation.
    #[error("Feature not supported: {feature}")]
    NotSupported { feature: String },

    /// Invalid parameter passed to SAI API.
    #[error("Invalid parameter: {message}")]
    InvalidParameter { message: String },

    /// The requested item was not found.
    #[error("Item not found: {item}")]
    NotFound { item: String },

    /// The item already exists.
    #[error("Item already exists: {item}")]
    AlreadyExists { item: String },

    /// Hardware table is full.
    #[error("Table full: {table}")]
    TableFull { table: String },

    /// Object is in use and cannot be removed.
    #[error("Object in use: {object}")]
    ObjectInUse { object: String },

    /// SAI context is not initialized.
    #[error("SAI not initialized")]
    Uninitialized,

    /// Internal error.
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl SaiError {
    /// Creates an error from a SAI status code.
    pub fn from_status(status: SaiStatus) -> Self {
        match status {
            SaiStatus::Success => {
                // This shouldn't happen, but handle it gracefully
                SaiError::Internal {
                    message: "from_status called with success status".to_string(),
                }
            }
            SaiStatus::NotSupported | SaiStatus::NotImplemented => SaiError::NotSupported {
                feature: "unknown".to_string(),
            },
            SaiStatus::InvalidParameter
            | SaiStatus::InvalidPortNumber
            | SaiStatus::InvalidPortMember
            | SaiStatus::InvalidVlanId
            | SaiStatus::InvalidObjectType
            | SaiStatus::InvalidObjectId
            | SaiStatus::InvalidAttribute => SaiError::InvalidParameter {
                message: format!("SAI returned {}", status),
            },
            SaiStatus::ItemNotFound | SaiStatus::AddrNotFound => SaiError::NotFound {
                item: "unknown".to_string(),
            },
            SaiStatus::ItemAlreadyExists => SaiError::AlreadyExists {
                item: "unknown".to_string(),
            },
            SaiStatus::TableFull | SaiStatus::NifTableFull | SaiStatus::HwTableFull => {
                SaiError::TableFull {
                    table: "unknown".to_string(),
                }
            }
            SaiStatus::ObjectInUse => SaiError::ObjectInUse {
                object: "unknown".to_string(),
            },
            SaiStatus::Uninitialized => SaiError::Uninitialized,
            _ => SaiError::Status { status },
        }
    }

    /// Creates a not supported error with a feature description.
    pub fn not_supported(feature: impl Into<String>) -> Self {
        SaiError::NotSupported {
            feature: feature.into(),
        }
    }

    /// Creates an invalid parameter error with a message.
    pub fn invalid_parameter(message: impl Into<String>) -> Self {
        SaiError::InvalidParameter {
            message: message.into(),
        }
    }

    /// Creates a not found error with an item description.
    pub fn not_found(item: impl Into<String>) -> Self {
        SaiError::NotFound { item: item.into() }
    }

    /// Creates an already exists error.
    pub fn already_exists(item: impl Into<String>) -> Self {
        SaiError::AlreadyExists { item: item.into() }
    }

    /// Creates a table full error.
    pub fn table_full(table: impl Into<String>) -> Self {
        SaiError::TableFull {
            table: table.into(),
        }
    }

    /// Creates an object in use error.
    pub fn object_in_use(object: impl Into<String>) -> Self {
        SaiError::ObjectInUse {
            object: object.into(),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        SaiError::Internal {
            message: message.into(),
        }
    }

    /// Returns the underlying SAI status if this is a Status error.
    pub fn status(&self) -> Option<SaiStatus> {
        match self {
            SaiError::Status { status } => Some(*status),
            _ => None,
        }
    }

    /// Returns true if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SaiError::Status {
                status: SaiStatus::InsufficientResources
                    | SaiStatus::NoMemory
                    | SaiStatus::NotExecuted
            }
        )
    }
}

/// Result type for SAI operations.
pub type SaiResult<T> = Result<T, SaiError>;

/// Extension trait for converting raw SAI status codes.
pub trait SaiStatusExt {
    /// Converts a raw status code to a Result.
    fn to_result(self) -> SaiResult<()>;
}

impl SaiStatusExt for i32 {
    fn to_result(self) -> SaiResult<()> {
        SaiStatus::from_raw(self).into_result()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_success() {
        assert!(SaiStatus::Success.is_success());
        assert!(!SaiStatus::Success.is_error());
        assert!(SaiStatus::Success.into_result().is_ok());
    }

    #[test]
    fn test_status_failure() {
        assert!(!SaiStatus::Failure.is_success());
        assert!(SaiStatus::Failure.is_error());
        assert!(SaiStatus::Failure.into_result().is_err());
    }

    #[test]
    fn test_status_from_raw() {
        assert_eq!(SaiStatus::from_raw(0), SaiStatus::Success);
        assert_eq!(SaiStatus::from_raw(-7), SaiStatus::ItemNotFound);
        assert_eq!(SaiStatus::from_raw(-999), SaiStatus::Failure);
    }

    #[test]
    fn test_error_from_status() {
        let err = SaiError::from_status(SaiStatus::ItemNotFound);
        assert!(matches!(err, SaiError::NotFound { .. }));

        let err = SaiError::from_status(SaiStatus::TableFull);
        assert!(matches!(err, SaiError::TableFull { .. }));
    }

    #[test]
    fn test_raw_status_to_result() {
        assert!(0_i32.to_result().is_ok());
        assert!((-7_i32).to_result().is_err());
    }

    #[test]
    fn test_error_retryable() {
        let err = SaiError::from_status(SaiStatus::InsufficientResources);
        assert!(err.is_retryable());

        let err = SaiError::from_status(SaiStatus::ItemNotFound);
        assert!(!err.is_retryable());
    }
}
