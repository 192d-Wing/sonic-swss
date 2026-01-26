//! VlanMgr - Core VLAN configuration manager implementation

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, instrument, warn};

use sonic_cfgmgr_common::{shell, CfgMgr, CfgMgrResult, FieldValues, Orch, WarmRestartState};

use crate::commands::{
    build_add_vlan_cmd, build_add_vlan_member_cmd, build_arp_evict_nocarrier_cmd,
    build_remove_vlan_cmd, build_remove_vlan_member_cmd, build_set_vlan_admin_cmd,
    build_set_vlan_mac_cmd, build_set_vlan_mtu_cmd, LAG_PREFIX, VLAN_PREFIX,
};
use crate::tables::{fields, CFG_VLAN_MEMBER_TABLE_NAME, CFG_VLAN_TABLE_NAME};
use crate::types::{TaggingMode, VlanInfo};

/// VlanMgr manages VLAN configuration
///
/// Configuration flow:
/// 1. VLAN table → Linux bridge VLAN operations + APP_VLAN_TABLE
/// 2. VLAN_MEMBER table → Linux bridge member operations + APP_VLAN_MEMBER_TABLE
pub struct VlanMgr {
    /// Active VLANs
    vlans: HashSet<String>,

    /// VLAN information cache (vlan_id -> VlanInfo)
    vlan_info: HashMap<u16, VlanInfo>,

    /// Port to VLAN membership: port -> vlan -> tagging_mode
    port_vlan_member: HashMap<String, HashMap<String, String>>,

    /// Warm restart replay lists
    vlan_replay: HashSet<String>,
    vlan_member_replay: HashSet<String>,
    replay_done: bool,

    /// Global MAC address
    global_mac: Option<String>,

    /// Mock mode for testing
    #[cfg(test)]
    mock_mode: bool,

    /// Captured commands in mock mode
    #[cfg(test)]
    captured_commands: Vec<String>,
}

impl VlanMgr {
    /// Creates a new VlanMgr instance
    pub fn new() -> Self {
        Self {
            vlans: HashSet::new(),
            vlan_info: HashMap::new(),
            port_vlan_member: HashMap::new(),
            vlan_replay: HashSet::new(),
            vlan_member_replay: HashSet::new(),
            replay_done: false,
            global_mac: None,
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            captured_commands: Vec::new(),
        }
    }

    /// Enables mock mode for testing
    #[cfg(test)]
    pub fn with_mock_mode(mut self) -> Self {
        self.mock_mode = true;
        self
    }

    /// Gets captured commands (for testing)
    #[cfg(test)]
    pub fn captured_commands(&self) -> &[String] {
        &self.captured_commands
    }

    /// Execute a shell command (with mock mode support)
    async fn exec(&mut self, cmd: &str) -> CfgMgrResult<()> {
        #[cfg(test)]
        if self.mock_mode {
            self.captured_commands.push(cmd.to_string());
            info!("Mock exec: {}", cmd);
            return Ok(());
        }

        shell::exec(cmd).await?;
        Ok(())
    }

    /// Set global MAC address
    pub fn set_global_mac(&mut self, mac: impl Into<String>) {
        self.global_mac = Some(mac.into());
    }

    /// Check if VLAN MAC is ready
    pub fn is_vlan_mac_ok(&self) -> bool {
        self.global_mac.is_some()
    }

    /// Extract VLAN ID from key like "Vlan100"
    fn extract_vlan_id(key: &str) -> Option<u16> {
        key.strip_prefix(VLAN_PREFIX)?.parse().ok()
    }

    /// Parse VLAN member key "Vlan100|Ethernet0" into (vlan_id, port_alias)
    fn parse_member_key(key: &str) -> Option<(u16, String)> {
        let parts: Vec<&str> = key.split('|').collect();
        if parts.len() != 2 {
            return None;
        }
        let vlan_id = Self::extract_vlan_id(parts[0])?;
        Some((vlan_id, parts[1].to_string()))
    }

    /// Add VLAN interface
    #[instrument(skip(self))]
    pub async fn add_host_vlan(&mut self, vlan_id: u16) -> CfgMgrResult<bool> {
        let mac = match &self.global_mac {
            Some(mac) => mac.clone(),
            None => {
                warn!("Global MAC not set, deferring VLAN {} creation", vlan_id);
                return Ok(false);
            }
        };

        let cmd = build_add_vlan_cmd(vlan_id, &mac);
        self.exec(&cmd).await?;

        // Disable ARP evict on nocarrier
        let arp_cmd = build_arp_evict_nocarrier_cmd(vlan_id);
        let _ = self.exec(&arp_cmd).await; // Ignore errors for this command

        info!("Added VLAN {}", vlan_id);
        Ok(true)
    }

    /// Remove VLAN interface
    #[instrument(skip(self))]
    pub async fn remove_host_vlan(&mut self, vlan_id: u16) -> CfgMgrResult<bool> {
        let cmd = build_remove_vlan_cmd(vlan_id);
        self.exec(&cmd).await?;

        info!("Removed VLAN {}", vlan_id);
        Ok(true)
    }

    /// Set VLAN admin state
    #[instrument(skip(self))]
    pub async fn set_host_vlan_admin_state(
        &mut self,
        vlan_id: u16,
        admin_status: &str,
    ) -> CfgMgrResult<bool> {
        let cmd = build_set_vlan_admin_cmd(vlan_id, admin_status);
        self.exec(&cmd).await?;

        info!("Set VLAN {} admin state to {}", vlan_id, admin_status);
        Ok(true)
    }

    /// Set VLAN MTU
    #[instrument(skip(self))]
    pub async fn set_host_vlan_mtu(&mut self, vlan_id: u16, mtu: u32) -> CfgMgrResult<bool> {
        let cmd = build_set_vlan_mtu_cmd(vlan_id, mtu);
        match self.exec(&cmd).await {
            Ok(_) => {
                info!("Set VLAN {} MTU to {}", vlan_id, mtu);
                Ok(true)
            }
            Err(e) => {
                warn!(
                    "Failed to set VLAN {} MTU: {} (member MTU constraint?)",
                    vlan_id, e
                );
                Ok(false)
            }
        }
    }

    /// Set VLAN MAC address
    #[instrument(skip(self))]
    pub async fn set_host_vlan_mac(&mut self, vlan_id: u16, mac: &str) -> CfgMgrResult<bool> {
        let cmd = build_set_vlan_mac_cmd(vlan_id, mac);
        self.exec(&cmd).await?;

        info!("Set VLAN {} MAC to {}", vlan_id, mac);
        Ok(true)
    }

    /// Add VLAN member
    #[instrument(skip(self))]
    pub async fn add_host_vlan_member(
        &mut self,
        vlan_id: u16,
        port_alias: &str,
        tagging_mode: TaggingMode,
    ) -> CfgMgrResult<bool> {
        let tagging_cmd = tagging_mode.to_bridge_cmd();
        let cmd = build_add_vlan_member_cmd(vlan_id, port_alias, tagging_cmd);

        // Handle LAG race condition with retry
        match self.exec(&cmd).await {
            Ok(_) => {
                info!(
                    "Added {} to VLAN {} as {}",
                    port_alias,
                    vlan_id,
                    tagging_mode.as_str()
                );
                Ok(true)
            }
            Err(e) if port_alias.starts_with(LAG_PREFIX) => {
                warn!("LAG race condition for {}, will retry: {}", port_alias, e);
                Ok(false) // Return false to trigger retry
            }
            Err(e) => Err(e),
        }
    }

    /// Remove VLAN member
    #[instrument(skip(self))]
    pub async fn remove_host_vlan_member(
        &mut self,
        vlan_id: u16,
        port_alias: &str,
    ) -> CfgMgrResult<bool> {
        let cmd = build_remove_vlan_member_cmd(vlan_id, port_alias);
        self.exec(&cmd).await?;

        info!("Removed {} from VLAN {}", port_alias, vlan_id);
        Ok(true)
    }

    /// Process VLAN SET operation
    #[instrument(skip(self, values))]
    pub async fn process_vlan_set(&mut self, key: &str, values: &FieldValues) -> CfgMgrResult<()> {
        if !self.is_vlan_mac_ok() {
            debug!("VLAN MAC not ready, deferring VLAN task");
            return Ok(());
        }

        let vlan_id = match Self::extract_vlan_id(key) {
            Some(id) => id,
            None => {
                warn!("Invalid VLAN key: {}", key);
                return Ok(());
            }
        };

        // Check if this is a new VLAN
        let is_new = !self.vlans.contains(key);

        if is_new {
            // Add VLAN interface
            self.add_host_vlan(vlan_id).await?;
            self.vlans.insert(key.to_string());
            self.vlan_info.insert(vlan_id, VlanInfo::new(vlan_id));
        }

        // Process configuration fields
        for (field, value) in values {
            match field.as_str() {
                fields::ADMIN_STATUS => {
                    self.set_host_vlan_admin_state(vlan_id, value).await?;
                }
                fields::MTU => {
                    if let Ok(mtu) = value.parse::<u32>() {
                        self.set_host_vlan_mtu(vlan_id, mtu).await?;
                    }
                }
                fields::MAC => {
                    self.set_host_vlan_mac(vlan_id, value).await?;
                }
                _ => {
                    debug!("Ignoring unknown VLAN field: {}", field);
                }
            }
        }

        // TODO: Write to APPL_DB (requires ProducerStateTable integration)
        debug!("Would write VLAN {} to APPL_DB", vlan_id);

        Ok(())
    }

    /// Process VLAN DEL operation
    #[instrument(skip(self))]
    pub async fn process_vlan_del(&mut self, key: &str) -> CfgMgrResult<()> {
        let vlan_id = match Self::extract_vlan_id(key) {
            Some(id) => id,
            None => {
                warn!("Invalid VLAN key: {}", key);
                return Ok(());
            }
        };

        // Remove VLAN interface
        self.remove_host_vlan(vlan_id).await?;
        self.vlans.remove(key);
        self.vlan_info.remove(&vlan_id);

        // TODO: Delete from APPL_DB
        debug!("Would delete VLAN {} from APPL_DB", vlan_id);

        Ok(())
    }

    /// Process VLAN_MEMBER SET operation
    #[instrument(skip(self, values))]
    pub async fn process_vlan_member_set(
        &mut self,
        key: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<()> {
        let (vlan_id, port_alias) = match Self::parse_member_key(key) {
            Some(parsed) => parsed,
            None => {
                warn!("Invalid VLAN member key: {}", key);
                return Ok(());
            }
        };

        // Extract tagging mode
        let tagging_mode = values
            .iter()
            .find(|(k, _)| k == fields::TAGGING_MODE)
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(TaggingMode::Tagged);

        // Add member
        self.add_host_vlan_member(vlan_id, &port_alias, tagging_mode)
            .await?;

        // Track membership
        self.port_vlan_member
            .entry(port_alias.clone())
            .or_default()
            .insert(
                format!("Vlan{}", vlan_id),
                tagging_mode.as_str().to_string(),
            );

        // TODO: Write to APPL_DB
        debug!("Would write VLAN member {} to APPL_DB", key);

        Ok(())
    }

    /// Process VLAN_MEMBER DEL operation
    #[instrument(skip(self))]
    pub async fn process_vlan_member_del(&mut self, key: &str) -> CfgMgrResult<()> {
        let (vlan_id, port_alias) = match Self::parse_member_key(key) {
            Some(parsed) => parsed,
            None => {
                warn!("Invalid VLAN member key: {}", key);
                return Ok(());
            }
        };

        // Remove member
        self.remove_host_vlan_member(vlan_id, &port_alias).await?;

        // Update tracking
        if let Some(port_vlans) = self.port_vlan_member.get_mut(&port_alias) {
            port_vlans.remove(&format!("Vlan{}", vlan_id));
        }

        // TODO: Delete from APPL_DB
        debug!("Would delete VLAN member {} from APPL_DB", key);

        Ok(())
    }
}

impl Default for VlanMgr {
    fn default() -> Self {
        Self::new()
    }
}

/// Orch trait implementation
#[async_trait]
impl Orch for VlanMgr {
    fn name(&self) -> &str {
        "vlanmgr"
    }

    async fn do_task(&mut self) {
        // Placeholder - actual implementation would:
        // 1. Drain consumers for VLAN and VLAN_MEMBER tables
        // 2. Process SET/DEL operations
        // 3. Write to APPL_DB via producers
        debug!("do_task called (placeholder)");
    }
}

/// CfgMgr trait implementation
#[async_trait]
impl CfgMgr for VlanMgr {
    fn daemon_name(&self) -> &str {
        "vlanmgrd"
    }

    fn is_warm_restart(&self) -> bool {
        !self.vlan_replay.is_empty() || !self.vlan_member_replay.is_empty()
    }

    fn warm_restart_state(&self) -> WarmRestartState {
        if self.replay_done {
            WarmRestartState::Reconciled
        } else if self.is_warm_restart() {
            WarmRestartState::Replayed
        } else {
            WarmRestartState::Disabled
        }
    }

    async fn set_warm_restart_state(&mut self, _state: WarmRestartState) {
        // State transitions handled internally
    }

    fn config_table_names(&self) -> &[&str] {
        &[CFG_VLAN_TABLE_NAME, CFG_VLAN_MEMBER_TABLE_NAME]
    }

    fn state_table_names(&self) -> &[&str] {
        &[] // TODO: Add STATE_DB tables when needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vlan_mgr_new() {
        let mgr = VlanMgr::new();
        assert!(mgr.vlans.is_empty());
        assert!(mgr.vlan_info.is_empty());
        assert!(!mgr.is_vlan_mac_ok());
    }

    #[test]
    fn test_extract_vlan_id() {
        assert_eq!(VlanMgr::extract_vlan_id("Vlan100"), Some(100));
        assert_eq!(VlanMgr::extract_vlan_id("Vlan1"), Some(1));
        assert_eq!(VlanMgr::extract_vlan_id("Invalid"), None);
    }

    #[test]
    fn test_parse_member_key() {
        let (vlan_id, port) = VlanMgr::parse_member_key("Vlan100|Ethernet0").unwrap();
        assert_eq!(vlan_id, 100);
        assert_eq!(port, "Ethernet0");

        assert!(VlanMgr::parse_member_key("Invalid").is_none());
    }

    #[tokio::test]
    async fn test_add_host_vlan() {
        let mut mgr = VlanMgr::new().with_mock_mode();
        mgr.set_global_mac("00:11:22:33:44:55");

        let result = mgr.add_host_vlan(100).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        let cmds = mgr.captured_commands();
        assert!(cmds.iter().any(|c| c.contains("vlan add vid 100")));
        assert!(cmds.iter().any(|c| c.contains("Vlan100")));
    }

    #[tokio::test]
    async fn test_remove_host_vlan() {
        let mut mgr = VlanMgr::new().with_mock_mode();

        let result = mgr.remove_host_vlan(100).await;
        assert!(result.is_ok());

        let cmds = mgr.captured_commands();
        assert!(cmds.iter().any(|c| c.contains("ip link del Vlan100")));
    }

    #[tokio::test]
    async fn test_set_vlan_admin_state() {
        let mut mgr = VlanMgr::new().with_mock_mode();

        mgr.set_host_vlan_admin_state(100, "down").await.unwrap();

        let cmds = mgr.captured_commands();
        assert!(cmds
            .iter()
            .any(|c| c.contains("Vlan100") && c.contains("down")));
    }

    #[tokio::test]
    async fn test_add_vlan_member() {
        let mut mgr = VlanMgr::new().with_mock_mode();

        let result = mgr
            .add_host_vlan_member(100, "Ethernet0", TaggingMode::Untagged)
            .await;
        assert!(result.is_ok());

        let cmds = mgr.captured_commands();
        assert!(cmds
            .iter()
            .any(|c| c.contains("Ethernet0") && c.contains("pvid untagged")));
    }

    #[tokio::test]
    async fn test_process_vlan_set() {
        let mut mgr = VlanMgr::new().with_mock_mode();
        mgr.set_global_mac("00:11:22:33:44:55");

        let fields = vec![
            ("admin_status".to_string(), "up".to_string()),
            ("mtu".to_string(), "1500".to_string()),
        ];

        mgr.process_vlan_set("Vlan100", &fields).await.unwrap();

        assert!(mgr.vlans.contains("Vlan100"));
        let cmds = mgr.captured_commands();
        assert!(cmds.iter().any(|c| c.contains("Vlan100")));
    }

    #[tokio::test]
    async fn test_process_vlan_member_set() {
        let mut mgr = VlanMgr::new().with_mock_mode();

        let fields = vec![("tagging_mode".to_string(), "untagged".to_string())];

        mgr.process_vlan_member_set("Vlan100|Ethernet0", &fields)
            .await
            .unwrap();

        let cmds = mgr.captured_commands();
        assert!(cmds
            .iter()
            .any(|c| c.contains("Ethernet0") && c.contains("pvid untagged")));
    }

    #[test]
    fn test_cfgmgr_trait() {
        let mgr = VlanMgr::new();
        assert_eq!(mgr.daemon_name(), "vlanmgrd");
        assert!(!mgr.is_warm_restart());

        let tables = mgr.config_table_names();
        assert_eq!(tables.len(), 2);
        assert!(tables.contains(&"VLAN"));
        assert!(tables.contains(&"VLAN_MEMBER"));
    }

    #[test]
    fn test_orch_trait() {
        let mgr = VlanMgr::new();
        assert_eq!(mgr.name(), "vlanmgr");
    }
}
