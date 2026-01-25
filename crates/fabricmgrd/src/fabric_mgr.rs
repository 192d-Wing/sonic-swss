//! FabricMgr - Core fabric monitoring configuration manager implementation

use async_trait::async_trait;
use tracing::{debug, instrument};

use sonic_cfgmgr_common::{CfgMgr, CfgMgrResult, FieldValues, Orch};

use crate::fields;
use crate::{
    CFG_FABRIC_MONITOR_DATA_TABLE_NAME, CFG_FABRIC_MONITOR_PORT_TABLE_NAME, FABRIC_MONITOR_DATA_KEY,
};

/// FabricMgr manages fabric monitoring configuration
///
/// Configuration flow:
/// 1. FABRIC_MONITOR table → APP_FABRIC_MONITOR_DATA table
/// 2. FABRIC_PORT table → APP_FABRIC_PORT_TABLE
///
/// This is a pure pass-through manager with no shell commands.
pub struct FabricMgr {
    /// Mock mode for testing
    #[cfg(test)]
    mock_mode: bool,

    /// Captured writes to APPL_DB in mock mode
    #[cfg(test)]
    captured_writes: Vec<(String, String, String, String)>, // (table, key, field, value)
}

impl FabricMgr {
    /// Creates a new FabricMgr instance
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            captured_writes: Vec::new(),
        }
    }

    /// Enables mock mode for testing
    #[cfg(test)]
    pub fn with_mock_mode(mut self) -> Self {
        self.mock_mode = true;
        self
    }

    /// Gets captured writes (for testing)
    #[cfg(test)]
    pub fn captured_writes(&self) -> &[(String, String, String, String)] {
        &self.captured_writes
    }

    /// Writes a single field-value pair to APPL_DB
    ///
    /// Routes to the appropriate table based on key:
    /// - "FABRIC_MONITOR_DATA" → APP_FABRIC_MONITOR_DATA
    /// - Other keys → APP_FABRIC_PORT_TABLE
    #[instrument(skip(self))]
    pub async fn write_config_to_app_db(
        &mut self,
        key: &str,
        field: &str,
        value: &str,
    ) -> CfgMgrResult<bool> {
        let table_name = if key == FABRIC_MONITOR_DATA_KEY {
            "APP_FABRIC_MONITOR_DATA"
        } else {
            "APP_FABRIC_PORT_TABLE"
        };

        #[cfg(test)]
        if self.mock_mode {
            self.captured_writes.push((
                table_name.to_string(),
                key.to_string(),
                field.to_string(),
                value.to_string(),
            ));
            info!("Mock write: {} → {}:{} = {}", table_name, key, field, value);
            return Ok(true);
        }

        // TODO: Implement with real ProducerStateTable
        debug!(
            "Would write to {}: {}:{} = {}",
            table_name, key, field, value
        );
        Ok(true)
    }

    /// Processes a SET operation from CONFIG_DB
    ///
    /// Writes each field-value pair individually to APPL_DB
    #[instrument(skip(self, values))]
    pub async fn process_set(&mut self, key: &str, values: &FieldValues) -> CfgMgrResult<()> {
        // Known fields that should be written individually
        let known_fields = [
            fields::MON_ERR_THRESH_CRC_CELLS,
            fields::MON_ERR_THRESH_RX_CELLS,
            fields::MON_POLL_THRESH_RECOVERY,
            fields::MON_POLL_THRESH_ISOLATION,
            fields::MON_STATE,
            fields::ALIAS,
            fields::LANES,
            fields::ISOLATE_STATUS,
        ];

        // First, process all known fields
        for (field, value) in values {
            if known_fields.contains(&field.as_str()) {
                self.write_config_to_app_db(key, field, value).await?;
            }
        }

        // Then, process any remaining fields
        for (field, value) in values {
            if !known_fields.contains(&field.as_str()) {
                self.write_config_to_app_db(key, field, value).await?;
            }
        }

        Ok(())
    }

    /// Processes a DEL operation from CONFIG_DB
    ///
    /// For fabricmgr, DELETE operations are not explicitly handled in the C++ code
    /// (no deletion from APPL_DB), so this is a no-op
    #[instrument(skip(self))]
    pub async fn process_del(&mut self, _key: &str) -> CfgMgrResult<()> {
        debug!("DELETE operation - no-op for fabricmgr");
        Ok(())
    }
}

impl Default for FabricMgr {
    fn default() -> Self {
        Self::new()
    }
}

/// Orch trait implementation
#[async_trait]
impl Orch for FabricMgr {
    fn name(&self) -> &str {
        "fabricmgr"
    }

    async fn do_task(&mut self) {
        // Placeholder - actual implementation would:
        // 1. Drain consumers for each subscribed table
        // 2. Process SET/DEL operations
        // 3. Write to APPL_DB via producers
        debug!("do_task called (placeholder)");
    }
}

/// CfgMgr trait implementation
#[async_trait]
impl CfgMgr for FabricMgr {
    fn daemon_name(&self) -> &str {
        "fabricmgrd"
    }

    fn is_warm_restart(&self) -> bool {
        false // fabricmgr does not support warm restart
    }

    fn warm_restart_state(&self) -> sonic_cfgmgr_common::WarmRestartState {
        sonic_cfgmgr_common::WarmRestartState::Disabled
    }

    async fn set_warm_restart_state(&mut self, _state: sonic_cfgmgr_common::WarmRestartState) {
        // No-op: fabricmgr does not support warm restart
    }

    fn config_table_names(&self) -> &[&str] {
        &[
            CFG_FABRIC_MONITOR_DATA_TABLE_NAME,
            CFG_FABRIC_MONITOR_PORT_TABLE_NAME,
        ]
    }

    fn state_table_names(&self) -> &[&str] {
        &[] // No STATE_DB tables
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fabric_mgr_new() {
        let mgr = FabricMgr::new();
        assert!(mgr.captured_writes.is_empty());
    }

    #[tokio::test]
    async fn test_write_monitor_data() {
        let mut mgr = FabricMgr::new().with_mock_mode();

        mgr.write_config_to_app_db(FABRIC_MONITOR_DATA_KEY, fields::MON_STATE, "enable")
            .await
            .unwrap();

        let writes = mgr.captured_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, "APP_FABRIC_MONITOR_DATA");
        assert_eq!(writes[0].1, FABRIC_MONITOR_DATA_KEY);
        assert_eq!(writes[0].2, fields::MON_STATE);
        assert_eq!(writes[0].3, "enable");
    }

    #[tokio::test]
    async fn test_write_fabric_port() {
        let mut mgr = FabricMgr::new().with_mock_mode();

        mgr.write_config_to_app_db("Fabric0", fields::ALIAS, "Fabric0")
            .await
            .unwrap();

        let writes = mgr.captured_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, "APP_FABRIC_PORT_TABLE");
        assert_eq!(writes[0].1, "Fabric0");
        assert_eq!(writes[0].2, fields::ALIAS);
        assert_eq!(writes[0].3, "Fabric0");
    }

    #[tokio::test]
    async fn test_process_set_monitor_data() {
        let mut mgr = FabricMgr::new().with_mock_mode();

        let values = vec![
            (fields::MON_STATE.to_string(), "enable".to_string()),
            (
                fields::MON_ERR_THRESH_CRC_CELLS.to_string(),
                "1000".to_string(),
            ),
            (
                fields::MON_ERR_THRESH_RX_CELLS.to_string(),
                "2000".to_string(),
            ),
        ];

        mgr.process_set(FABRIC_MONITOR_DATA_KEY, &values)
            .await
            .unwrap();

        let writes = mgr.captured_writes();
        assert_eq!(writes.len(), 3);

        // Verify all fields were written
        assert!(writes
            .iter()
            .any(|(_, _, field, value)| field == fields::MON_STATE && value == "enable"));
        assert!(writes.iter().any(|(_, _, field, value)| field
            == fields::MON_ERR_THRESH_CRC_CELLS
            && value == "1000"));
        assert!(writes.iter().any(
            |(_, _, field, value)| field == fields::MON_ERR_THRESH_RX_CELLS && value == "2000"
        ));
    }

    #[tokio::test]
    async fn test_process_set_fabric_port() {
        let mut mgr = FabricMgr::new().with_mock_mode();

        let values = vec![
            (fields::ALIAS.to_string(), "Fabric0".to_string()),
            (fields::LANES.to_string(), "0,1,2,3".to_string()),
            (fields::ISOLATE_STATUS.to_string(), "False".to_string()),
        ];

        mgr.process_set("Fabric0", &values).await.unwrap();

        let writes = mgr.captured_writes();
        assert_eq!(writes.len(), 3);

        // Verify routing to correct table
        assert!(writes
            .iter()
            .all(|(table, _, _, _)| table == "APP_FABRIC_PORT_TABLE"));

        // Verify all fields were written
        assert!(writes
            .iter()
            .any(|(_, _, field, value)| field == fields::ALIAS && value == "Fabric0"));
        assert!(writes
            .iter()
            .any(|(_, _, field, value)| field == fields::LANES && value == "0,1,2,3"));
        assert!(writes
            .iter()
            .any(|(_, _, field, value)| field == fields::ISOLATE_STATUS && value == "False"));
    }

    #[tokio::test]
    async fn test_process_set_unknown_fields() {
        let mut mgr = FabricMgr::new().with_mock_mode();

        let values = vec![
            (fields::ALIAS.to_string(), "Fabric0".to_string()),
            ("custom_field".to_string(), "custom_value".to_string()),
        ];

        mgr.process_set("Fabric0", &values).await.unwrap();

        let writes = mgr.captured_writes();
        assert_eq!(writes.len(), 2);

        // Verify both known and unknown fields are written
        assert!(writes.iter().any(|(_, _, field, _)| field == fields::ALIAS));
        assert!(writes
            .iter()
            .any(|(_, _, field, value)| field == "custom_field" && value == "custom_value"));
    }

    #[tokio::test]
    async fn test_process_del() {
        let mut mgr = FabricMgr::new().with_mock_mode();

        // DEL is a no-op in fabricmgr
        mgr.process_del("Fabric0").await.unwrap();

        let writes = mgr.captured_writes();
        assert_eq!(writes.len(), 0); // No writes for DELETE
    }

    #[test]
    fn test_cfgmgr_trait() {
        let mgr = FabricMgr::new();
        assert_eq!(mgr.daemon_name(), "fabricmgrd");
        assert!(!mgr.is_warm_restart());

        let tables = mgr.config_table_names();
        assert_eq!(tables.len(), 2);
        assert!(tables.contains(&"FABRIC_MONITOR"));
        assert!(tables.contains(&"FABRIC_PORT"));

        let state_tables = mgr.state_table_names();
        assert_eq!(state_tables.len(), 0);
    }

    #[test]
    fn test_orch_trait() {
        let mgr = FabricMgr::new();
        assert_eq!(mgr.name(), "fabricmgr");
    }
}
