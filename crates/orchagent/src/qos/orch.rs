//! QoS orchestration logic.

use super::types::{QosMapEntry, QosStats, SchedulerEntry, WredProfile};
use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum QosOrchError {
    MapNotFound(String),
    SchedulerNotFound(String),
    WredNotFound(String),
    InvalidMapping(u8, u8),
    InvalidWeight(u8),
    InvalidThreshold(u32),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct QosOrchConfig {
    pub enable_wred: bool,
    pub enable_ecn: bool,
}

#[derive(Debug, Clone, Default)]
pub struct QosOrchStats {
    pub stats: QosStats,
    pub errors: u64,
}

pub trait QosOrchCallbacks: Send + Sync {
    fn on_map_created(&self, map: &QosMapEntry);
    fn on_map_removed(&self, map_name: &str);
    fn on_scheduler_created(&self, scheduler: &SchedulerEntry);
    fn on_scheduler_removed(&self, scheduler_name: &str);
    fn on_wred_profile_created(&self, profile: &WredProfile);
    fn on_wred_profile_removed(&self, profile_name: &str);
}

pub struct QosOrch {
    config: QosOrchConfig,
    stats: QosOrchStats,
    qos_maps: HashMap<String, QosMapEntry>,
    schedulers: HashMap<String, SchedulerEntry>,
    wred_profiles: HashMap<String, WredProfile>,
}

impl QosOrch {
    pub fn new(config: QosOrchConfig) -> Self {
        Self {
            config,
            stats: QosOrchStats::default(),
            qos_maps: HashMap::new(),
            schedulers: HashMap::new(),
            wred_profiles: HashMap::new(),
        }
    }

    pub fn get_map(&self, name: &str) -> Option<&QosMapEntry> {
        self.qos_maps.get(name)
    }

    pub fn get_map_mut(&mut self, name: &str) -> Option<&mut QosMapEntry> {
        self.qos_maps.get_mut(name)
    }

    pub fn add_map(&mut self, entry: QosMapEntry) -> Result<(), QosOrchError> {
        let name = entry.name.clone();

        if self.qos_maps.contains_key(&name) {
            let error_msg = "QoS map already exists".to_string();
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "QosOrch", "add_map")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&name)
                    .with_object_type("qos_map")
                    .with_error(&error_msg)
            );
            return Err(QosOrchError::SaiError(error_msg));
        }

        // Validate mappings (TC/Queue typically 0-7, DSCP 0-63)
        for (&from, &to) in &entry.mappings {
            if from > 63 || to > 63 {
                let error_msg = format!("Invalid mapping: {} -> {}", from, to);
                audit_log!(
                    AuditRecord::new(AuditCategory::ResourceCreate, "QosOrch", "add_map")
                        .with_outcome(AuditOutcome::Failure)
                        .with_object_id(&name)
                        .with_object_type("qos_map")
                        .with_error(&error_msg)
                );
                return Err(QosOrchError::InvalidMapping(from, to));
            }
        }

        self.stats.stats.maps_created = self.stats.stats.maps_created.saturating_add(1);
        self.qos_maps.insert(name.clone(), entry);

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "QosOrch", "add_map")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(&name)
                .with_object_type("qos_map")
        );

        Ok(())
    }

    pub fn remove_map(&mut self, name: &str) -> Result<QosMapEntry, QosOrchError> {
        self.qos_maps
            .remove(name)
            .ok_or_else(|| {
                audit_log!(AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "QosOrch",
                    "remove_map"
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(name)
                .with_object_type("qos_map")
                .with_error(&format!("QoS map not found: {}", name)));
                QosOrchError::MapNotFound(name.to_string())
            })
            .map(|entry| {
                audit_log!(AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "QosOrch",
                    "remove_map"
                )
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("qos_map"));
                entry
            })
    }

    pub fn update_map_mapping(&mut self, name: &str, from: u8, to: u8) -> Result<(), QosOrchError> {
        if from > 63 || to > 63 {
            return Err(QosOrchError::InvalidMapping(from, to));
        }

        let map = self
            .qos_maps
            .get_mut(name)
            .ok_or_else(|| QosOrchError::MapNotFound(name.to_string()))?;

        map.add_mapping(from, to);
        Ok(())
    }

    pub fn get_scheduler(&self, name: &str) -> Option<&SchedulerEntry> {
        self.schedulers.get(name)
    }

    pub fn add_scheduler(&mut self, entry: SchedulerEntry) -> Result<(), QosOrchError> {
        let name = entry.name.clone();

        if self.schedulers.contains_key(&name) {
            let error_msg = "Scheduler already exists".to_string();
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "QosOrch", "add_scheduler")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&name)
                    .with_object_type("scheduler")
                    .with_error(&error_msg)
            );
            return Err(QosOrchError::SaiError(error_msg));
        }

        // Validate weight (typically 1-100)
        if entry.config.weight == 0 {
            let error_msg = format!("Invalid weight: {}", entry.config.weight);
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "QosOrch", "add_scheduler")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&name)
                    .with_object_type("scheduler")
                    .with_error(&error_msg)
            );
            return Err(QosOrchError::InvalidWeight(entry.config.weight));
        }

        self.stats.stats.schedulers_created = self.stats.stats.schedulers_created.saturating_add(1);
        self.schedulers.insert(name.clone(), entry);

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "QosOrch", "add_scheduler")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(&name)
                .with_object_type("scheduler")
        );

        Ok(())
    }

    pub fn remove_scheduler(&mut self, name: &str) -> Result<SchedulerEntry, QosOrchError> {
        self.schedulers
            .remove(name)
            .ok_or_else(|| {
                audit_log!(AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "QosOrch",
                    "remove_scheduler"
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(name)
                .with_object_type("scheduler")
                .with_error(&format!("Scheduler not found: {}", name)));
                QosOrchError::SchedulerNotFound(name.to_string())
            })
            .map(|entry| {
                audit_log!(AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "QosOrch",
                    "remove_scheduler"
                )
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name)
                .with_object_type("scheduler"));
                entry
            })
    }

    pub fn get_wred_profile(&self, name: &str) -> Option<&WredProfile> {
        self.wred_profiles.get(name)
    }

    pub fn add_wred_profile(&mut self, profile: WredProfile) -> Result<(), QosOrchError> {
        let name = profile.name.clone();

        if self.wred_profiles.contains_key(&name) {
            return Err(QosOrchError::SaiError(
                "WRED profile already exists".to_string(),
            ));
        }

        // Validate thresholds
        if let (Some(min), Some(max)) = (profile.green_min_threshold, profile.green_max_threshold) {
            if min > max {
                return Err(QosOrchError::InvalidThreshold(min));
            }
        }

        if let (Some(min), Some(max)) = (profile.yellow_min_threshold, profile.yellow_max_threshold)
        {
            if min > max {
                return Err(QosOrchError::InvalidThreshold(min));
            }
        }

        if let (Some(min), Some(max)) = (profile.red_min_threshold, profile.red_max_threshold) {
            if min > max {
                return Err(QosOrchError::InvalidThreshold(min));
            }
        }

        self.stats.stats.wred_profiles_created =
            self.stats.stats.wred_profiles_created.saturating_add(1);
        self.wred_profiles.insert(name, profile);

        Ok(())
    }

    pub fn remove_wred_profile(&mut self, name: &str) -> Result<WredProfile, QosOrchError> {
        self.wred_profiles
            .remove(name)
            .ok_or_else(|| QosOrchError::WredNotFound(name.to_string()))
    }

    pub fn map_count(&self) -> usize {
        self.qos_maps.len()
    }

    pub fn scheduler_count(&self) -> usize {
        self.schedulers.len()
    }

    pub fn wred_profile_count(&self) -> usize {
        self.wred_profiles.len()
    }

    pub fn stats(&self) -> &QosOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{QosMapType, SchedulerConfig, SchedulerType};
    use super::*;

    fn create_test_qos_map(name: &str) -> QosMapEntry {
        let mut map = QosMapEntry::new(name.to_string(), QosMapType::DscpToTc);
        map.add_mapping(0, 0);
        map.add_mapping(8, 1);
        map
    }

    fn create_test_scheduler(name: &str, weight: u8) -> SchedulerEntry {
        SchedulerEntry::new(
            name.to_string(),
            SchedulerConfig {
                scheduler_type: SchedulerType::Dwrr,
                weight,
                meter_type: None,
                cir: None,
                cbs: None,
                pir: None,
                pbs: None,
            },
        )
    }

    fn create_test_wred_profile(name: &str) -> WredProfile {
        let mut profile = WredProfile::new(name.to_string());
        profile.green_enable = true;
        profile.green_min_threshold = Some(1000);
        profile.green_max_threshold = Some(2000);
        profile
    }

    #[test]
    fn test_add_map() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let map = create_test_qos_map("dscp_to_tc_map");

        assert_eq!(orch.map_count(), 0);
        assert_eq!(orch.stats().stats.maps_created, 0);

        orch.add_map(map).unwrap();
        assert_eq!(orch.map_count(), 1);
        assert_eq!(orch.stats().stats.maps_created, 1);

        let retrieved = orch.get_map("dscp_to_tc_map").unwrap();
        assert_eq!(retrieved.name, "dscp_to_tc_map");
        assert_eq!(retrieved.mappings.len(), 2);
    }

    #[test]
    fn test_add_map_invalid_mapping() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let mut map = QosMapEntry::new("invalid_map".to_string(), QosMapType::DscpToTc);

        // DSCP values must be 0-63
        map.add_mapping(64, 0); // Invalid: from value > 63

        let result = orch.add_map(map);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidMapping(64, 0)
        ));
        assert_eq!(orch.map_count(), 0);
    }

    #[test]
    fn test_add_map_invalid_mapping_to_value() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let mut map = QosMapEntry::new("invalid_map".to_string(), QosMapType::DscpToTc);

        // Both from and to values must be 0-63
        map.add_mapping(0, 64); // Invalid: to value > 63

        let result = orch.add_map(map);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidMapping(0, 64)
        ));
        assert_eq!(orch.map_count(), 0);
    }

    #[test]
    fn test_add_map_duplicate() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let map1 = create_test_qos_map("dscp_to_tc_map");
        let map2 = create_test_qos_map("dscp_to_tc_map");

        orch.add_map(map1).unwrap();
        let result = orch.add_map(map2);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), QosOrchError::SaiError(_)));
        assert_eq!(orch.map_count(), 1);
    }

    #[test]
    fn test_update_map_mapping() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let map = create_test_qos_map("dscp_to_tc_map");

        orch.add_map(map).unwrap();

        // Update an existing mapping
        orch.update_map_mapping("dscp_to_tc_map", 0, 7).unwrap();

        let updated_map = orch.get_map("dscp_to_tc_map").unwrap();
        assert_eq!(updated_map.mappings.get(&0), Some(&7));

        // Add a new mapping
        orch.update_map_mapping("dscp_to_tc_map", 16, 2).unwrap();
        let updated_map = orch.get_map("dscp_to_tc_map").unwrap();
        assert_eq!(updated_map.mappings.get(&16), Some(&2));
        assert_eq!(updated_map.mappings.len(), 3);
    }

    #[test]
    fn test_update_map_mapping_not_found() {
        let mut orch = QosOrch::new(QosOrchConfig::default());

        let result = orch.update_map_mapping("nonexistent_map", 0, 0);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), QosOrchError::MapNotFound(_)));
    }

    #[test]
    fn test_update_map_mapping_invalid() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let map = create_test_qos_map("dscp_to_tc_map");
        orch.add_map(map).unwrap();

        // Invalid from value
        let result = orch.update_map_mapping("dscp_to_tc_map", 64, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidMapping(64, 0)
        ));

        // Invalid to value
        let result = orch.update_map_mapping("dscp_to_tc_map", 0, 64);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidMapping(0, 64)
        ));
    }

    #[test]
    fn test_remove_map() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let map = create_test_qos_map("dscp_to_tc_map");

        orch.add_map(map).unwrap();
        assert_eq!(orch.map_count(), 1);

        let removed = orch.remove_map("dscp_to_tc_map").unwrap();
        assert_eq!(removed.name, "dscp_to_tc_map");
        assert_eq!(orch.map_count(), 0);
    }

    #[test]
    fn test_remove_map_not_found() {
        let mut orch = QosOrch::new(QosOrchConfig::default());

        let result = orch.remove_map("nonexistent_map");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), QosOrchError::MapNotFound(_)));
    }

    #[test]
    fn test_add_scheduler() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let scheduler = create_test_scheduler("scheduler0", 10);

        assert_eq!(orch.scheduler_count(), 0);
        assert_eq!(orch.stats().stats.schedulers_created, 0);

        orch.add_scheduler(scheduler).unwrap();
        assert_eq!(orch.scheduler_count(), 1);
        assert_eq!(orch.stats().stats.schedulers_created, 1);

        let retrieved = orch.get_scheduler("scheduler0").unwrap();
        assert_eq!(retrieved.name, "scheduler0");
        assert_eq!(retrieved.config.weight, 10);
    }

    #[test]
    fn test_add_scheduler_invalid_weight() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let scheduler = create_test_scheduler("scheduler0", 0); // Invalid: weight must be > 0

        let result = orch.add_scheduler(scheduler);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidWeight(0)
        ));
        assert_eq!(orch.scheduler_count(), 0);
    }

    #[test]
    fn test_add_scheduler_duplicate() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let scheduler1 = create_test_scheduler("scheduler0", 10);
        let scheduler2 = create_test_scheduler("scheduler0", 20);

        orch.add_scheduler(scheduler1).unwrap();
        let result = orch.add_scheduler(scheduler2);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), QosOrchError::SaiError(_)));
        assert_eq!(orch.scheduler_count(), 1);
    }

    #[test]
    fn test_remove_scheduler() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let scheduler = create_test_scheduler("scheduler0", 10);

        orch.add_scheduler(scheduler).unwrap();
        assert_eq!(orch.scheduler_count(), 1);

        let removed = orch.remove_scheduler("scheduler0").unwrap();
        assert_eq!(removed.name, "scheduler0");
        assert_eq!(orch.scheduler_count(), 0);
    }

    #[test]
    fn test_remove_scheduler_not_found() {
        let mut orch = QosOrch::new(QosOrchConfig::default());

        let result = orch.remove_scheduler("nonexistent_scheduler");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::SchedulerNotFound(_)
        ));
    }

    #[test]
    fn test_add_wred_profile() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let profile = create_test_wred_profile("wred_profile0");

        assert_eq!(orch.wred_profile_count(), 0);
        assert_eq!(orch.stats().stats.wred_profiles_created, 0);

        orch.add_wred_profile(profile).unwrap();
        assert_eq!(orch.wred_profile_count(), 1);
        assert_eq!(orch.stats().stats.wred_profiles_created, 1);

        let retrieved = orch.get_wred_profile("wred_profile0").unwrap();
        assert_eq!(retrieved.name, "wred_profile0");
        assert_eq!(retrieved.green_enable, true);
        assert_eq!(retrieved.green_min_threshold, Some(1000));
        assert_eq!(retrieved.green_max_threshold, Some(2000));
    }

    #[test]
    fn test_wred_profile_invalid_green_thresholds() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let mut profile = WredProfile::new("wred_profile0".to_string());
        profile.green_enable = true;
        profile.green_min_threshold = Some(2000);
        profile.green_max_threshold = Some(1000); // Invalid: min > max

        let result = orch.add_wred_profile(profile);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidThreshold(2000)
        ));
        assert_eq!(orch.wred_profile_count(), 0);
    }

    #[test]
    fn test_wred_profile_invalid_yellow_thresholds() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let mut profile = WredProfile::new("wred_profile0".to_string());
        profile.yellow_enable = true;
        profile.yellow_min_threshold = Some(3000);
        profile.yellow_max_threshold = Some(2000); // Invalid: min > max

        let result = orch.add_wred_profile(profile);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidThreshold(3000)
        ));
        assert_eq!(orch.wred_profile_count(), 0);
    }

    #[test]
    fn test_wred_profile_invalid_red_thresholds() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let mut profile = WredProfile::new("wred_profile0".to_string());
        profile.red_enable = true;
        profile.red_min_threshold = Some(4000);
        profile.red_max_threshold = Some(3000); // Invalid: min > max

        let result = orch.add_wred_profile(profile);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QosOrchError::InvalidThreshold(4000)
        ));
        assert_eq!(orch.wred_profile_count(), 0);
    }

    #[test]
    fn test_wred_profile_valid_thresholds_all_colors() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let mut profile = WredProfile::new("wred_profile0".to_string());

        // All colors with valid thresholds
        profile.green_enable = true;
        profile.green_min_threshold = Some(1000);
        profile.green_max_threshold = Some(2000);

        profile.yellow_enable = true;
        profile.yellow_min_threshold = Some(1500);
        profile.yellow_max_threshold = Some(2500);

        profile.red_enable = true;
        profile.red_min_threshold = Some(2000);
        profile.red_max_threshold = Some(3000);

        orch.add_wred_profile(profile).unwrap();
        assert_eq!(orch.wred_profile_count(), 1);
    }

    #[test]
    fn test_wred_profile_equal_thresholds() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let mut profile = WredProfile::new("wred_profile0".to_string());
        profile.green_enable = true;
        profile.green_min_threshold = Some(1000);
        profile.green_max_threshold = Some(1000); // Valid: min == max is allowed

        orch.add_wred_profile(profile).unwrap();
        assert_eq!(orch.wred_profile_count(), 1);
    }

    #[test]
    fn test_remove_wred_profile() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let profile = create_test_wred_profile("wred_profile0");

        orch.add_wred_profile(profile).unwrap();
        assert_eq!(orch.wred_profile_count(), 1);

        let removed = orch.remove_wred_profile("wred_profile0").unwrap();
        assert_eq!(removed.name, "wred_profile0");
        assert_eq!(orch.wred_profile_count(), 0);
    }

    #[test]
    fn test_remove_wred_profile_not_found() {
        let mut orch = QosOrch::new(QosOrchConfig::default());

        let result = orch.remove_wred_profile("nonexistent_profile");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), QosOrchError::WredNotFound(_)));
    }

    #[test]
    fn test_multiple_maps_different_types() {
        let mut orch = QosOrch::new(QosOrchConfig::default());

        let mut map1 = QosMapEntry::new("dscp_to_tc".to_string(), QosMapType::DscpToTc);
        map1.add_mapping(0, 0);

        let mut map2 = QosMapEntry::new("dscp_to_queue".to_string(), QosMapType::DscpToQueue);
        map2.add_mapping(8, 1);

        orch.add_map(map1).unwrap();
        orch.add_map(map2).unwrap();

        assert_eq!(orch.map_count(), 2);
        assert_eq!(orch.stats().stats.maps_created, 2);
    }

    #[test]
    fn test_scheduler_weight_boundary() {
        let mut orch = QosOrch::new(QosOrchConfig::default());

        // Weight = 1 should be valid (minimum valid value)
        let scheduler1 = create_test_scheduler("scheduler1", 1);
        orch.add_scheduler(scheduler1).unwrap();
        assert_eq!(orch.scheduler_count(), 1);

        // Weight = 255 should be valid (maximum u8 value)
        let scheduler2 = create_test_scheduler("scheduler2", 255);
        orch.add_scheduler(scheduler2).unwrap();
        assert_eq!(orch.scheduler_count(), 2);
    }

    #[test]
    fn test_get_map_mut() {
        let mut orch = QosOrch::new(QosOrchConfig::default());
        let map = create_test_qos_map("dscp_to_tc_map");
        orch.add_map(map).unwrap();

        // Get mutable reference and modify
        {
            let map_mut = orch.get_map_mut("dscp_to_tc_map").unwrap();
            map_mut.add_mapping(16, 3);
        }

        // Verify the change
        let map = orch.get_map("dscp_to_tc_map").unwrap();
        assert_eq!(map.mappings.get(&16), Some(&3));
        assert_eq!(map.mappings.len(), 3);
    }

    #[test]
    fn test_get_map_not_found() {
        let orch = QosOrch::new(QosOrchConfig::default());
        assert!(orch.get_map("nonexistent").is_none());
    }

    #[test]
    fn test_get_scheduler_not_found() {
        let orch = QosOrch::new(QosOrchConfig::default());
        assert!(orch.get_scheduler("nonexistent").is_none());
    }

    #[test]
    fn test_get_wred_profile_not_found() {
        let orch = QosOrch::new(QosOrchConfig::default());
        assert!(orch.get_wred_profile("nonexistent").is_none());
    }
}
