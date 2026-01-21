//! Task processing status and result types.

use thiserror::Error;

/// Result of processing a single task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    /// Task completed successfully
    Success,
    /// Task failed due to invalid input
    InvalidEntry,
    /// Task failed (generic)
    Failed,
    /// Task should be retried later
    NeedRetry,
    /// Task was ignored (duplicate, etc.)
    Ignore,
    /// Task was a duplicate of an existing entry
    Duplicated,
    /// Task is waiting for a dependency
    WaitingForDependency,
}

impl TaskStatus {
    /// Returns true if the task completed successfully.
    pub fn is_success(&self) -> bool {
        matches!(self, TaskStatus::Success | TaskStatus::Ignore | TaskStatus::Duplicated)
    }

    /// Returns true if the task should be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(self, TaskStatus::NeedRetry | TaskStatus::WaitingForDependency)
    }

    /// Returns true if the task failed permanently.
    pub fn is_failure(&self) -> bool {
        matches!(self, TaskStatus::InvalidEntry | TaskStatus::Failed)
    }
}

/// Error type for task processing failures.
#[derive(Debug, Clone, Error)]
pub enum TaskError {
    /// Task failed due to invalid entry data
    #[error("Invalid entry: {message}")]
    InvalidEntry { message: String },

    /// Task failed due to a SAI error
    #[error("SAI error: {message}")]
    SaiError { message: String },

    /// Task should be retried later
    #[error("Retry needed: {reason}")]
    NeedRetry { reason: String },

    /// Task is waiting for a dependency
    #[error("Waiting for dependency: {dependency}")]
    WaitingForDependency { dependency: String },

    /// Task was ignored
    #[error("Ignored: {reason}")]
    Ignored { reason: String },

    /// Internal error
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl TaskError {
    /// Creates an invalid entry error.
    pub fn invalid_entry(message: impl Into<String>) -> Self {
        TaskError::InvalidEntry {
            message: message.into(),
        }
    }

    /// Creates a SAI error.
    pub fn sai_error(message: impl Into<String>) -> Self {
        TaskError::SaiError {
            message: message.into(),
        }
    }

    /// Creates a retry error.
    pub fn need_retry(reason: impl Into<String>) -> Self {
        TaskError::NeedRetry {
            reason: reason.into(),
        }
    }

    /// Creates a dependency wait error.
    pub fn waiting_for(dependency: impl Into<String>) -> Self {
        TaskError::WaitingForDependency {
            dependency: dependency.into(),
        }
    }

    /// Creates an ignored error.
    pub fn ignored(reason: impl Into<String>) -> Self {
        TaskError::Ignored {
            reason: reason.into(),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        TaskError::Internal {
            message: message.into(),
        }
    }

    /// Converts this error to a TaskStatus.
    pub fn to_status(&self) -> TaskStatus {
        match self {
            TaskError::InvalidEntry { .. } => TaskStatus::InvalidEntry,
            TaskError::SaiError { .. } => TaskStatus::Failed,
            TaskError::NeedRetry { .. } => TaskStatus::NeedRetry,
            TaskError::WaitingForDependency { .. } => TaskStatus::WaitingForDependency,
            TaskError::Ignored { .. } => TaskStatus::Ignore,
            TaskError::Internal { .. } => TaskStatus::Failed,
        }
    }
}

/// Result type for task processing.
pub type TaskResult<T> = Result<T, TaskError>;

/// Extension trait for converting TaskResult to TaskStatus.
pub trait TaskResultExt {
    /// Converts this result to a TaskStatus.
    fn to_status(&self) -> TaskStatus;
}

impl<T> TaskResultExt for TaskResult<T> {
    fn to_status(&self) -> TaskStatus {
        match self {
            Ok(_) => TaskStatus::Success,
            Err(e) => e.to_status(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_classification() {
        assert!(TaskStatus::Success.is_success());
        assert!(TaskStatus::Ignore.is_success());
        assert!(!TaskStatus::Failed.is_success());

        assert!(TaskStatus::NeedRetry.is_retryable());
        assert!(TaskStatus::WaitingForDependency.is_retryable());
        assert!(!TaskStatus::Success.is_retryable());

        assert!(TaskStatus::Failed.is_failure());
        assert!(TaskStatus::InvalidEntry.is_failure());
        assert!(!TaskStatus::NeedRetry.is_failure());
    }

    #[test]
    fn test_task_error_to_status() {
        assert_eq!(
            TaskError::invalid_entry("test").to_status(),
            TaskStatus::InvalidEntry
        );
        assert_eq!(
            TaskError::need_retry("test").to_status(),
            TaskStatus::NeedRetry
        );
        assert_eq!(
            TaskError::ignored("test").to_status(),
            TaskStatus::Ignore
        );
    }

    #[test]
    fn test_task_result_ext() {
        let ok: TaskResult<()> = Ok(());
        assert_eq!(ok.to_status(), TaskStatus::Success);

        let err: TaskResult<()> = Err(TaskError::need_retry("test"));
        assert_eq!(err.to_status(), TaskStatus::NeedRetry);
    }
}
