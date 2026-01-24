//! Buffer orchestration logic.

use super::types::{BufferPoolEntry, BufferProfileEntry, BufferStats};
use std::collections::HashMap;
use thiserror::Error;
use crate::audit_log;

#[derive(Debug, Clone, Error)]
pub enum BufferOrchError {
    #[error("Pool not found: {0}")]
    PoolNotFound(String),
    #[error("Profile not found: {0}")]
    ProfileNotFound(String),
    #[error("Invalid threshold: {0}")]
    InvalidThreshold(String),
    #[error("SAI error: {0}")]
    SaiError(String),
    #[error("Reference count error: {0}")]
    RefCountError(String),
}

#[derive(Debug, Clone, Default)]
pub struct BufferOrchConfig {
    pub enable_ingress_buffer_drop: bool,
    pub enable_egress_buffer_drop: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BufferOrchStats {
    pub stats: BufferStats,
    pub errors: u64,
}

pub trait BufferOrchCallbacks: Send + Sync {
    fn on_pool_created(&self, pool: &BufferPoolEntry);
    fn on_pool_removed(&self, pool_name: &str);
    fn on_profile_created(&self, profile: &BufferProfileEntry);
    fn on_profile_removed(&self, profile_name: &str);
}

pub struct BufferOrch {
    config: BufferOrchConfig,
    stats: BufferOrchStats,
    pools: HashMap<String, BufferPoolEntry>,
    profiles: HashMap<String, BufferProfileEntry>,
}

impl BufferOrch {
    pub fn new(config: BufferOrchConfig) -> Self {
        Self {
            config,
            stats: BufferOrchStats::default(),
            pools: HashMap::new(),
            profiles: HashMap::new(),
        }
    }

    pub fn get_pool(&self, name: &str) -> Option<&BufferPoolEntry> {
        self.pools.get(name)
    }

    pub fn get_pool_mut(&mut self, name: &str) -> Option<&mut BufferPoolEntry> {
        self.pools.get_mut(name)
    }

    pub fn add_pool(&mut self, entry: BufferPoolEntry) -> Result<(), BufferOrchError> {
        let name = entry.name.clone();

        if self.pools.contains_key(&name) {
            let record = crate::audit::AuditRecord::new(
                crate::audit::AuditCategory::ErrorCondition,
                "BufferOrch",
                format!("create_buffer_pool_failed: {}", name),
            )
            .with_outcome(crate::audit::AuditOutcome::Failure)
            .with_object_id(&name)
            .with_object_type("buffer_pool")
            .with_error("Pool already exists");
            audit_log!(record);
            return Err(BufferOrchError::SaiError("Pool already exists".to_string()));
        }

        self.stats.stats.pools_created = self.stats.stats.pools_created.saturating_add(1);
        self.pools.insert(name.clone(), entry.clone());

        let record = crate::audit::AuditRecord::new(
            crate::audit::AuditCategory::ResourceCreate,
            "BufferOrch",
            format!("create_buffer_pool: {}", name),
        )
        .with_outcome(crate::audit::AuditOutcome::Success)
        .with_object_id(&name)
        .with_object_type("buffer_pool")
        .with_details(serde_json::json!({
            "pool_name": name,
            "size": entry.config.size,
            "pool_type": format!("{:?}", entry.config.pool_type),
            "mode": format!("{:?}", entry.config.mode),
            "sai_oid": entry.sai_oid,
        }));
        audit_log!(record);

        Ok(())
    }

    pub fn remove_pool(&mut self, name: &str) -> Result<BufferPoolEntry, BufferOrchError> {
        let entry = self.pools.get(name)
            .ok_or_else(|| {
                audit_log!(
                    resource_id: name,
                    action: "delete_buffer_pool",
                    category: "ResourceDelete",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Pool not found",
                        "pool_name": name,
                    })
                );
                BufferOrchError::PoolNotFound(name.to_string())
            })?;

        if entry.ref_count > 0 {
            audit_log!(
                resource_id: name,
                action: "delete_buffer_pool",
                category: "ResourceDelete",
                outcome: "FAIL",
                details: serde_json::json!({
                    "error": "Pool has references",
                    "pool_name": name,
                    "ref_count": entry.ref_count,
                })
            );
            return Err(BufferOrchError::RefCountError(
                format!("Pool {} still has {} references", name, entry.ref_count)
            ));
        }

        let removed = self.pools.remove(name)
            .ok_or_else(|| BufferOrchError::PoolNotFound(name.to_string()))?;

        audit_log!(
            resource_id: name,
            action: "delete_buffer_pool",
            category: "ResourceDelete",
            outcome: "SUCCESS",
            details: serde_json::json!({
                "pool_name": name,
                "size": removed.config.size,
                "pool_type": format!("{:?}", removed.config.pool_type),
            })
        );

        Ok(removed)
    }

    pub fn increment_pool_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let pool = self.pools.get_mut(name)
            .ok_or_else(|| {
                audit_log!(
                    resource_id: name,
                    action: "create_priority_group",
                    category: "ResourceCreate",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Pool not found",
                        "pool_name": name,
                    })
                );
                BufferOrchError::PoolNotFound(name.to_string())
            })?;
        let new_count = pool.add_ref();

        audit_log!(
            resource_id: name,
            action: "create_priority_group",
            category: "ResourceCreate",
            outcome: "SUCCESS",
            details: serde_json::json!({
                "pool_name": name,
                "ref_count": new_count,
            })
        );

        Ok(new_count)
    }

    pub fn decrement_pool_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let pool = self.pools.get_mut(name)
            .ok_or_else(|| {
                audit_log!(
                    resource_id: name,
                    action: "update_pg_configuration",
                    category: "ResourceModify",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Pool not found",
                        "pool_name": name,
                    })
                );
                BufferOrchError::PoolNotFound(name.to_string())
            })?;
        pool.remove_ref()
            .map(|new_count| {
                audit_log!(
                    resource_id: name,
                    action: "update_pg_configuration",
                    category: "ResourceModify",
                    outcome: "SUCCESS",
                    details: serde_json::json!({
                        "pool_name": name,
                        "ref_count": new_count,
                    })
                );
                new_count
            })
            .map_err(|e| {
                audit_log!(
                    resource_id: name,
                    action: "update_pg_configuration",
                    category: "ResourceModify",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": e,
                        "pool_name": name,
                    })
                );
                BufferOrchError::RefCountError(e)
            })
    }

    pub fn get_profile(&self, name: &str) -> Option<&BufferProfileEntry> {
        self.profiles.get(name)
    }

    pub fn get_profile_mut(&mut self, name: &str) -> Option<&mut BufferProfileEntry> {
        self.profiles.get_mut(name)
    }

    pub fn add_profile(&mut self, entry: BufferProfileEntry) -> Result<(), BufferOrchError> {
        let name = entry.name.clone();

        if self.profiles.contains_key(&name) {
            audit_log!(
                resource_id: &name,
                action: "create_buffer_profile",
                category: "ResourceCreate",
                outcome: "FAIL",
                details: serde_json::json!({
                    "error": "Profile already exists",
                    "profile_name": name,
                })
            );
            return Err(BufferOrchError::SaiError("Profile already exists".to_string()));
        }

        // Verify pool exists
        if !self.pools.contains_key(&entry.config.pool_name) {
            audit_log!(
                resource_id: &name,
                action: "create_buffer_profile",
                category: "ResourceCreate",
                outcome: "FAIL",
                details: serde_json::json!({
                    "error": "Referenced pool not found",
                    "profile_name": name,
                    "pool_name": entry.config.pool_name,
                })
            );
            return Err(BufferOrchError::PoolNotFound(entry.config.pool_name.clone()));
        }

        self.stats.stats.profiles_created = self.stats.stats.profiles_created.saturating_add(1);
        self.profiles.insert(name.clone(), entry.clone());

        audit_log!(
            resource_id: &name,
            action: "create_buffer_profile",
            category: "ResourceCreate",
            outcome: "SUCCESS",
            details: serde_json::json!({
                "profile_name": name,
                "pool_name": entry.config.pool_name,
                "size": entry.config.size,
                "sai_oid": entry.sai_oid,
            })
        );

        Ok(())
    }

    pub fn remove_profile(&mut self, name: &str) -> Result<BufferProfileEntry, BufferOrchError> {
        let entry = self.profiles.get(name)
            .ok_or_else(|| {
                audit_log!(
                    resource_id: name,
                    action: "delete_buffer_profile",
                    category: "ResourceDelete",
                    outcome: "FAIL",
                    details: serde_json::json!({
                        "error": "Profile not found",
                        "profile_name": name,
                    })
                );
                BufferOrchError::ProfileNotFound(name.to_string())
            })?;

        if entry.ref_count > 0 {
            audit_log!(
                resource_id: name,
                action: "delete_buffer_profile",
                category: "ResourceDelete",
                outcome: "FAIL",
                details: serde_json::json!({
                    "error": "Profile has references",
                    "profile_name": name,
                    "ref_count": entry.ref_count,
                })
            );
            return Err(BufferOrchError::RefCountError(
                format!("Profile {} still has {} references", name, entry.ref_count)
            ));
        }

        let removed = self.profiles.remove(name)
            .ok_or_else(|| BufferOrchError::ProfileNotFound(name.to_string()))?;

        audit_log!(
            resource_id: name,
            action: "delete_buffer_profile",
            category: "ResourceDelete",
            outcome: "SUCCESS",
            details: serde_json::json!({
                "profile_name": name,
                "pool_name": removed.config.pool_name,
                "size": removed.config.size,
            })
        );

        Ok(removed)
    }

    pub fn increment_profile_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let profile = self.profiles.get_mut(name)
            .ok_or_else(|| BufferOrchError::ProfileNotFound(name.to_string()))?;
        Ok(profile.add_ref())
    }

    pub fn decrement_profile_ref(&mut self, name: &str) -> Result<u32, BufferOrchError> {
        let profile = self.profiles.get_mut(name)
            .ok_or_else(|| BufferOrchError::ProfileNotFound(name.to_string()))?;
        profile.remove_ref()
            .map_err(|e| BufferOrchError::RefCountError(e))
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    pub fn stats(&self) -> &BufferOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{BufferPoolConfig, BufferProfileConfig};

    fn create_test_pool(name: &str, size: u64) -> BufferPoolEntry {
        BufferPoolEntry {
            name: name.to_string(),
            config: BufferPoolConfig {
                pool_type: super::super::types::BufferPoolType::Ingress,
                mode: super::super::types::BufferPoolMode::Dynamic,
                size,
                threshold_mode: super::super::types::ThresholdMode::Dynamic,
                xoff_threshold: None,
                xon_threshold: None,
            },
            sai_oid: 0,
            ref_count: 0,
        }
    }

    fn create_test_profile(name: &str, pool_name: &str, size: u64) -> BufferProfileEntry {
        BufferProfileEntry {
            name: name.to_string(),
            config: super::super::types::BufferProfileConfig {
                pool_name: pool_name.to_string(),
                size,
                dynamic_threshold: None,
                static_threshold: None,
                xoff_threshold: None,
                xon_threshold: None,
                xon_offset: None,
            },
            sai_oid: 0,
            ref_count: 0,
        }
    }

    #[test]
    fn test_add_pool() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);

        orch.add_pool(pool).unwrap();
        assert_eq!(orch.pool_count(), 1);
        assert_eq!(orch.stats().stats.pools_created, 1);
    }

    #[test]
    fn test_add_duplicate_pool() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool1 = create_test_pool("ingress_lossless_pool", 10485760);
        let pool2 = create_test_pool("ingress_lossless_pool", 20971520);

        orch.add_pool(pool1).unwrap();
        let result = orch.add_pool(pool2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BufferOrchError::SaiError(_)));
    }

    #[test]
    fn test_remove_pool() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);

        orch.add_pool(pool).unwrap();
        let removed = orch.remove_pool("ingress_lossless_pool").unwrap();
        assert_eq!(removed.config.size, 10485760);
        assert_eq!(orch.pool_count(), 0);
    }

    #[test]
    fn test_remove_pool_with_references() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);

        orch.add_pool(pool).unwrap();
        orch.increment_pool_ref("ingress_lossless_pool").unwrap();

        let result = orch.remove_pool("ingress_lossless_pool");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BufferOrchError::RefCountError(_)));
    }

    #[test]
    fn test_pool_ref_counting() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);

        orch.add_pool(pool).unwrap();

        let count1 = orch.increment_pool_ref("ingress_lossless_pool").unwrap();
        assert_eq!(count1, 1);

        let count2 = orch.increment_pool_ref("ingress_lossless_pool").unwrap();
        assert_eq!(count2, 2);

        let count3 = orch.decrement_pool_ref("ingress_lossless_pool").unwrap();
        assert_eq!(count3, 1);

        let count4 = orch.decrement_pool_ref("ingress_lossless_pool").unwrap();
        assert_eq!(count4, 0);
    }

    #[test]
    fn test_pool_ref_underflow() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);

        orch.add_pool(pool).unwrap();

        let result = orch.decrement_pool_ref("ingress_lossless_pool");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BufferOrchError::RefCountError(_)));
    }

    #[test]
    fn test_add_profile() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);
        let profile = create_test_profile("pg_lossless_profile", "ingress_lossless_pool", 1024);

        orch.add_pool(pool).unwrap();
        orch.add_profile(profile).unwrap();
        assert_eq!(orch.profile_count(), 1);
        assert_eq!(orch.stats().stats.profiles_created, 1);
    }

    #[test]
    fn test_add_profile_without_pool() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let profile = create_test_profile("pg_lossless_profile", "missing_pool", 1024);

        let result = orch.add_profile(profile);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BufferOrchError::PoolNotFound(_)));
    }

    #[test]
    fn test_remove_profile() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);
        let profile = create_test_profile("pg_lossless_profile", "ingress_lossless_pool", 1024);

        orch.add_pool(pool).unwrap();
        orch.add_profile(profile).unwrap();

        let removed = orch.remove_profile("pg_lossless_profile").unwrap();
        assert_eq!(removed.config.size, 1024);
        assert_eq!(orch.profile_count(), 0);
    }

    #[test]
    fn test_remove_profile_with_references() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);
        let profile = create_test_profile("pg_lossless_profile", "ingress_lossless_pool", 1024);

        orch.add_pool(pool).unwrap();
        orch.add_profile(profile).unwrap();
        orch.increment_profile_ref("pg_lossless_profile").unwrap();

        let result = orch.remove_profile("pg_lossless_profile");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BufferOrchError::RefCountError(_)));
    }

    #[test]
    fn test_profile_ref_counting() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);
        let profile = create_test_profile("pg_lossless_profile", "ingress_lossless_pool", 1024);

        orch.add_pool(pool).unwrap();
        orch.add_profile(profile).unwrap();

        let count1 = orch.increment_profile_ref("pg_lossless_profile").unwrap();
        assert_eq!(count1, 1);

        let count2 = orch.increment_profile_ref("pg_lossless_profile").unwrap();
        assert_eq!(count2, 2);

        let count3 = orch.decrement_profile_ref("pg_lossless_profile").unwrap();
        assert_eq!(count3, 1);
    }

    #[test]
    fn test_profile_ref_underflow() {
        let mut orch = BufferOrch::new(BufferOrchConfig::default());
        let pool = create_test_pool("ingress_lossless_pool", 10485760);
        let profile = create_test_profile("pg_lossless_profile", "ingress_lossless_pool", 1024);

        orch.add_pool(pool).unwrap();
        orch.add_profile(profile).unwrap();

        let result = orch.decrement_profile_ref("pg_lossless_profile");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BufferOrchError::RefCountError(_)));
    }
}
