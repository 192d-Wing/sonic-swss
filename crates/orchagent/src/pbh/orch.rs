//! Policy-Based Hashing orchestration logic.

use super::types::{PbhHashEntry, PbhRuleEntry, PbhStats, PbhTableEntry};
use crate::{audit_log, audit::{AuditCategory, AuditOutcome, AuditRecord}};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum PbhOrchError {
    #[error("Hash not found: {0}")]
    HashNotFound(String),
    #[error("Table not found: {0}")]
    TableNotFound(String),
    #[error("Rule not found: {0}")]
    RuleNotFound(String),
    #[error("Invalid priority: {0}")]
    InvalidPriority(u32),
    #[error("Invalid hash field: {0}")]
    InvalidHashField(String),
    #[error("SAI error: {0}")]
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct PbhOrchConfig {
    pub enable_flow_counters: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PbhOrchStats {
    pub stats: PbhStats,
    pub errors: u64,
}

pub trait PbhOrchCallbacks: Send + Sync {
    fn on_hash_created(&self, hash: &PbhHashEntry);
    fn on_hash_removed(&self, hash_name: &str);
    fn on_table_created(&self, table: &PbhTableEntry);
    fn on_table_removed(&self, table_name: &str);
    fn on_rule_created(&self, rule: &PbhRuleEntry);
    fn on_rule_removed(&self, table_name: &str, rule_name: &str);
}

pub struct PbhOrch {
    config: PbhOrchConfig,
    stats: PbhOrchStats,
    hashes: HashMap<String, PbhHashEntry>,
    tables: HashMap<String, PbhTableEntry>,
    rules: HashMap<(String, String), PbhRuleEntry>,
}

impl PbhOrch {
    pub fn new(config: PbhOrchConfig) -> Self {
        Self {
            config,
            stats: PbhOrchStats::default(),
            hashes: HashMap::new(),
            tables: HashMap::new(),
            rules: HashMap::new(),
        }
    }

    pub fn get_hash(&self, name: &str) -> Option<&PbhHashEntry> {
        self.hashes.get(name)
    }

    pub fn get_table(&self, name: &str) -> Option<&PbhTableEntry> {
        self.tables.get(name)
    }

    pub fn get_rule(&self, table_name: &str, rule_name: &str) -> Option<&PbhRuleEntry> {
        self.rules.get(&(table_name.to_string(), rule_name.to_string()))
    }

    pub fn stats(&self) -> &PbhOrchStats {
        &self.stats
    }

    pub fn create_pbh_table(&mut self, name: String) -> Result<(), PbhOrchError> {
        if self.tables.contains_key(&name) {
            let err = PbhOrchError::TableNotFound(name.clone());
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "PbhOrch", "create_pbh_table")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("pbh_table")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        self.stats.stats.tables_created += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "PbhOrch", "create_pbh_table")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("pbh_table")
        );

        Ok(())
    }

    pub fn remove_pbh_table(&mut self, name: &str) -> Result<(), PbhOrchError> {
        if !self.tables.contains_key(name) {
            let err = PbhOrchError::TableNotFound(name.to_string());
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceDelete, "PbhOrch", "remove_pbh_table")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("pbh_table")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        self.tables.remove(name);
        self.stats.stats.tables_created -= 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceDelete, "PbhOrch", "remove_pbh_table")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("pbh_table")
        );

        Ok(())
    }

    pub fn create_pbh_rule(&mut self, table_name: String, rule_name: String) -> Result<(), PbhOrchError> {
        if !self.tables.contains_key(&table_name) {
            let err = PbhOrchError::TableNotFound(table_name);
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "PbhOrch", "create_pbh_rule")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(rule_name)
                    .with_object_type("pbh_rule")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        if self.rules.contains_key(&(table_name.clone(), rule_name.clone())) {
            let err = PbhOrchError::RuleNotFound(rule_name.clone());
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "PbhOrch", "create_pbh_rule")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(format!("{}/{}", table_name, rule_name))
                    .with_object_type("pbh_rule")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        self.stats.stats.rules_created += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "PbhOrch", "create_pbh_rule")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(format!("{}/{}", table_name, rule_name))
                .with_object_type("pbh_rule")
        );

        Ok(())
    }

    pub fn remove_pbh_rule(&mut self, table_name: &str, rule_name: &str) -> Result<(), PbhOrchError> {
        if !self.rules.contains_key(&(table_name.to_string(), rule_name.to_string())) {
            let err = PbhOrchError::RuleNotFound(rule_name.to_string());
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceDelete, "PbhOrch", "remove_pbh_rule")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(format!("{}/{}", table_name, rule_name))
                    .with_object_type("pbh_rule")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        self.rules.remove(&(table_name.to_string(), rule_name.to_string()));
        self.stats.stats.rules_created -= 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceDelete, "PbhOrch", "remove_pbh_rule")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(format!("{}/{}", table_name, rule_name))
                .with_object_type("pbh_rule")
        );

        Ok(())
    }

    pub fn create_pbh_hash(&mut self, name: String) -> Result<(), PbhOrchError> {
        if self.hashes.contains_key(&name) {
            let err = PbhOrchError::HashNotFound(name.clone());
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "PbhOrch", "create_pbh_hash")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("pbh_hash")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        self.stats.stats.hashes_created += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "PbhOrch", "create_pbh_hash")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("pbh_hash")
        );

        Ok(())
    }

    pub fn remove_pbh_hash(&mut self, name: &str) -> Result<(), PbhOrchError> {
        if !self.hashes.contains_key(name) {
            let err = PbhOrchError::HashNotFound(name.to_string());
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceDelete, "PbhOrch", "remove_pbh_hash")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name)
                    .with_object_type("pbh_hash")
                    .with_error(err.to_string())
            );
            return Err(err);
        }

        self.hashes.remove(name);
        self.stats.stats.hashes_created -= 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceDelete, "PbhOrch", "remove_pbh_hash")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("pbh_hash")
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{PbhHashConfig, PbhHashField, PbhPacketAction, PbhRuleConfig, PbhTableConfig};

    #[test]
    fn test_new_pbh_orch_with_default_config() {
        let config = PbhOrchConfig::default();
        let orch = PbhOrch::new(config);

        assert_eq!(orch.stats().stats.hashes_created, 0);
        assert_eq!(orch.stats().stats.tables_created, 0);
        assert_eq!(orch.stats().stats.rules_created, 0);
        assert_eq!(orch.stats().errors, 0);
    }

    #[test]
    fn test_new_pbh_orch_with_flow_counters_enabled() {
        let config = PbhOrchConfig {
            enable_flow_counters: true,
        };
        let orch = PbhOrch::new(config.clone());

        assert_eq!(orch.config.enable_flow_counters, true);
        assert_eq!(orch.stats().errors, 0);
    }

    #[test]
    fn test_get_hash_returns_none_for_nonexistent_hash() {
        let config = PbhOrchConfig::default();
        let orch = PbhOrch::new(config);

        assert!(orch.get_hash("nonexistent").is_none());
    }

    #[test]
    fn test_get_table_returns_none_for_nonexistent_table() {
        let config = PbhOrchConfig::default();
        let orch = PbhOrch::new(config);

        assert!(orch.get_table("nonexistent").is_none());
    }

    #[test]
    fn test_get_rule_returns_none_for_nonexistent_rule() {
        let config = PbhOrchConfig::default();
        let orch = PbhOrch::new(config);

        assert!(orch.get_rule("table1", "rule1").is_none());
    }

    #[test]
    fn test_pbh_orch_error_variants() {
        let err1 = PbhOrchError::HashNotFound("hash1".to_string());
        let err2 = PbhOrchError::TableNotFound("table1".to_string());
        let err3 = PbhOrchError::RuleNotFound("rule1".to_string());
        let err4 = PbhOrchError::InvalidPriority(1000);
        let err5 = PbhOrchError::InvalidHashField("invalid".to_string());
        let err6 = PbhOrchError::SaiError("SAI error".to_string());

        assert!(matches!(err1, PbhOrchError::HashNotFound(_)));
        assert!(matches!(err2, PbhOrchError::TableNotFound(_)));
        assert!(matches!(err3, PbhOrchError::RuleNotFound(_)));
        assert!(matches!(err4, PbhOrchError::InvalidPriority(_)));
        assert!(matches!(err5, PbhOrchError::InvalidHashField(_)));
        assert!(matches!(err6, PbhOrchError::SaiError(_)));
    }

    #[test]
    fn test_pbh_orch_error_clone() {
        let err = PbhOrchError::HashNotFound("test_hash".to_string());
        let cloned = err.clone();

        assert!(matches!(cloned, PbhOrchError::HashNotFound(_)));
    }

    #[test]
    fn test_pbh_orch_config_default() {
        let config = PbhOrchConfig::default();

        assert_eq!(config.enable_flow_counters, false);
    }

    #[test]
    fn test_pbh_orch_config_clone() {
        let config = PbhOrchConfig {
            enable_flow_counters: true,
        };
        let cloned = config.clone();

        assert_eq!(cloned.enable_flow_counters, true);
    }

    #[test]
    fn test_pbh_orch_stats_default() {
        let stats = PbhOrchStats::default();

        assert_eq!(stats.stats.hashes_created, 0);
        assert_eq!(stats.stats.tables_created, 0);
        assert_eq!(stats.stats.rules_created, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_pbh_orch_stats_clone() {
        let mut stats = PbhOrchStats::default();
        stats.errors = 5;
        stats.stats.hashes_created = 10;

        let cloned = stats.clone();

        assert_eq!(cloned.errors, 5);
        assert_eq!(cloned.stats.hashes_created, 10);
    }

    #[test]
    fn test_multiple_pbh_orch_instances() {
        let config1 = PbhOrchConfig {
            enable_flow_counters: true,
        };
        let config2 = PbhOrchConfig::default();

        let orch1 = PbhOrch::new(config1);
        let orch2 = PbhOrch::new(config2);

        assert_eq!(orch1.config.enable_flow_counters, true);
        assert_eq!(orch2.config.enable_flow_counters, false);
    }
}
