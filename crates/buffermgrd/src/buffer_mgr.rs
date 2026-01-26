//! Buffer Manager - Core buffer profile and PG management

use async_trait::async_trait;
use sonic_cfgmgr_common::{
    CfgMgr, CfgMgrResult, FieldValues, FieldValuesExt, WarmRestartState,
};
use sonic_orch_common::Orch;
use tracing::info;

use crate::pg_bitmap::{generate_pg_combinations, pfc_to_bitmap};
use crate::tables::*;
use crate::types::*;

/// Buffer Manager
///
/// Manages buffer profiles and buffer PG assignments based on port speed,
/// cable length, and PFC configuration.
pub struct BufferMgr {
    /// PG profile lookup table loaded from file
    pg_profile_lookup: PgProfileLookup,

    /// Cable length per port
    cable_len_lookup: PortCableLength,

    /// Speed per port
    speed_lookup: PortSpeed,

    /// PFC enable status per port (e.g., "3,4")
    port_pfc_status: PortPfcStatus,

    /// Admin status per port ("up" or "down")
    port_status_lookup: PortAdminStatus,

    /// Platform type
    platform: Platform,

    /// Whether lookup file was successfully processed
    pgfile_processed: bool,

    /// Dynamic buffer model flag
    dynamic_buffer_model: bool,

    #[cfg(test)]
    mock_mode: bool,
}

impl BufferMgr {
    /// Create a new BufferMgr with parsed PG profile lookup
    pub fn new(pg_profile_lookup: PgProfileLookup) -> Self {
        let platform = Platform::from_env();
        let pgfile_processed = !pg_profile_lookup.is_empty();
        info!("BufferMgr initialized on platform: {:?}", platform);

        Self {
            pg_profile_lookup,
            cable_len_lookup: PortCableLength::new(),
            speed_lookup: PortSpeed::new(),
            port_pfc_status: PortPfcStatus::new(),
            port_status_lookup: PortAdminStatus::new(),
            platform,
            pgfile_processed,
            dynamic_buffer_model: false,
            #[cfg(test)]
            mock_mode: false,
        }
    }

    #[cfg(test)]
    pub fn new_mock(pg_profile_lookup: PgProfileLookup) -> Self {
        let mut mgr = Self::new(pg_profile_lookup);
        mgr.mock_mode = true;
        mgr
    }

    /// Handle cable length update for a port
    pub fn do_cable_task(&mut self, port: &str, cable_length: &str) -> CfgMgrResult<bool> {
        self.cable_len_lookup
            .insert(port.to_string(), cable_length.to_string());
        info!("Cable length set to {} for port {}", cable_length, port);
        Ok(true)
    }

    /// Handle speed update for a port - generates buffer profiles
    pub async fn do_speed_update_task(&mut self, port: &str) -> CfgMgrResult<bool> {
        // Check if cable length is available
        let cable = match self.cable_len_lookup.get(port) {
            Some(c) => c.clone(),
            None => {
                info!(
                    "Unable to create/update PG profile for port {}. Cable length is not set",
                    port
                );
                return Ok(false); // Retry later
            }
        };

        // Skip if cable is 0m (no buffer config needed)
        if cable == "0m" {
            info!(
                "Not creating/updating PG profile for port {}. Cable length is set to {}",
                port, cable
            );
            return Ok(true);
        }

        // Check if admin status is available
        if !self.port_status_lookup.contains_key(port) {
            info!("Admin status is not available for port {}", port);
            return Ok(false); // Retry later
        }

        // Check if PFC status is available
        let pfc_enable = match self.port_pfc_status.get(port) {
            Some(p) => p.clone(),
            None => {
                info!("PFC enable status is not available for port {}", port);
                return Ok(true); // Not an error, just not ready
            }
        };

        let speed = self.speed_lookup.get(port).cloned().unwrap_or_default();

        // Create buffer profile key
        let buffer_profile_key = format!("pg_lossless_{}_{}_ profile", speed, cable);

        // Convert PFC enable to bitmap and generate PG combinations
        let lossless_pg_bitmap = pfc_to_bitmap(&pfc_enable);
        let lossless_pg_combinations = generate_pg_combinations(lossless_pg_bitmap);

        // Platform-specific: skip if port is down on Mellanox/Barefoot
        if self.port_status_lookup.get(port) == Some(&"down".to_string())
            && self.platform.is_mellanox_or_barefoot()
        {
            info!(
                "Port {} is down on {:?} platform, skipping buffer profile creation",
                port, self.platform
            );
            return Ok(true);
        }

        // TODO: Get PG profile from lookup
        // TODO: Write buffer profile to APPL_DB
        // TODO: Write buffer PG entries to APPL_DB for each PG combination

        info!(
            "Would create buffer profile {} for port {} with PG combinations: {:?}",
            buffer_profile_key, port, lossless_pg_combinations
        );

        Ok(true)
    }

    /// Get buffer pool mode
    pub fn get_pg_pool_mode(&self) -> Option<String> {
        // TODO: Read from CONFIG_DB BUFFER_POOL table
        // For now, return None
        None
    }

    /// Handle PORT table updates (speed, admin_status)
    pub async fn do_port_task(
        &mut self,
        port: &str,
        _op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        // Update speed if present
        if let Some(speed) = values.get_field(port_fields::SPEED) {
            self.speed_lookup
                .insert(port.to_string(), speed.to_string());
            info!("Port {} speed set to {}", port, speed);
        }

        // Update admin status if present
        if let Some(status) = values.get_field(port_fields::ADMIN_STATUS) {
            self.port_status_lookup
                .insert(port.to_string(), status.to_string());
            info!("Port {} admin_status set to {}", port, status);
        }

        // Trigger speed update task to regenerate profiles
        self.do_speed_update_task(port).await
    }

    /// Handle PORT_QOS_MAP table updates (PFC enable)
    pub async fn do_port_qos_task(
        &mut self,
        port: &str,
        _op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        if let Some(pfc_enable) = values.get_field(qos_map_fields::PFC_ENABLE) {
            self.port_pfc_status
                .insert(port.to_string(), pfc_enable.to_string());
            info!("Port {} PFC enable set to {}", port, pfc_enable);

            // Trigger speed update task to regenerate profiles
            return self.do_speed_update_task(port).await;
        }

        Ok(true)
    }

    /// Handle CABLE_LENGTH table updates
    pub async fn do_cable_length_task(
        &mut self,
        _key: &str,
        _op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        // Key can be a port name or "AZURE" (global)
        // Values are port -> cable length mappings
        for (port, cable_length) in values {
            self.do_cable_task(port, cable_length)?;

            // Trigger speed update for this port if speed is known
            if self.speed_lookup.contains_key(port) {
                self.do_speed_update_task(port).await?;
            }
        }

        Ok(true)
    }

    /// Handle generic buffer table passthrough to APPL_DB
    pub fn do_buffer_table_task(
        &mut self,
        _table_name: &str,
        _key: &str,
        _op: &str,
        _values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        // TODO: Implement passthrough to APPL_DB tables
        // BUFFER_PROFILE, BUFFER_PG, BUFFER_POOL, etc.
        Ok(true)
    }
}

impl Default for BufferMgr {
    fn default() -> Self {
        Self::new(PgProfileLookup::new())
    }
}

#[async_trait]
impl Orch for BufferMgr {
    fn name(&self) -> &str {
        "buffermgr"
    }

    async fn do_task(&mut self) {
        // TODO: Process consumers
        // This will be implemented when integrating with ConsumerStateTable
    }
}

#[async_trait]
impl CfgMgr for BufferMgr {
    fn daemon_name(&self) -> &str {
        "buffermgrd"
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
        &[
            CFG_PORT_TABLE,
            CFG_PORT_CABLE_LEN_TABLE,
            CFG_PORT_QOS_MAP_TABLE,
            CFG_BUFFER_PROFILE_TABLE,
            CFG_BUFFER_PG_TABLE,
            CFG_BUFFER_POOL_TABLE,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_lookup() -> PgProfileLookup {
        let mut lookup = PgProfileLookup::new();
        let mut speed_map = std::collections::HashMap::new();

        speed_map.insert(
            "5m".to_string(),
            PgProfile {
                size: "34816".to_string(),
                xon: "18432".to_string(),
                xoff: "16384".to_string(),
                threshold: "1".to_string(),
                xon_offset: "2496".to_string(),
            },
        );

        lookup.insert("40000".to_string(), speed_map);
        lookup
    }

    #[test]
    fn test_buffer_mgr_new() {
        let lookup = make_test_lookup();
        let mgr = BufferMgr::new(lookup);

        assert!(mgr.pgfile_processed);
        assert!(!mgr.dynamic_buffer_model);
        assert!(mgr.cable_len_lookup.is_empty());
    }

    #[test]
    fn test_do_cable_task() {
        let lookup = make_test_lookup();
        let mut mgr = BufferMgr::new_mock(lookup);

        let result = mgr.do_cable_task("Ethernet0", "5m").unwrap();
        assert!(result);
        assert_eq!(
            mgr.cable_len_lookup.get("Ethernet0"),
            Some(&"5m".to_string())
        );
    }

    #[tokio::test]
    async fn test_do_speed_update_task_no_cable() {
        let lookup = make_test_lookup();
        let mut mgr = BufferMgr::new_mock(lookup);

        // No cable set yet
        let result = mgr.do_speed_update_task("Ethernet0").await.unwrap();
        assert!(!result); // Should return false (retry later)
    }

    #[tokio::test]
    async fn test_do_speed_update_task_cable_0m() {
        let lookup = make_test_lookup();
        let mut mgr = BufferMgr::new_mock(lookup);

        mgr.do_cable_task("Ethernet0", "0m").unwrap();

        let result = mgr.do_speed_update_task("Ethernet0").await.unwrap();
        assert!(result); // Should return true (no config needed)
    }

    #[tokio::test]
    async fn test_do_speed_update_task_no_admin_status() {
        let lookup = make_test_lookup();
        let mut mgr = BufferMgr::new_mock(lookup);

        mgr.do_cable_task("Ethernet0", "5m").unwrap();

        // No admin status set
        let result = mgr.do_speed_update_task("Ethernet0").await.unwrap();
        assert!(!result); // Should return false (retry later)
    }

    #[tokio::test]
    async fn test_do_speed_update_task_platform_specific() {
        let lookup = make_test_lookup();
        let mut mgr = BufferMgr::new_mock(lookup);

        // Set up Mellanox platform
        std::env::set_var("ASIC_VENDOR", "mellanox");
        mgr.platform = Platform::from_env();

        mgr.do_cable_task("Ethernet0", "5m").unwrap();
        mgr.port_status_lookup
            .insert("Ethernet0".to_string(), "down".to_string());
        mgr.port_pfc_status
            .insert("Ethernet0".to_string(), "3,4".to_string());
        mgr.speed_lookup
            .insert("Ethernet0".to_string(), "40000".to_string());

        let result = mgr.do_speed_update_task("Ethernet0").await.unwrap();
        assert!(result); // Should skip due to down port on Mellanox
    }

    #[tokio::test]
    async fn test_do_port_task() {
        let lookup = make_test_lookup();
        let mut mgr = BufferMgr::new_mock(lookup);

        let values = vec![
            ("speed".to_string(), "40000".to_string()),
            ("admin_status".to_string(), "up".to_string()),
        ];

        mgr.do_port_task("Ethernet0", "SET", &values).await.unwrap();

        assert_eq!(
            mgr.speed_lookup.get("Ethernet0"),
            Some(&"40000".to_string())
        );
        assert_eq!(
            mgr.port_status_lookup.get("Ethernet0"),
            Some(&"up".to_string())
        );
    }

    #[tokio::test]
    async fn test_do_port_qos_task() {
        let lookup = make_test_lookup();
        let mut mgr = BufferMgr::new_mock(lookup);

        let values = vec![("pfc_enable".to_string(), "3,4".to_string())];

        mgr.do_port_qos_task("Ethernet0", "SET", &values)
            .await
            .unwrap();

        assert_eq!(
            mgr.port_pfc_status.get("Ethernet0"),
            Some(&"3,4".to_string())
        );
    }
}
