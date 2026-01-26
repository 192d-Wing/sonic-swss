//! Verification helpers for testing configuration managers
//!
//! Provides assertion helpers to verify APPL_DB state and command execution

use crate::RedisTestEnv;
use std::collections::HashMap;
use thiserror::Error;

/// Verification error types
#[derive(Error, Debug)]
pub enum VerificationError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Expected key '{key}' not found in APPL_DB")]
    KeyNotFound { key: String },

    #[error("Expected field '{field}' not found in key '{key}'")]
    FieldNotFound { key: String, field: String },

    #[error("Value mismatch for {key}:{field}: expected '{expected}', got '{actual}'")]
    ValueMismatch {
        key: String,
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Expected {expected} keys matching pattern '{pattern}', found {actual}")]
    KeyCountMismatch {
        pattern: String,
        expected: usize,
        actual: usize,
    },
}

/// Result type for verification operations
pub type VerifyResult<T> = Result<T, VerificationError>;

/// APPL_DB verification helper
pub struct AppDbVerifier<'a> {
    env: &'a RedisTestEnv,
}

impl<'a> AppDbVerifier<'a> {
    /// Create a new APPL_DB verifier
    pub fn new(env: &'a RedisTestEnv) -> Self {
        Self { env }
    }

    /// Verify that a key exists in APPL_DB
    pub async fn assert_key_exists(&self, key: &str) -> VerifyResult<()> {
        let exists = self.env.exists(key).await?;
        if !exists {
            return Err(VerificationError::KeyNotFound {
                key: key.to_string(),
            });
        }
        Ok(())
    }

    /// Verify that a key does not exist in APPL_DB
    pub async fn assert_key_not_exists(&self, key: &str) -> VerifyResult<()> {
        let exists = self.env.exists(key).await?;
        if exists {
            return Err(VerificationError::ValueMismatch {
                key: key.to_string(),
                field: "exists".to_string(),
                expected: "false".to_string(),
                actual: "true".to_string(),
            });
        }
        Ok(())
    }

    /// Verify that a hash field has a specific value
    pub async fn assert_field_value(
        &self,
        key: &str,
        field: &str,
        expected: &str,
    ) -> VerifyResult<()> {
        let actual = self.env.hget(key, field).await?;

        match actual {
            None => Err(VerificationError::FieldNotFound {
                key: key.to_string(),
                field: field.to_string(),
            }),
            Some(actual_value) if actual_value == expected => Ok(()),
            Some(actual_value) => Err(VerificationError::ValueMismatch {
                key: key.to_string(),
                field: field.to_string(),
                expected: expected.to_string(),
                actual: actual_value,
            }),
        }
    }

    /// Verify that all fields match expected values
    pub async fn assert_all_fields(
        &self,
        key: &str,
        expected: &HashMap<String, String>,
    ) -> VerifyResult<()> {
        let all_fields = self.env.hgetall(key).await?;
        let actual_map: HashMap<String, String> = all_fields.into_iter().collect();

        for (field, expected_value) in expected {
            match actual_map.get(field) {
                None => {
                    return Err(VerificationError::FieldNotFound {
                        key: key.to_string(),
                        field: field.clone(),
                    })
                }
                Some(actual_value) if actual_value == expected_value => {}
                Some(actual_value) => {
                    return Err(VerificationError::ValueMismatch {
                        key: key.to_string(),
                        field: field.clone(),
                        expected: expected_value.clone(),
                        actual: actual_value.clone(),
                    })
                }
            }
        }

        Ok(())
    }

    /// Verify that a specific number of keys match a pattern
    pub async fn assert_key_count(&self, pattern: &str, expected_count: usize) -> VerifyResult<()> {
        let keys = self.env.keys(pattern).await?;
        let actual_count = keys.len();

        if actual_count != expected_count {
            return Err(VerificationError::KeyCountMismatch {
                pattern: pattern.to_string(),
                expected: expected_count,
                actual: actual_count,
            });
        }

        Ok(())
    }

    /// Get all fields for a key (for inspection)
    pub async fn get_all_fields(&self, key: &str) -> VerifyResult<HashMap<String, String>> {
        let fields = self.env.hgetall(key).await?;
        Ok(fields.into_iter().collect())
    }

    /// Get all keys matching a pattern (for inspection)
    pub async fn get_keys(&self, pattern: &str) -> VerifyResult<Vec<String>> {
        Ok(self.env.keys(pattern).await?)
    }
}

/// Command execution verifier (for mock mode)
pub struct CommandVerifier {
    captured_commands: Vec<String>,
}

impl CommandVerifier {
    /// Create a new command verifier
    pub fn new(captured_commands: Vec<String>) -> Self {
        Self { captured_commands }
    }

    /// Verify that a specific command was executed
    pub fn assert_command_executed(&self, expected: &str) -> VerifyResult<()> {
        if self
            .captured_commands
            .iter()
            .any(|cmd| cmd.contains(expected))
        {
            Ok(())
        } else {
            Err(VerificationError::ValueMismatch {
                key: "command_list".to_string(),
                field: "executed".to_string(),
                expected: expected.to_string(),
                actual: format!("{:?}", self.captured_commands),
            })
        }
    }

    /// Verify that a command was NOT executed
    pub fn assert_command_not_executed(&self, expected: &str) -> VerifyResult<()> {
        if self
            .captured_commands
            .iter()
            .any(|cmd| cmd.contains(expected))
        {
            Err(VerificationError::ValueMismatch {
                key: "command_list".to_string(),
                field: "not_executed".to_string(),
                expected: "not present".to_string(),
                actual: expected.to_string(),
            })
        } else {
            Ok(())
        }
    }

    /// Verify the number of commands executed
    pub fn assert_command_count(&self, expected: usize) -> VerifyResult<()> {
        let actual = self.captured_commands.len();
        if actual != expected {
            Err(VerificationError::KeyCountMismatch {
                pattern: "commands".to_string(),
                expected,
                actual,
            })
        } else {
            Ok(())
        }
    }

    /// Get all captured commands
    pub fn captured_commands(&self) -> &[String] {
        &self.captured_commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_verifier() {
        let commands = vec![
            "systemctl restart hsflowd".to_string(),
            "ip link set dev eth0 mtu 9100".to_string(),
        ];

        let verifier = CommandVerifier::new(commands);

        assert!(verifier.assert_command_executed("hsflowd").is_ok());
        assert!(verifier.assert_command_executed("ip link").is_ok());
        assert!(verifier.assert_command_not_executed("nonexistent").is_ok());
        assert!(verifier.assert_command_count(2).is_ok());

        assert!(verifier.assert_command_count(3).is_err());
        assert!(verifier.assert_command_executed("nonexistent").is_err());
    }
}
