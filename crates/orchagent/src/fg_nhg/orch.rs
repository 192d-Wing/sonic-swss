//! Fine-Grained Next Hop Group orchestration logic.

use super::types::{FgNhgEntry, FgNhgPrefix, FgNhgStats};
use std::collections::HashMap;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};
use crate::audit_log;

#[derive(Debug, Clone)]
pub enum FgNhgOrchError {
    NhgNotFound(FgNhgPrefix),
    InvalidBucketSize(u32),
    InvalidWeight(u32),
    MemberNotFound(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct FgNhgOrchConfig {
    pub default_bucket_size: u32,
    pub enable_rebalancing: bool,
}

impl FgNhgOrchConfig {
    pub fn with_bucket_size(mut self, size: u32) -> Self {
        self.default_bucket_size = size;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct FgNhgOrchStats {
    pub stats: FgNhgStats,
    pub errors: u64,
}

pub trait FgNhgOrchCallbacks: Send + Sync {
    fn on_nhg_created(&self, entry: &FgNhgEntry);
    fn on_nhg_removed(&self, prefix: &FgNhgPrefix);
    fn on_member_added(&self, prefix: &FgNhgPrefix, member_ip: &str);
    fn on_member_removed(&self, prefix: &FgNhgPrefix, member_ip: &str);
}

pub struct FgNhgOrch {
    config: FgNhgOrchConfig,
    stats: FgNhgOrchStats,
    nhgs: HashMap<FgNhgPrefix, FgNhgEntry>,
}

impl FgNhgOrch {
    pub fn new(config: FgNhgOrchConfig) -> Self {
        Self {
            config,
            stats: FgNhgOrchStats::default(),
            nhgs: HashMap::new(),
        }
    }

    pub fn get_nhg(&self, prefix: &FgNhgPrefix) -> Option<&FgNhgEntry> {
        self.nhgs.get(prefix)
    }

    pub fn stats(&self) -> &FgNhgOrchStats {
        &self.stats
    }

    /// Create a fine-grained next hop group with audit logging.
    pub fn create_fg_nhg(&mut self, prefix: FgNhgPrefix, entry: FgNhgEntry) -> Result<(), FgNhgOrchError> {
        if self.nhgs.contains_key(&prefix) {
            let record = AuditRecord::new(
                AuditCategory::ErrorCondition,
                "FgNhgOrch",
                format!("create_fg_nhg_failed: {}", prefix.ip_prefix),
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&prefix.ip_prefix)
            .with_object_type("fg_nhg")
            .with_error("NHG already exists");
            audit_log!(record);

            return Err(FgNhgOrchError::NhgNotFound(prefix));
        }

        let member_count = entry.next_hops.len();
        let total_weight: u32 = entry.next_hops.iter().map(|nh| nh.weight).sum();

        self.nhgs.insert(prefix.clone(), entry);
        self.stats.stats.nhgs_created += 1;

        let record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "FgNhgOrch",
            format!("create_fg_nhg: {}", prefix.ip_prefix),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&prefix.ip_prefix)
        .with_object_type("fg_nhg")
        .with_details(serde_json::json!({
            "bucket_size": self.config.default_bucket_size,
            "member_count": member_count,
            "total_weight": total_weight,
        }));
        audit_log!(record);

        Ok(())
    }

    /// Update a fine-grained next hop group with audit logging.
    pub fn update_fg_nhg(&mut self, prefix: FgNhgPrefix, entry: FgNhgEntry) -> Result<(), FgNhgOrchError> {
        let old_entry = self.nhgs.get(&prefix)
            .ok_or_else(|| FgNhgOrchError::NhgNotFound(prefix.clone()))?;

        let old_member_count = old_entry.next_hops.len();
        let new_member_count = entry.next_hops.len();

        self.nhgs.insert(prefix.clone(), entry);
        self.stats.stats.members_added += (new_member_count - old_member_count.min(new_member_count)) as u64;

        let record = AuditRecord::new(
            AuditCategory::ResourceModify,
            "FgNhgOrch",
            format!("update_fg_nhg: {}", prefix.ip_prefix),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&prefix.ip_prefix)
        .with_object_type("fg_nhg")
        .with_details(serde_json::json!({
            "old_member_count": old_member_count,
            "new_member_count": new_member_count,
        }));
        audit_log!(record);

        Ok(())
    }

    /// Remove a fine-grained next hop group with audit logging.
    pub fn remove_fg_nhg(&mut self, prefix: &FgNhgPrefix) -> Result<(), FgNhgOrchError> {
        let entry = self.nhgs.remove(prefix)
            .ok_or_else(|| {
                let record = AuditRecord::new(
                    AuditCategory::ErrorCondition,
                    "FgNhgOrch",
                    format!("remove_fg_nhg_failed: {}", prefix.ip_prefix),
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(&prefix.ip_prefix)
                .with_object_type("fg_nhg")
                .with_error("NHG not found");
                audit_log!(record);

                FgNhgOrchError::NhgNotFound(prefix.clone())
            })?;

        self.stats.stats.nhgs_created -= 1;
        self.stats.stats.members_added -= entry.next_hops.len() as u64;

        let record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "FgNhgOrch",
            format!("remove_fg_nhg: {}", prefix.ip_prefix),
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&prefix.ip_prefix)
        .with_object_type("fg_nhg")
        .with_details(serde_json::json!({
            "member_count": entry.next_hops.len(),
        }));
        audit_log!(record);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fg_nhg_orch_new_default_config() {
        let config = FgNhgOrchConfig::default();
        let orch = FgNhgOrch::new(config);

        assert_eq!(orch.stats.stats.nhgs_created, 0);
        assert_eq!(orch.stats.stats.members_added, 0);
        assert_eq!(orch.stats.errors, 0);
        assert_eq!(orch.nhgs.len(), 0);
    }

    #[test]
    fn test_fg_nhg_orch_new_with_config() {
        let config = FgNhgOrchConfig {
            default_bucket_size: 128,
            enable_rebalancing: true,
        };
        let orch = FgNhgOrch::new(config);

        assert_eq!(orch.stats().errors, 0);
    }

    #[test]
    fn test_fg_nhg_orch_config_with_bucket_size() {
        let config = FgNhgOrchConfig::default().with_bucket_size(256);

        assert_eq!(config.default_bucket_size, 256);
    }

    #[test]
    fn test_fg_nhg_orch_get_nhg_not_found() {
        let orch = FgNhgOrch::new(FgNhgOrchConfig::default());
        let prefix = FgNhgPrefix::new("192.168.0.0/24".to_string());

        assert!(orch.get_nhg(&prefix).is_none());
    }

    #[test]
    fn test_fg_nhg_orch_stats_access() {
        let orch = FgNhgOrch::new(FgNhgOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.stats.nhgs_created, 0);
        assert_eq!(stats.stats.members_added, 0);
        assert_eq!(stats.stats.rebalances, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_fg_nhg_orch_empty_initialization() {
        let orch = FgNhgOrch::new(FgNhgOrchConfig::default());

        assert_eq!(orch.nhgs.len(), 0);
        let prefix = FgNhgPrefix::new("10.0.0.0/8".to_string());
        assert!(orch.get_nhg(&prefix).is_none());
    }

    #[test]
    fn test_fg_nhg_orch_config_clone() {
        let config1 = FgNhgOrchConfig {
            default_bucket_size: 64,
            enable_rebalancing: false,
        };
        let config2 = config1.clone();

        assert_eq!(config1.default_bucket_size, config2.default_bucket_size);
        assert_eq!(config1.enable_rebalancing, config2.enable_rebalancing);
    }

    #[test]
    fn test_fg_nhg_orch_stats_default() {
        let stats = FgNhgOrchStats::default();

        assert_eq!(stats.stats.nhgs_created, 0);
        assert_eq!(stats.stats.members_added, 0);
        assert_eq!(stats.stats.rebalances, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_fg_nhg_orch_stats_clone() {
        let mut stats1 = FgNhgOrchStats::default();
        stats1.errors = 5;
        stats1.stats.nhgs_created = 10;

        let stats2 = stats1.clone();

        assert_eq!(stats1.errors, stats2.errors);
        assert_eq!(stats1.stats.nhgs_created, stats2.stats.nhgs_created);
    }

    #[test]
    fn test_fg_nhg_orch_error_nhg_not_found() {
        let prefix = FgNhgPrefix::new("192.168.1.0/24".to_string());
        let error = FgNhgOrchError::NhgNotFound(prefix.clone());

        match error {
            FgNhgOrchError::NhgNotFound(p) => {
                assert_eq!(p.ip_prefix, prefix.ip_prefix);
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_fg_nhg_orch_error_invalid_bucket_size() {
        let error = FgNhgOrchError::InvalidBucketSize(0);

        match error {
            FgNhgOrchError::InvalidBucketSize(size) => {
                assert_eq!(size, 0);
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_fg_nhg_orch_error_invalid_weight() {
        let error = FgNhgOrchError::InvalidWeight(100);

        match error {
            FgNhgOrchError::InvalidWeight(weight) => {
                assert_eq!(weight, 100);
            }
            _ => panic!("Wrong error type"),
        }
    }
}
