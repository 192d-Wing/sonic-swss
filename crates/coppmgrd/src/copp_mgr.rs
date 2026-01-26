//! CoPP Manager - Core implementation

use async_trait::async_trait;
use sonic_cfgmgr_common::{CfgMgr, CfgMgrResult, FieldValues, FieldValuesExt, WarmRestartState};
use sonic_orch_common::Orch;
use tracing::{debug, info};

use crate::tables::*;
use crate::types::*;

/// CoPP Manager
///
/// Manages Control Plane Policing configuration including:
/// - Trap groups (policer settings)
/// - Trap IDs mapped to groups
/// - Feature-based trap enable/disable
pub struct CoppMgr {
    /// Trap name → trap configuration
    trap_conf_map: CoppTrapConfMap,

    /// Trap ID → group name mapping
    trap_id_group_map: CoppTrapIdGroupMap,

    /// Group → (field → value) nested map for current group config
    group_fvs: CoppGroupFvs,

    /// Feature → field values for FEATURE table cache
    features_cfg: FeaturesCfg,

    /// Init trap configuration from JSON file
    trap_init_cfg: CoppCfg,

    /// Init group configuration from JSON file
    group_init_cfg: CoppCfg,

    /// Path to CoPP config file
    copp_cfg_file: String,

    #[cfg(test)]
    mock_mode: bool,
}

impl CoppMgr {
    /// Create a new CoppMgr with parsed init config
    pub fn new(trap_init_cfg: CoppCfg, group_init_cfg: CoppCfg, copp_cfg_file: String) -> Self {
        info!(
            "CoppMgr initialized with {} trap entries, {} group entries from {}",
            trap_init_cfg.len(),
            group_init_cfg.len(),
            copp_cfg_file
        );

        Self {
            trap_conf_map: CoppTrapConfMap::new(),
            trap_id_group_map: CoppTrapIdGroupMap::new(),
            group_fvs: CoppGroupFvs::new(),
            features_cfg: FeaturesCfg::new(),
            trap_init_cfg,
            group_init_cfg,
            copp_cfg_file,
            #[cfg(test)]
            mock_mode: false,
        }
    }

    #[cfg(test)]
    pub fn new_mock(
        trap_init_cfg: CoppCfg,
        group_init_cfg: CoppCfg,
        copp_cfg_file: String,
    ) -> Self {
        let mut mgr = Self::new(trap_init_cfg, group_init_cfg, copp_cfg_file);
        mgr.mock_mode = true;
        mgr
    }

    /// Check if trap group has all traps disabled (pending state)
    ///
    /// A trap group is "pending" if:
    /// - It has at least one trap ID mapped to it, AND
    /// - All those trap IDs are disabled
    ///
    /// When pending, the group should not be written to APPL_DB
    fn check_trap_group_pending(&self, trap_group: &str) -> bool {
        let mut traps_present = false;

        for (trap_id, group) in &self.trap_id_group_map {
            if group == trap_group {
                traps_present = true;

                // At least one trap is enabled → not pending
                if !self.is_trap_id_disabled(trap_id) {
                    return false;
                }
            }
        }

        traps_present // Pending if has traps but all disabled
    }

    /// Check if trap ID is disabled
    ///
    /// A trap ID is disabled if:
    /// - Its trap is NOT always_enabled, AND
    /// - Its associated feature is not enabled
    fn is_trap_id_disabled(&self, trap_id: &str) -> bool {
        // Find trap name containing this trap_id
        for (trap_name, conf) in &self.trap_conf_map {
            if conf.trap_ids.contains(trap_id) {
                // Check always_enabled first
                if conf.is_always_enabled {
                    return false;
                }

                // Check feature state
                if self.is_feature_enabled(trap_name) {
                    return false;
                }
            }
        }

        true
    }

    /// Check if feature is enabled
    ///
    /// Feature is enabled if state = "enabled" or "always_enabled"
    fn is_feature_enabled(&self, feature: &str) -> bool {
        if let Some(fvs) = self.features_cfg.get(feature) {
            if let Some(state) = fvs.get_field(feature_fields::STATE) {
                return state == "enabled" || state == "always_enabled";
            }
        }
        false
    }

    /// Get aggregated trap IDs for a trap group
    ///
    /// Returns comma-separated list of all enabled trap IDs in the group
    fn get_trap_group_trap_ids(&self, trap_group: &str) -> String {
        let mut trap_ids = Vec::new();

        for (trap_id, group) in &self.trap_id_group_map {
            if group == trap_group && !self.is_trap_id_disabled(trap_id) {
                trap_ids.push(trap_id.as_str());
            }
        }

        trap_ids.join(",")
    }

    /// Add trap IDs to group mapping
    ///
    /// Parses comma-separated trap_ids string and maps each ID to the group
    fn add_trap_ids_to_group(&mut self, trap_group: &str, trap_ids: &str) {
        for trap_id in trap_ids.split(',') {
            let trap_id = trap_id.trim();
            if !trap_id.is_empty() {
                debug!("Mapping trap ID {} to group {}", trap_id, trap_group);
                self.trap_id_group_map
                    .insert(trap_id.to_string(), trap_group.to_string());
            }
        }
    }

    /// Remove trap IDs from group mapping
    fn remove_trap_ids_from_group(&mut self, trap_ids: &str) {
        for trap_id in trap_ids.split(',') {
            let trap_id = trap_id.trim();
            if !trap_id.is_empty() {
                debug!("Removing trap ID {} from group mapping", trap_id);
                self.trap_id_group_map.remove(trap_id);
            }
        }
    }

    /// Add trap to a group
    ///
    /// Updates trap_id_group_map and writes to APPL_DB if group is not pending
    fn add_trap(&mut self, trap_ids: &str, trap_group: &str) {
        self.add_trap_ids_to_group(trap_group, trap_ids);

        let trap_group_trap_ids = self.get_trap_group_trap_ids(trap_group);

        if !self.check_trap_group_pending(trap_group) {
            info!(
                "Adding trap {} to group {} (total trap_ids: {})",
                trap_ids, trap_group, trap_group_trap_ids
            );

            // TODO: Write to APPL_DB
            // m_appCoppTable.set(trap_group, fvs);
            // setCoppGroupStateOk(trap_group);
        } else {
            debug!(
                "Trap group {} is pending, not writing to APPL_DB",
                trap_group
            );
        }
    }

    /// Remove trap from a group
    ///
    /// Updates trap_id_group_map and updates APPL_DB
    fn remove_trap(&mut self, key: &str) {
        if let Some(conf) = self.trap_conf_map.get(key) {
            let trap_group = conf.trap_group.clone();
            let trap_ids = conf.trap_ids.clone();

            self.remove_trap_ids_from_group(&trap_ids);

            let remaining_trap_ids = self.get_trap_group_trap_ids(&trap_group);

            if !self.check_trap_group_pending(&trap_group) {
                info!(
                    "Removing trap {} from group {} (remaining trap_ids: {})",
                    key, trap_group, remaining_trap_ids
                );

                // TODO: Write to APPL_DB
                // m_appCoppTable.set(trap_group, fvs);
                // setCoppGroupStateOk(trap_group);
            }
        }
    }

    /// Set feature trap IDs status based on feature enable/disable
    ///
    /// Called when FEATURE table is updated
    pub fn set_feature_trap_ids_status(&mut self, feature: &str, enable: bool) {
        // Check if this feature has a trap config
        let (always_enabled, trap_group) = if let Some(conf) = self.trap_conf_map.get(feature) {
            (conf.is_always_enabled, conf.trap_group.clone())
        } else {
            return; // No trap config for this feature
        };

        // Determine if trap should be disabled
        let disabled_trap = !always_enabled && !self.is_feature_enabled(feature);

        // Check current and desired state
        if (enable && !disabled_trap) || (!enable && disabled_trap) {
            return; // Already in desired state
        }

        let prev_group_state = self.check_trap_group_pending(&trap_group);

        // Update features cache
        let state = if enable { "enabled" } else { "disabled" };
        if let Some(fvs) = self.features_cfg.get_mut(feature) {
            for (field, value) in fvs.iter_mut() {
                if field == feature_fields::STATE {
                    *value = state.to_string();
                }
            }
        }

        // Handle trap group state changes
        if self.check_trap_group_pending(&trap_group) && !prev_group_state {
            // Group moved to pending → remove from APPL_DB
            info!(
                "Trap group {} moved to pending state, removing from APPL_DB",
                trap_group
            );
            // TODO: m_appCoppTable.del(trap_group);
            // TODO: delCoppGroupStateOk(trap_group);
        } else if prev_group_state && !self.check_trap_group_pending(&trap_group) {
            // Group moved from pending → add to APPL_DB
            info!(
                "Trap group {} moved from pending to enabled, adding to APPL_DB",
                trap_group
            );

            let _trap_ids = self.get_trap_group_trap_ids(&trap_group);
            // TODO: Build fvs with group fields + _trap_ids
            // TODO: m_appCoppTable.set(trap_group, fvs);
            // TODO: setCoppGroupStateOk(trap_group);
        } else if !self.check_trap_group_pending(&trap_group) {
            // Group is not pending, just update trap_ids
            let trap_ids = self.get_trap_group_trap_ids(&trap_group);
            info!(
                "Updating trap_ids for group {} to: {}",
                trap_group, trap_ids
            );
            // TODO: m_appCoppTable.set(trap_group, fvs with updated trap_ids);
            // TODO: setCoppGroupStateOk(trap_group);
        }
    }

    /// Handle COPP_TRAP table updates
    pub async fn do_copp_trap_task(
        &mut self,
        _key: &str,
        _op: &str,
        _values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        // TODO: Implement SET/DEL logic from C++ lines 531-809
        // This is complex trap management logic
        Ok(true)
    }

    /// Handle COPP_GROUP table updates
    pub async fn do_copp_group_task(
        &mut self,
        _key: &str,
        _op: &str,
        _values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        // TODO: Implement SET/DEL logic from C++ lines 840-925
        Ok(true)
    }

    /// Handle FEATURE table updates
    pub async fn do_feature_task(
        &mut self,
        key: &str,
        op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        if op == "SET" {
            // Initialize or update feature state
            if !self.features_cfg.contains_key(key) {
                self.features_cfg
                    .insert(key.to_string(), vec![("state".to_string(), "".to_string())]);
            }

            for (field, value) in values {
                if field == feature_fields::STATE {
                    let status = value == "enabled" || value == "always_enabled";
                    self.set_feature_trap_ids_status(key, status);
                }
            }
        } else if op == "DEL" {
            self.set_feature_trap_ids_status(key, false);
        }

        Ok(true)
    }
}

#[async_trait]
impl Orch for CoppMgr {
    fn name(&self) -> &str {
        "coppmgr"
    }

    async fn do_task(&mut self) {
        // TODO: Process consumers when integrating with ConsumerStateTable
    }
}

#[async_trait]
impl CfgMgr for CoppMgr {
    fn daemon_name(&self) -> &str {
        "coppmgrd"
    }

    fn is_warm_restart(&self) -> bool {
        // TODO: Implement warm restart detection
        false
    }

    fn warm_restart_state(&self) -> WarmRestartState {
        WarmRestartState::Disabled
    }

    async fn set_warm_restart_state(&mut self, _state: WarmRestartState) {
        // TODO: Write to STATE_DB WARM_RESTART_TABLE
    }

    fn config_table_names(&self) -> &[&str] {
        &[CFG_COPP_TRAP_TABLE, CFG_COPP_GROUP_TABLE, CFG_FEATURE_TABLE]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fvs(items: &[(&str, &str)]) -> FieldValues {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_copp_mgr_new() {
        let trap_cfg = CoppCfg::new();
        let group_cfg = CoppCfg::new();
        let mgr = CoppMgr::new(trap_cfg, group_cfg, COPP_INIT_FILE.to_string());

        assert_eq!(mgr.trap_conf_map.len(), 0);
        assert_eq!(mgr.trap_id_group_map.len(), 0);
    }

    #[test]
    fn test_add_trap_ids_to_group() {
        let mut mgr = CoppMgr::new_mock(CoppCfg::new(), CoppCfg::new(), COPP_INIT_FILE.to_string());

        mgr.add_trap_ids_to_group("queue1", "arp_req,arp_resp,neigh_discovery");

        assert_eq!(mgr.trap_id_group_map.len(), 3);
        assert_eq!(
            mgr.trap_id_group_map.get("arp_req"),
            Some(&"queue1".to_string())
        );
        assert_eq!(
            mgr.trap_id_group_map.get("arp_resp"),
            Some(&"queue1".to_string())
        );
        assert_eq!(
            mgr.trap_id_group_map.get("neigh_discovery"),
            Some(&"queue1".to_string())
        );
    }

    #[test]
    fn test_remove_trap_ids_from_group() {
        let mut mgr = CoppMgr::new_mock(CoppCfg::new(), CoppCfg::new(), COPP_INIT_FILE.to_string());

        mgr.add_trap_ids_to_group("queue1", "arp_req,arp_resp");
        assert_eq!(mgr.trap_id_group_map.len(), 2);

        mgr.remove_trap_ids_from_group("arp_req");
        assert_eq!(mgr.trap_id_group_map.len(), 1);
        assert!(!mgr.trap_id_group_map.contains_key("arp_req"));
        assert!(mgr.trap_id_group_map.contains_key("arp_resp"));
    }

    #[test]
    fn test_get_trap_group_trap_ids_all_enabled() {
        let mut mgr = CoppMgr::new_mock(CoppCfg::new(), CoppCfg::new(), COPP_INIT_FILE.to_string());

        // Add trap configuration (always enabled)
        mgr.trap_conf_map.insert(
            "arp".to_string(),
            CoppTrapConf::new("arp_req,arp_resp".to_string(), "queue1".to_string(), true),
        );

        mgr.add_trap_ids_to_group("queue1", "arp_req,arp_resp");

        let trap_ids = mgr.get_trap_group_trap_ids("queue1");
        assert!(trap_ids.contains("arp_req"));
        assert!(trap_ids.contains("arp_resp"));
    }

    #[test]
    fn test_is_trap_id_disabled_always_enabled() {
        let mut mgr = CoppMgr::new_mock(CoppCfg::new(), CoppCfg::new(), COPP_INIT_FILE.to_string());

        mgr.trap_conf_map.insert(
            "bgp".to_string(),
            CoppTrapConf::new("bgp,bgpv6".to_string(), "queue4".to_string(), true),
        );

        // always_enabled → never disabled
        assert!(!mgr.is_trap_id_disabled("bgp"));
        assert!(!mgr.is_trap_id_disabled("bgpv6"));
    }

    #[test]
    fn test_is_trap_id_disabled_feature_disabled() {
        let mut mgr = CoppMgr::new_mock(CoppCfg::new(), CoppCfg::new(), COPP_INIT_FILE.to_string());

        mgr.trap_conf_map.insert(
            "arp".to_string(),
            CoppTrapConf::new("arp_req".to_string(), "queue1".to_string(), false),
        );

        // No feature config → disabled
        assert!(mgr.is_trap_id_disabled("arp_req"));

        // Add disabled feature
        mgr.features_cfg
            .insert("arp".to_string(), make_fvs(&[("state", "disabled")]));
        assert!(mgr.is_trap_id_disabled("arp_req"));

        // Enable feature
        mgr.features_cfg
            .insert("arp".to_string(), make_fvs(&[("state", "enabled")]));
        assert!(!mgr.is_trap_id_disabled("arp_req"));
    }

    #[test]
    fn test_check_trap_group_pending() {
        let mut mgr = CoppMgr::new_mock(CoppCfg::new(), CoppCfg::new(), COPP_INIT_FILE.to_string());

        // Add trap (not always enabled, no feature)
        mgr.trap_conf_map.insert(
            "arp".to_string(),
            CoppTrapConf::new("arp_req,arp_resp".to_string(), "queue1".to_string(), false),
        );
        mgr.add_trap_ids_to_group("queue1", "arp_req,arp_resp");

        // All traps disabled → pending
        assert!(mgr.check_trap_group_pending("queue1"));

        // Enable feature → not pending
        mgr.features_cfg
            .insert("arp".to_string(), make_fvs(&[("state", "enabled")]));
        assert!(!mgr.check_trap_group_pending("queue1"));
    }

    #[tokio::test]
    async fn test_do_feature_task() {
        let mut mgr = CoppMgr::new_mock(CoppCfg::new(), CoppCfg::new(), COPP_INIT_FILE.to_string());

        // Add trap config
        mgr.trap_conf_map.insert(
            "arp".to_string(),
            CoppTrapConf::new("arp_req".to_string(), "queue1".to_string(), false),
        );

        let values = make_fvs(&[("state", "enabled")]);
        mgr.do_feature_task("arp", "SET", &values).await.unwrap();

        assert!(mgr.is_feature_enabled("arp"));
    }
}
