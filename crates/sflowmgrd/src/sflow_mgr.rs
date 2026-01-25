//! SflowMgr - Core sFlow configuration manager implementation

use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{debug, error, info, instrument, warn};

use sonic_cfgmgr_common::{shell, CfgMgr, CfgMgrError, CfgMgrResult, FieldValues, Orch};

use crate::constants::*;
use crate::fields;
use crate::types::SflowPortInfo;
use crate::{
    CFG_PORT_TABLE_NAME, CFG_SFLOW_SESSION_TABLE_NAME, CFG_SFLOW_TABLE_NAME, STATE_PORT_TABLE_NAME,
};

/// SflowMgr manages sFlow sampling configuration
///
/// Configuration flow:
/// 1. Global config: SFLOW table → APP_SFLOW_TABLE + service control
/// 2. Session config: SFLOW_SESSION table → APP_SFLOW_SESSION_TABLE
/// 3. Port speed: PORT table → updates sampling rates
/// 4. Oper speed: PORT_TABLE (STATE_DB) → updates sampling rates
pub struct SflowMgr {
    /// Per-port configuration map
    port_config_map: HashMap<String, SflowPortInfo>,

    /// Global sFlow enable/disable
    global_enable: bool,

    /// Global sampling direction ("rx", "tx", "both")
    global_direction: String,

    /// Whether "all interfaces" configuration is enabled
    intf_all_conf: bool,

    /// Direction for "all interfaces" configuration
    intf_all_dir: String,

    /// Mock mode for testing (capture commands instead of executing)
    #[cfg(test)]
    mock_mode: bool,

    /// Captured service commands in mock mode
    #[cfg(test)]
    captured_service_commands: Vec<String>,
}

impl SflowMgr {
    /// Creates a new SflowMgr instance
    pub fn new() -> Self {
        Self {
            port_config_map: HashMap::new(),
            global_enable: false,
            global_direction: DEFAULT_DIRECTION.to_string(),
            intf_all_conf: true,
            intf_all_dir: DEFAULT_DIRECTION.to_string(),
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            captured_service_commands: Vec::new(),
        }
    }

    /// Enables mock mode for testing
    #[cfg(test)]
    pub fn with_mock_mode(mut self) -> Self {
        self.mock_mode = true;
        self
    }

    /// Gets captured service commands (for testing)
    #[cfg(test)]
    pub fn captured_service_commands(&self) -> &[String] {
        &self.captured_service_commands
    }

    /// Checks if a port is enabled for sFlow sampling
    ///
    /// A port is enabled if:
    /// - Global sFlow is enabled, AND
    /// - Either "all interfaces" is configured, OR
    ///   the port has local admin config set to "up"
    #[instrument(skip(self))]
    pub fn is_port_enabled(&self, alias: &str) -> bool {
        let port_info = match self.port_config_map.get(alias) {
            Some(info) => info,
            None => return false,
        };

        let local_admin = port_info.local_admin_cfg;
        let status = port_info.admin == "up";

        self.global_enable && (self.intf_all_conf || (local_admin && status))
    }

    /// Finds the appropriate sampling rate for a port
    ///
    /// Priority:
    /// 1. Operational speed (if available)
    /// 2. Configured speed
    /// 3. ERROR_SPEED if port not found
    #[instrument(skip(self))]
    pub fn find_sampling_rate(&self, alias: &str) -> String {
        let port_info = match self.port_config_map.get(alias) {
            Some(info) => info,
            None => {
                error!("Port {} not found in configuration map", alias);
                return ERROR_SPEED.to_string();
            }
        };

        let oper_speed = &port_info.oper_speed;
        let cfg_speed = &port_info.speed;

        if !oper_speed.is_empty() && oper_speed != NA_SPEED {
            oper_speed.clone()
        } else {
            cfg_speed.clone()
        }
    }

    /// Handles hsflowd service lifecycle
    ///
    /// Commands:
    /// - `enable=true`: systemctl restart hsflowd
    /// - `enable=false`: systemctl stop hsflowd
    #[instrument(skip(self))]
    pub async fn handle_service(&mut self, enable: bool) -> CfgMgrResult<()> {
        let cmd = if enable {
            "systemctl restart hsflowd"
        } else {
            "systemctl stop hsflowd"
        };

        #[cfg(test)]
        if self.mock_mode {
            self.captured_service_commands.push(cmd.to_string());
            info!("Mock mode: captured service command: {}", cmd);
            return Ok(());
        }

        match shell::exec(cmd).await {
            Ok(result) if result.success() => {
                info!("Service command succeeded: {}", cmd);
                Ok(())
            }
            Ok(result) => {
                warn!(
                    "Service command failed: {} (exit code: {})",
                    cmd, result.exit_code
                );
                Err(CfgMgrError::ShellCommandFailed {
                    command: cmd.to_string(),
                    exit_code: result.exit_code,
                    output: result.stderr,
                })
            }
            Err(e) => {
                error!("Failed to execute service command: {}", e);
                Err(e)
            }
        }
    }

    /// Builds field-value tuples for global sFlow session configuration
    fn build_global_session_fvs(&self, alias: &str, direction: &str) -> FieldValues {
        vec![
            (
                fields::ADMIN_STATE.to_string(),
                DEFAULT_ADMIN_STATE.to_string(),
            ),
            (
                fields::SAMPLE_RATE.to_string(),
                self.find_sampling_rate(alias),
            ),
            (fields::SAMPLE_DIRECTION.to_string(), direction.to_string()),
        ]
    }

    /// Builds field-value tuples for port-specific sFlow session configuration
    fn build_port_session_fvs(&self, port_info: &SflowPortInfo) -> FieldValues {
        let mut fvs = Vec::new();

        if port_info.local_admin_cfg {
            fvs.push((fields::ADMIN_STATE.to_string(), port_info.admin.clone()));
        }

        fvs.push((fields::SAMPLE_RATE.to_string(), port_info.rate.clone()));

        if port_info.local_dir_cfg {
            fvs.push((fields::SAMPLE_DIRECTION.to_string(), port_info.dir.clone()));
        }

        fvs
    }

    /// Handles session configuration for all ports
    ///
    /// Called when global "all interfaces" configuration changes
    #[instrument(skip(self))]
    pub async fn handle_session_all(&mut self, enable: bool, direction: &str) -> CfgMgrResult<()> {
        for (alias, port_info) in &self.port_config_map {
            if enable {
                let fvs = if port_info.has_local_config() {
                    let mut fvs = self.build_port_session_fvs(port_info);

                    // Use global admin state if not locally configured
                    if !port_info.local_admin_cfg {
                        fvs.push((
                            fields::ADMIN_STATE.to_string(),
                            DEFAULT_ADMIN_STATE.to_string(),
                        ));
                    }

                    // Use global direction if not locally configured
                    if !port_info.local_dir_cfg {
                        fvs.push((fields::SAMPLE_DIRECTION.to_string(), direction.to_string()));
                    }

                    fvs
                } else {
                    self.build_global_session_fvs(alias, direction)
                };

                self.write_to_app_db_session(alias, fvs).await?;
            } else if !port_info.local_admin_cfg {
                self.delete_from_app_db_session(alias).await?;
            }
        }

        Ok(())
    }

    /// Handles session configuration for ports with local configuration
    #[instrument(skip(self))]
    pub async fn handle_session_local(&mut self, enable: bool) -> CfgMgrResult<()> {
        for (alias, port_info) in &self.port_config_map {
            if port_info.has_local_config() {
                let fvs = self.build_port_session_fvs(port_info);

                if enable {
                    self.write_to_app_db_session(alias, fvs).await?;
                } else {
                    self.delete_from_app_db_session(alias).await?;
                }
            }
        }

        Ok(())
    }

    /// Processes and fills missing configuration values for a port session
    ///
    /// This handles the logic where:
    /// - Local config values are used when present
    /// - Global/default values are filled in when local config is absent
    #[instrument(skip(self, values))]
    pub fn check_and_fill_values(
        &mut self,
        alias: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<FieldValues> {
        // First pass: collect values and determine what's present
        let mut rate_present = false;
        let mut admin_present = false;
        let mut dir_present = false;
        let mut fvs = Vec::new();

        // Extract alias clone for find_sampling_rate call
        let alias_owned = alias.to_string();

        // Process provided values
        for (field, value) in values {
            match field.as_str() {
                fields::SAMPLE_RATE => {
                    rate_present = true;
                    fvs.push((field.clone(), value.clone()));
                }
                fields::ADMIN_STATE => {
                    admin_present = true;
                    fvs.push((field.clone(), value.clone()));
                }
                fields::SAMPLE_DIRECTION => {
                    dir_present = true;
                    fvs.push((field.clone(), value.clone()));
                }
                "NULL" => continue,
                _ => {}
            }
        }

        // Get or create port_info and update it
        let port_info = self.port_config_map.entry(alias_owned.clone()).or_default();

        // Update port_info based on what was present
        for (field, value) in values {
            match field.as_str() {
                fields::SAMPLE_RATE => {
                    port_info.rate = value.clone();
                    port_info.local_rate_cfg = true;
                }
                fields::ADMIN_STATE => {
                    port_info.admin = value.clone();
                    port_info.local_admin_cfg = true;
                }
                fields::SAMPLE_DIRECTION => {
                    port_info.dir = value.clone();
                    port_info.local_dir_cfg = true;
                }
                _ => {}
            }
        }

        // Fill missing values with defaults
        if !rate_present {
            let default_rate = if port_info.rate.is_empty() || port_info.local_rate_cfg {
                self.find_sampling_rate(&alias_owned)
            } else {
                port_info.rate.clone()
            };

            let port_info_mut = self.port_config_map.get_mut(&alias_owned).unwrap();
            port_info_mut.rate = default_rate.clone();
            port_info_mut.local_rate_cfg = false;
            fvs.push((fields::SAMPLE_RATE.to_string(), default_rate));
        }

        if !admin_present {
            let port_info_mut = self.port_config_map.get_mut(&alias_owned).unwrap();
            if port_info_mut.admin.is_empty() {
                port_info_mut.admin = DEFAULT_ADMIN_STATE.to_string();
            }
            let admin_value = port_info_mut.admin.clone();
            port_info_mut.local_admin_cfg = false;
            fvs.push((fields::ADMIN_STATE.to_string(), admin_value));
        }

        if !dir_present {
            let port_info_mut = self.port_config_map.get_mut(&alias_owned).unwrap();
            if port_info_mut.dir.is_empty() {
                port_info_mut.dir = self.global_direction.clone();
            }
            let dir_value = port_info_mut.dir.clone();
            port_info_mut.local_dir_cfg = false;
            fvs.push((fields::SAMPLE_DIRECTION.to_string(), dir_value));
        }

        Ok(fvs)
    }

    /// Stub: Writes configuration to APPL_DB SFLOW_TABLE
    ///
    /// In production, this would use ProducerStateTable
    #[instrument(skip(self, _fvs))]
    async fn write_to_app_db_sflow(&self, _key: &str, _fvs: FieldValues) -> CfgMgrResult<()> {
        // TODO: Implement with real ProducerStateTable
        debug!("Would write to APP_SFLOW_TABLE");
        Ok(())
    }

    /// Stub: Writes configuration to APPL_DB SFLOW_SESSION_TABLE
    #[instrument(skip(self, _fvs))]
    async fn write_to_app_db_session(&self, _key: &str, _fvs: FieldValues) -> CfgMgrResult<()> {
        // TODO: Implement with real ProducerStateTable
        debug!("Would write to APP_SFLOW_SESSION_TABLE");
        Ok(())
    }

    /// Stub: Deletes entry from APPL_DB SFLOW_TABLE
    #[instrument(skip(self))]
    async fn delete_from_app_db_sflow(&self, _key: &str) -> CfgMgrResult<()> {
        // TODO: Implement with real ProducerStateTable
        debug!("Would delete from APP_SFLOW_TABLE");
        Ok(())
    }

    /// Stub: Deletes entry from APPL_DB SFLOW_SESSION_TABLE
    #[instrument(skip(self))]
    async fn delete_from_app_db_session(&self, _key: &str) -> CfgMgrResult<()> {
        // TODO: Implement with real ProducerStateTable
        debug!("Would delete from APP_SFLOW_SESSION_TABLE");
        Ok(())
    }

    /// Processes PORT table updates (port speed changes)
    #[instrument(skip(self, _key, _op, values))]
    pub async fn process_port_update(
        &mut self,
        _key: &str,
        _op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<()> {
        // Extract speed from values
        let new_speed = values
            .iter()
            .find(|(field, _)| field == fields::SPEED)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| ERROR_SPEED.to_string());

        debug!("Port speed update: {}", new_speed);
        // TODO: Implement full port update logic from C++
        Ok(())
    }

    /// Processes STATE_DB PORT_TABLE updates (operational speed)
    #[instrument(skip(self, _key, _op, values))]
    pub async fn process_oper_speed(
        &mut self,
        _key: &str,
        _op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<()> {
        // Extract oper_speed from values
        let oper_speed = values
            .iter()
            .find(|(field, _)| field == fields::SPEED)
            .map(|(_, value)| value.clone())
            .unwrap_or_default();

        debug!("Operational speed update: {}", oper_speed);
        // TODO: Implement full oper speed processing from C++
        Ok(())
    }
}

impl Default for SflowMgr {
    fn default() -> Self {
        Self::new()
    }
}

/// Orch trait implementation
#[async_trait]
impl Orch for SflowMgr {
    fn name(&self) -> &str {
        "sflowmgr"
    }

    async fn do_task(&mut self) {
        // Placeholder - actual implementation would:
        // 1. Drain consumers for each subscribed table
        // 2. Process entries based on table type
        // 3. Update APPL_DB via producers
        debug!("do_task called (placeholder)");
    }
}

/// CfgMgr trait implementation
#[async_trait]
impl CfgMgr for SflowMgr {
    fn daemon_name(&self) -> &str {
        "sflowmgrd"
    }

    fn is_warm_restart(&self) -> bool {
        false // sflowmgr does not support warm restart
    }

    fn warm_restart_state(&self) -> sonic_cfgmgr_common::WarmRestartState {
        sonic_cfgmgr_common::WarmRestartState::Disabled
    }

    async fn set_warm_restart_state(&mut self, _state: sonic_cfgmgr_common::WarmRestartState) {
        // No-op: sflowmgr does not support warm restart
    }

    fn config_table_names(&self) -> &[&str] {
        &[
            CFG_SFLOW_TABLE_NAME,
            CFG_SFLOW_SESSION_TABLE_NAME,
            CFG_PORT_TABLE_NAME,
        ]
    }

    fn state_table_names(&self) -> &[&str] {
        &[STATE_PORT_TABLE_NAME]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sflow_mgr_new() {
        let mgr = SflowMgr::new();
        assert!(!mgr.global_enable);
        assert_eq!(mgr.global_direction, "rx");
        assert!(mgr.intf_all_conf);
        assert_eq!(mgr.intf_all_dir, "rx");
        assert!(mgr.port_config_map.is_empty());
    }

    #[test]
    fn test_is_port_enabled_global_disabled() {
        let mut mgr = SflowMgr::new();
        mgr.global_enable = false;

        let mut port_info = SflowPortInfo::new();
        port_info.local_admin_cfg = true;
        port_info.admin = "up".to_string();
        mgr.port_config_map
            .insert("Ethernet0".to_string(), port_info);

        assert!(!mgr.is_port_enabled("Ethernet0"));
    }

    #[test]
    fn test_is_port_enabled_with_all_interfaces() {
        let mut mgr = SflowMgr::new();
        mgr.global_enable = true;
        mgr.intf_all_conf = true;

        let port_info = SflowPortInfo::new();
        mgr.port_config_map
            .insert("Ethernet0".to_string(), port_info);

        assert!(mgr.is_port_enabled("Ethernet0"));
    }

    #[test]
    fn test_is_port_enabled_with_local_config() {
        let mut mgr = SflowMgr::new();
        mgr.global_enable = true;
        mgr.intf_all_conf = false;

        let mut port_info = SflowPortInfo::new();
        port_info.local_admin_cfg = true;
        port_info.admin = "up".to_string();
        mgr.port_config_map
            .insert("Ethernet0".to_string(), port_info);

        assert!(mgr.is_port_enabled("Ethernet0"));
    }

    #[test]
    fn test_find_sampling_rate_uses_oper_speed() {
        let mut mgr = SflowMgr::new();

        let mut port_info = SflowPortInfo::new();
        port_info.speed = "100000".to_string();
        port_info.oper_speed = "40000".to_string();
        mgr.port_config_map
            .insert("Ethernet0".to_string(), port_info);

        assert_eq!(mgr.find_sampling_rate("Ethernet0"), "40000");
    }

    #[test]
    fn test_find_sampling_rate_fallback_to_cfg_speed() {
        let mut mgr = SflowMgr::new();

        let mut port_info = SflowPortInfo::new();
        port_info.speed = "100000".to_string();
        port_info.oper_speed = "N/A".to_string();
        mgr.port_config_map
            .insert("Ethernet0".to_string(), port_info);

        assert_eq!(mgr.find_sampling_rate("Ethernet0"), "100000");
    }

    #[test]
    fn test_find_sampling_rate_port_not_found() {
        let mgr = SflowMgr::new();
        assert_eq!(mgr.find_sampling_rate("NonExistent"), "error");
    }

    #[tokio::test]
    async fn test_handle_service_enable() {
        let mut mgr = SflowMgr::new().with_mock_mode();
        mgr.handle_service(true).await.unwrap();

        let commands = mgr.captured_service_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "systemctl restart hsflowd");
    }

    #[tokio::test]
    async fn test_handle_service_disable() {
        let mut mgr = SflowMgr::new().with_mock_mode();
        mgr.handle_service(false).await.unwrap();

        let commands = mgr.captured_service_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "systemctl stop hsflowd");
    }

    #[test]
    fn test_build_global_session_fvs() {
        let mut mgr = SflowMgr::new();

        let mut port_info = SflowPortInfo::new();
        port_info.speed = "100000".to_string();
        mgr.port_config_map
            .insert("Ethernet0".to_string(), port_info);

        let fvs = mgr.build_global_session_fvs("Ethernet0", "rx");

        assert_eq!(fvs.len(), 3);
        assert!(fvs.contains(&("admin_state".to_string(), "up".to_string())));
        assert!(fvs.contains(&("sample_rate".to_string(), "100000".to_string())));
        assert!(fvs.contains(&("sample_direction".to_string(), "rx".to_string())));
    }

    #[test]
    fn test_build_port_session_fvs() {
        let mgr = SflowMgr::new();

        let mut port_info = SflowPortInfo::new();
        port_info.local_admin_cfg = true;
        port_info.admin = "down".to_string();
        port_info.local_rate_cfg = true;
        port_info.rate = "5000".to_string();
        port_info.local_dir_cfg = true;
        port_info.dir = "both".to_string();

        let fvs = mgr.build_port_session_fvs(&port_info);

        assert_eq!(fvs.len(), 3);
        assert!(fvs.contains(&("admin_state".to_string(), "down".to_string())));
        assert!(fvs.contains(&("sample_rate".to_string(), "5000".to_string())));
        assert!(fvs.contains(&("sample_direction".to_string(), "both".to_string())));
    }

    #[test]
    fn test_cfgmgr_trait() {
        let mgr = SflowMgr::new();
        assert_eq!(mgr.daemon_name(), "sflowmgrd");
        assert!(!mgr.is_warm_restart());

        let tables = mgr.config_table_names();
        assert_eq!(tables.len(), 3);
        assert!(tables.contains(&"SFLOW"));
        assert!(tables.contains(&"SFLOW_SESSION"));
        assert!(tables.contains(&"PORT"));

        let state_tables = mgr.state_table_names();
        assert_eq!(state_tables.len(), 1);
        assert!(state_tables.contains(&"PORT_TABLE"));
    }
}
