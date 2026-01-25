//! PortMgr implementation - the core port configuration manager.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tracing::{debug, info, instrument, warn};

use sonic_cfgmgr_common::{
    defaults, shell, CfgMgr, CfgMgrError, CfgMgrResult, FieldValues, Orch, WarmRestartState,
};

use crate::tables::{self, fields};

/// Operation type for table entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// SET operation (add or update).
    Set,
    /// DEL operation (remove).
    Del,
}

/// A pending task from the consumer queue.
#[derive(Debug, Clone)]
pub struct PendingTask {
    /// The key (e.g., port alias).
    pub key: String,
    /// The operation type.
    pub op: Operation,
    /// The field-value pairs.
    pub fvs: FieldValues,
}

/// Port configuration manager.
///
/// Manages port MTU and admin status configuration by:
/// 1. Reading configuration from CONFIG_DB
/// 2. Executing `ip link` commands to configure the network stack
/// 3. Writing configuration to APPL_DB for orchagent
pub struct PortMgr {
    /// Daemon name for logging and warm restart.
    daemon_name: String,

    /// Warm restart enabled flag.
    warm_restart: bool,

    /// Current warm restart state.
    warm_restart_state: WarmRestartState,

    /// Set of known ports (have been configured at least once).
    port_list: HashSet<String>,

    /// Pending tasks to retry (port not ready yet).
    pending_tasks: HashMap<String, PendingTask>,

    /// Mock mode for testing (don't execute shell commands).
    #[cfg(test)]
    mock_mode: bool,

    /// Captured shell commands in mock mode.
    #[cfg(test)]
    captured_commands: Vec<String>,

    /// Mock port states for testing.
    #[cfg(test)]
    mock_port_states: HashMap<String, bool>,

    /// Mock app DB writes for testing.
    #[cfg(test)]
    app_db_writes: Vec<(String, FieldValues)>,
}

impl PortMgr {
    /// Creates a new PortMgr instance.
    ///
    /// In a real implementation, this would take database connections.
    /// For now, we create a standalone instance for testing.
    pub fn new() -> Self {
        Self {
            daemon_name: "portmgrd".to_string(),
            warm_restart: false,
            warm_restart_state: WarmRestartState::Disabled,
            port_list: HashSet::new(),
            pending_tasks: HashMap::new(),
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            captured_commands: Vec::new(),
            #[cfg(test)]
            mock_port_states: HashMap::new(),
            #[cfg(test)]
            app_db_writes: Vec::new(),
        }
    }

    /// Creates a new PortMgr with warm restart enabled.
    pub fn with_warm_restart(mut self, enabled: bool) -> Self {
        self.warm_restart = enabled;
        if enabled {
            self.warm_restart_state = WarmRestartState::Initialized;
        }
        self
    }

    /// Sets the port MTU using `ip link set`.
    ///
    /// # Arguments
    ///
    /// * `alias` - The port name (e.g., "Ethernet0")
    /// * `mtu` - The MTU value as a string (e.g., "9100")
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - MTU was set successfully
    /// * `Ok(false)` - Port not ready, should retry
    /// * `Err(_)` - Command failed fatally
    #[instrument(skip(self), fields(port = %alias, mtu = %mtu))]
    pub async fn set_port_mtu(&mut self, alias: &str, mtu: &str) -> CfgMgrResult<bool> {
        let cmd = format!(
            "{} link set dev {} mtu {}",
            shell::IP_CMD,
            shell::shellquote(alias),
            shell::shellquote(mtu)
        );

        #[cfg(test)]
        if self.mock_mode {
            self.captured_commands.push(cmd.clone());
            return Ok(true);
        }

        let result = shell::exec(&cmd).await?;

        if result.success() {
            // Also write to app DB
            self.write_config_to_app_db(alias, fields::MTU, mtu).await?;
            info!("Set MTU for {} to {}", alias, mtu);
            Ok(true)
        } else if !self.is_port_state_ok(alias).await? {
            // Port not ready yet - this is expected during startup
            warn!(
                "Setting MTU for {} failed - port not ready: {}",
                alias, result.stderr
            );
            Ok(false)
        } else {
            // Port is ready but command still failed - this could happen
            // for port channel members during startup
            warn!(
                "Setting MTU for {} failed (port is ready): {}",
                alias, result.stderr
            );
            Ok(false)
        }
    }

    /// Sets the port admin status using `ip link set`.
    ///
    /// # Arguments
    ///
    /// * `alias` - The port name (e.g., "Ethernet0")
    /// * `up` - True for up, false for down
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Admin status was set successfully
    /// * `Ok(false)` - Port not ready, should retry
    /// * `Err(_)` - Command failed fatally
    #[instrument(skip(self), fields(port = %alias, up = %up))]
    pub async fn set_port_admin_status(&mut self, alias: &str, up: bool) -> CfgMgrResult<bool> {
        let status = if up { "up" } else { "down" };
        let cmd = format!(
            "{} link set dev {} {}",
            shell::IP_CMD,
            shell::shellquote(alias),
            status
        );

        #[cfg(test)]
        if self.mock_mode {
            self.captured_commands.push(cmd.clone());
            return Ok(true);
        }

        let result = shell::exec(&cmd).await?;

        if result.success() {
            self.write_config_to_app_db(alias, fields::ADMIN_STATUS, status)
                .await?;
            info!("Set admin status for {} to {}", alias, status);
            Ok(true)
        } else if !self.is_port_state_ok(alias).await? {
            warn!(
                "Setting admin status for {} failed - port not ready: {}",
                alias, result.stderr
            );
            Ok(false)
        } else {
            // Port is ready but command failed - this is a real error
            Err(CfgMgrError::ShellCommandFailed {
                command: cmd,
                exit_code: result.exit_code,
                output: result.combined_output(),
            })
        }
    }

    /// Checks if a port is ready (exists in STATE_DB with state).
    ///
    /// # Arguments
    ///
    /// * `alias` - The port name to check
    ///
    /// # Returns
    ///
    /// True if the port exists and has a state, false otherwise.
    #[instrument(skip(self), fields(port = %alias))]
    pub async fn is_port_state_ok(&self, alias: &str) -> CfgMgrResult<bool> {
        #[cfg(test)]
        if self.mock_mode {
            return Ok(self.mock_port_states.get(alias).copied().unwrap_or(false));
        }

        // In real implementation, this would query STATE_DB
        // For now, return false to indicate we need the real DB connection
        debug!("Checking port state for {} (stub)", alias);
        Ok(false)
    }

    /// Writes configuration to APPL_DB.
    async fn write_config_to_app_db(
        &mut self,
        alias: &str,
        field: &str,
        value: &str,
    ) -> CfgMgrResult<()> {
        let fvs = vec![(field.to_string(), value.to_string())];
        self.write_config_to_app_db_multi(alias, fvs).await
    }

    /// Writes multiple fields to APPL_DB.
    async fn write_config_to_app_db_multi(
        &mut self,
        alias: &str,
        fvs: FieldValues,
    ) -> CfgMgrResult<()> {
        #[cfg(test)]
        {
            self.app_db_writes.push((alias.to_string(), fvs));
            Ok(())
        }

        #[cfg(not(test))]
        {
            // In real implementation, this would write to APPL_DB
            debug!("Writing to APPL_DB: {}:{:?}", alias, fvs);
            Ok(())
        }
    }

    /// Processes a SET operation for a port.
    #[instrument(skip(self, fvs), fields(port = %alias))]
    pub async fn process_port_set(&mut self, alias: &str, fvs: FieldValues) -> CfgMgrResult<()> {
        let port_ok = self.is_port_state_ok(alias).await?;
        let configured = self.port_list.contains(alias);

        // Determine MTU and admin status
        let mut mtu = if !configured {
            Some(defaults::DEFAULT_MTU.to_string())
        } else {
            None
        };
        let mut admin_status = if !configured {
            Some(defaults::DEFAULT_ADMIN_STATUS.to_string())
        } else {
            None
        };

        // Collect other field-values to pass through
        let mut other_fvs: FieldValues = Vec::new();

        for (field, value) in &fvs {
            match field.as_str() {
                fields::MTU => mtu = Some(value.clone()),
                fields::ADMIN_STATUS => admin_status = Some(value.clone()),
                _ => other_fvs.push((field.clone(), value.clone())),
            }
        }

        // First time seeing this port - add to list
        if !configured {
            self.port_list.insert(alias.to_string());
        } else if !port_ok {
            // Port already configured but not ready - skip for now
            debug!("Port {} configured but not ready, skipping", alias);
            return Ok(());
        }

        // If port is not ready, write config to APPL_DB anyway
        // (orchagent will create the port) but defer ip commands
        if !port_ok {
            let mut all_fvs = other_fvs.clone();
            if let Some(ref m) = mtu {
                all_fvs.push((fields::MTU.to_string(), m.clone()));
            }
            if let Some(ref s) = admin_status {
                all_fvs.push((fields::ADMIN_STATUS.to_string(), s.clone()));
            }

            if !all_fvs.is_empty() {
                self.write_config_to_app_db_multi(alias, all_fvs).await?;
            }

            info!("Port {} not ready, pending ip commands", alias);

            // Save pending task for retry
            let pending = PendingTask {
                key: alias.to_string(),
                op: Operation::Set,
                fvs: vec![
                    (fields::MTU.to_string(), mtu.clone().unwrap_or_default()),
                    (
                        fields::ADMIN_STATUS.to_string(),
                        admin_status.clone().unwrap_or_default(),
                    ),
                ],
            };
            self.pending_tasks.insert(alias.to_string(), pending);

            return Ok(());
        }

        // Write other fields to APPL_DB
        if !other_fvs.is_empty() {
            self.write_config_to_app_db_multi(alias, other_fvs).await?;
        }

        // Execute ip commands for MTU and admin status
        if let Some(m) = mtu {
            if !m.is_empty() {
                self.set_port_mtu(alias, &m).await?;
                info!("Configured {} MTU to {}", alias, m);
            }
        }

        if let Some(s) = admin_status {
            if !s.is_empty() {
                let up = s == "up";
                self.set_port_admin_status(alias, up).await?;
                info!("Configured {} admin status to {}", alias, s);
            }
        }

        // Remove from pending if it was there
        self.pending_tasks.remove(alias);

        Ok(())
    }

    /// Processes a DEL operation for a port.
    #[instrument(skip(self), fields(port = %alias))]
    pub async fn process_port_del(&mut self, alias: &str) -> CfgMgrResult<()> {
        info!("Deleting port {}", alias);

        // In real implementation, would delete from APPL_DB
        #[cfg(test)]
        {
            // For testing, we just track the deletion
            self.app_db_writes
                .push((format!("DEL:{}", alias), Vec::new()));
        }

        self.port_list.remove(alias);
        self.pending_tasks.remove(alias);

        Ok(())
    }

    /// Processes a SendToIngress port SET operation.
    #[instrument(skip(self, fvs), fields(port = %alias))]
    pub async fn process_send_to_ingress_set(
        &mut self,
        alias: &str,
        fvs: FieldValues,
    ) -> CfgMgrResult<()> {
        info!("Adding SendToIngress port: {}", alias);

        // Simply pass through to APPL_DB
        self.write_config_to_app_db_multi(alias, fvs).await?;

        Ok(())
    }

    /// Processes a SendToIngress port DEL operation.
    #[instrument(skip(self), fields(port = %alias))]
    pub async fn process_send_to_ingress_del(&mut self, alias: &str) -> CfgMgrResult<()> {
        info!("Removing SendToIngress port: {}", alias);

        // In real implementation, would delete from APPL_DB
        #[cfg(test)]
        {
            self.app_db_writes
                .push((format!("DEL:SendToIngress:{}", alias), Vec::new()));
        }

        Ok(())
    }

    /// Returns the number of pending tasks.
    pub fn pending_count(&self) -> usize {
        self.pending_tasks.len()
    }

    /// Returns the set of known ports.
    pub fn ports(&self) -> &HashSet<String> {
        &self.port_list
    }
}

impl Default for PortMgr {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Orch for PortMgr {
    fn name(&self) -> &str {
        "PortMgr"
    }

    async fn do_task(&mut self) {
        // In real implementation, this would drain the consumer queue
        // and call process_port_set/del for each entry
        debug!("PortMgr::do_task called");
    }

    fn has_pending_tasks(&self) -> bool {
        !self.pending_tasks.is_empty()
    }

    fn dump_pending_tasks(&self) -> Vec<String> {
        self.pending_tasks
            .keys()
            .map(|k| format!("PORT:{}", k))
            .collect()
    }
}

#[async_trait]
impl CfgMgr for PortMgr {
    fn daemon_name(&self) -> &str {
        &self.daemon_name
    }

    fn is_warm_restart(&self) -> bool {
        self.warm_restart
    }

    fn warm_restart_state(&self) -> WarmRestartState {
        self.warm_restart_state
    }

    async fn set_warm_restart_state(&mut self, state: WarmRestartState) {
        info!(
            "Setting warm restart state for {} to {:?}",
            self.daemon_name, state
        );
        self.warm_restart_state = state;
    }

    fn config_table_names(&self) -> &[&str] {
        &[
            tables::CFG_PORT_TABLE_NAME,
            tables::CFG_SEND_TO_INGRESS_PORT_TABLE_NAME,
        ]
    }

    fn state_table_names(&self) -> &[&str] {
        &[tables::STATE_PORT_TABLE_NAME]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mgr() -> PortMgr {
        let mut mgr = PortMgr::new();
        mgr.mock_mode = true;
        mgr
    }

    #[tokio::test]
    async fn test_set_port_mtu() {
        let mut mgr = test_mgr();

        let result = mgr.set_port_mtu("Ethernet0", "9100").await.unwrap();
        assert!(result);

        assert_eq!(mgr.captured_commands.len(), 1);
        assert!(mgr.captured_commands[0].contains("link set dev"));
        assert!(mgr.captured_commands[0].contains("mtu"));
        assert!(mgr.captured_commands[0].contains("9100"));
    }

    #[tokio::test]
    async fn test_set_port_admin_status_up() {
        let mut mgr = test_mgr();

        let result = mgr.set_port_admin_status("Ethernet0", true).await.unwrap();
        assert!(result);

        assert!(mgr.captured_commands[0].contains("link set dev"));
        assert!(mgr.captured_commands[0].contains(" up"));
    }

    #[tokio::test]
    async fn test_set_port_admin_status_down() {
        let mut mgr = test_mgr();

        let result = mgr.set_port_admin_status("Ethernet0", false).await.unwrap();
        assert!(result);

        assert!(mgr.captured_commands[0].contains(" down"));
    }

    #[tokio::test]
    async fn test_process_port_set_first_time() {
        let mut mgr = test_mgr();
        mgr.mock_port_states.insert("Ethernet0".to_string(), true);

        let fvs = vec![("speed".to_string(), "100000".to_string())];

        mgr.process_port_set("Ethernet0", fvs).await.unwrap();

        // Should have set default MTU and admin status
        assert!(mgr.port_list.contains("Ethernet0"));
        assert_eq!(mgr.captured_commands.len(), 2); // MTU + admin status
    }

    #[tokio::test]
    async fn test_process_port_set_with_custom_mtu() {
        let mut mgr = test_mgr();
        mgr.mock_port_states.insert("Ethernet0".to_string(), true);

        let fvs = vec![("mtu".to_string(), "1500".to_string())];

        mgr.process_port_set("Ethernet0", fvs).await.unwrap();

        // Should have used custom MTU
        assert!(mgr.captured_commands[0].contains("1500"));
    }

    #[tokio::test]
    async fn test_process_port_set_port_not_ready() {
        let mut mgr = test_mgr();
        // Port is NOT ready (not in mock_port_states)

        let fvs = vec![("mtu".to_string(), "9100".to_string())];

        mgr.process_port_set("Ethernet0", fvs).await.unwrap();

        // Should NOT have executed any commands
        assert!(mgr.captured_commands.is_empty());

        // Should have pending task
        assert_eq!(mgr.pending_count(), 1);
        assert!(mgr.pending_tasks.contains_key("Ethernet0"));

        // Should still write to APPL_DB
        assert!(!mgr.app_db_writes.is_empty());
    }

    #[tokio::test]
    async fn test_process_port_del() {
        let mut mgr = test_mgr();
        mgr.port_list.insert("Ethernet0".to_string());

        mgr.process_port_del("Ethernet0").await.unwrap();

        assert!(!mgr.port_list.contains("Ethernet0"));
    }

    #[tokio::test]
    async fn test_send_to_ingress() {
        let mut mgr = test_mgr();

        let fvs = vec![("src_port".to_string(), "Ethernet0".to_string())];
        mgr.process_send_to_ingress_set("IngressPort1", fvs)
            .await
            .unwrap();

        assert!(!mgr.app_db_writes.is_empty());
    }

    #[test]
    fn test_orch_trait() {
        let mgr = test_mgr();

        assert_eq!(mgr.name(), "PortMgr");
        assert!(!mgr.has_pending_tasks());
    }

    #[test]
    fn test_cfgmgr_trait() {
        let mgr = test_mgr();

        assert_eq!(mgr.daemon_name(), "portmgrd");
        assert!(!mgr.is_warm_restart());
        assert_eq!(mgr.config_table_names(), &["PORT", "SEND_TO_INGRESS_PORT"]);
    }

    #[test]
    fn test_warm_restart() {
        let mgr = PortMgr::new().with_warm_restart(true);

        assert!(mgr.is_warm_restart());
        assert_eq!(mgr.warm_restart_state(), WarmRestartState::Initialized);
    }
}
