//! VRF Manager - Core VRF lifecycle and EVPN/VXLAN management

use std::collections::{BTreeSet, HashMap};

use async_trait::async_trait;
use sonic_cfgmgr_common::{
    shell, CfgMgr, CfgMgrError, CfgMgrResult, FieldValues, WarmRestartState,
};
use sonic_orch_common::Orch;
use tracing::{debug, info, instrument};

use crate::commands::*;
use crate::tables::fields;
use crate::types::*;

/// VRF Manager
///
/// Manages VRF lifecycle, routing table allocation, and EVPN/VXLAN integration
pub struct VrfMgr {
    /// VRF name -> routing table ID mapping
    vrf_table_map: HashMap<String, u32>,

    /// Available routing table IDs (1001-2000)
    free_tables: BTreeSet<u32>,

    /// VRF name -> VNI mapping (for EVPN)
    vrf_vni_map: HashMap<String, u32>,

    /// EVPN VXLAN tunnel name
    evpn_vxlan_tunnel: Option<String>,

    /// Testing support
    #[cfg(test)]
    mock_mode: bool,
    #[cfg(test)]
    captured_commands: Vec<String>,
}

impl VrfMgr {
    /// Create a new VrfMgr instance
    pub fn new() -> Self {
        // Initialize routing table pool (1001-2000)
        let mut free_tables = BTreeSet::new();
        for table_id in VRF_TABLE_START..VRF_TABLE_END {
            free_tables.insert(table_id);
        }

        info!(
            "VrfMgr initialized with {} free routing tables",
            free_tables.len()
        );

        Self {
            vrf_table_map: HashMap::new(),
            free_tables,
            vrf_vni_map: HashMap::new(),
            evpn_vxlan_tunnel: None,
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            captured_commands: Vec::new(),
        }
    }

    /// Allocate a free routing table ID
    fn get_free_table(&mut self) -> Option<u32> {
        let table_id = self.free_tables.iter().next().copied()?;
        self.free_tables.remove(&table_id);
        debug!("Allocated routing table ID {}", table_id);
        Some(table_id)
    }

    /// Return a routing table ID to the pool
    fn recycle_table(&mut self, table_id: u32) {
        self.free_tables.insert(table_id);
        debug!("Recycled routing table ID {}", table_id);
    }

    /// Create VRF device
    #[instrument(skip(self))]
    pub async fn set_link(&mut self, vrf_name: &str) -> CfgMgrResult<bool> {
        // Check if VRF already exists
        if self.vrf_table_map.contains_key(vrf_name) {
            debug!("VRF {} already exists", vrf_name);
            return Ok(true);
        }

        // Special handling for mgmt VRF (pre-created by hostcfgd)
        if vrf_name == MGMT_VRF_NAME {
            self.vrf_table_map
                .insert(vrf_name.to_string(), MGMT_VRF_TABLE_ID);
            info!("Registered mgmt VRF with table ID {}", MGMT_VRF_TABLE_ID);
            return Ok(true);
        }

        // Allocate routing table ID
        let table_id = self
            .get_free_table()
            .ok_or_else(|| CfgMgrError::internal("No free routing tables available"))?;

        // Create VRF device
        let add_cmd = build_add_vrf_cmd(vrf_name, table_id);
        self.exec(&add_cmd).await?;

        // Bring up VRF device
        let up_cmd = build_set_vrf_up_cmd(vrf_name);
        self.exec(&up_cmd).await?;

        self.vrf_table_map.insert(vrf_name.to_string(), table_id);
        info!("Created VRF {} with table ID {}", vrf_name, table_id);

        Ok(true)
    }

    /// Delete VRF device
    #[instrument(skip(self))]
    pub async fn del_link(&mut self, vrf_name: &str) -> CfgMgrResult<bool> {
        let table_id = match self.vrf_table_map.get(vrf_name) {
            Some(&id) => id,
            None => {
                debug!("VRF {} does not exist", vrf_name);
                return Ok(false);
            }
        };

        // Don't delete mgmt VRF device (managed by hostcfgd)
        if vrf_name == MGMT_VRF_NAME {
            self.recycle_table(table_id);
            self.vrf_table_map.remove(vrf_name);
            info!("Unregistered mgmt VRF (device not deleted)");
            return Ok(true);
        }

        // Delete VRF device
        let cmd = build_del_vrf_cmd(vrf_name);
        self.exec(&cmd).await?;

        self.recycle_table(table_id);
        self.vrf_table_map.remove(vrf_name);
        info!("Deleted VRF {} (table ID {} recycled)", vrf_name, table_id);

        Ok(true)
    }

    /// Get VNI for a VRF (for EVPN)
    pub fn get_vrf_mapped_vni(&self, vrf_name: &str) -> Option<u32> {
        self.vrf_vni_map.get(vrf_name).copied()
    }

    /// Process VRF SET operation (CONFIG_DB)
    #[instrument(skip(self))]
    pub async fn process_vrf_set(&mut self, key: &str, _values: &FieldValues) -> CfgMgrResult<()> {
        let vrf_name = key;

        // Create VRF device
        self.set_link(vrf_name).await?;

        // TODO: Write to APPL_DB VRF_TABLE and VNET_TABLE
        debug!("Would write VRF {} to APPL_DB", vrf_name);

        Ok(())
    }

    /// Process VRF DEL operation (CONFIG_DB)
    #[instrument(skip(self))]
    pub async fn process_vrf_del(&mut self, key: &str) -> CfgMgrResult<()> {
        let vrf_name = key;

        // Delete VRF device
        self.del_link(vrf_name).await?;

        // Remove VNI mapping if exists
        if let Some(vni) = self.vrf_vni_map.remove(vrf_name) {
            info!("Removed VRF {} VNI mapping (VNI {})", vrf_name, vni);
        }

        // TODO: Delete from APPL_DB VRF_TABLE and VNET_TABLE
        debug!("Would delete VRF {} from APPL_DB", vrf_name);

        Ok(())
    }

    /// Process VXLAN_TUNNEL SET operation (VRF-VNI mapping)
    #[instrument(skip(self))]
    pub async fn process_vxlan_tunnel_set(
        &mut self,
        key: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<()> {
        let vrf_name = key;

        // Extract VNI from CONFIG_DB
        let vni = values
            .iter()
            .find(|(k, _)| k == fields::VNI)
            .and_then(|(_, v)| v.parse::<u32>().ok())
            .ok_or_else(|| CfgMgrError::invalid_config("vni", "Missing or invalid VNI field"))?;

        // Store VRF-VNI mapping
        self.vrf_vni_map.insert(vrf_name.to_string(), vni);
        info!("Mapped VRF {} to VNI {}", vrf_name, vni);

        // If EVPN tunnel is configured, write to APPL_DB
        if let Some(ref tunnel) = self.evpn_vxlan_tunnel {
            self.update_vxlan_vrf_table(vrf_name, vni, tunnel, true)
                .await?;
        }

        Ok(())
    }

    /// Process VXLAN_TUNNEL DEL operation
    #[instrument(skip(self))]
    pub async fn process_vxlan_tunnel_del(&mut self, key: &str) -> CfgMgrResult<()> {
        let vrf_name = key;

        if let Some(vni) = self.vrf_vni_map.remove(vrf_name) {
            info!("Removed VRF {} VNI mapping (VNI {})", vrf_name, vni);

            // If EVPN tunnel is configured, remove from APPL_DB
            if let Some(ref tunnel) = self.evpn_vxlan_tunnel {
                self.update_vxlan_vrf_table(vrf_name, vni, tunnel, false)
                    .await?;
            }
        }

        Ok(())
    }

    /// Process EVPN_NVO SET operation
    #[instrument(skip(self))]
    pub async fn process_evpn_nvo_set(
        &mut self,
        _key: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<()> {
        // Extract VXLAN tunnel name
        let tunnel = values
            .iter()
            .find(|(k, _)| k == fields::SOURCE_VTEP)
            .map(|(_, v): &(String, String)| v.clone())
            .unwrap_or_else(|| "vtep".to_string());

        self.evpn_vxlan_tunnel = Some(tunnel.clone());
        info!("Configured EVPN VXLAN tunnel: {}", tunnel);

        // Sync all VRF-VNI mappings to APPL_DB
        self.sync_vxlan_vrf_table(true).await?;

        Ok(())
    }

    /// Process EVPN_NVO DEL operation
    #[instrument(skip(self))]
    pub async fn process_evpn_nvo_del(&mut self, _key: &str) -> CfgMgrResult<()> {
        if let Some(tunnel) = self.evpn_vxlan_tunnel.take() {
            info!("Removed EVPN VXLAN tunnel: {}", tunnel);

            // Remove all VRF-VNI mappings from APPL_DB
            self.sync_vxlan_vrf_table(false).await?;
        }

        Ok(())
    }

    /// Update VXLAN_VRF_TABLE in APPL_DB
    async fn update_vxlan_vrf_table(
        &self,
        vrf_name: &str,
        vni: u32,
        tunnel: &str,
        add: bool,
    ) -> CfgMgrResult<()> {
        // TODO: Write to or delete from APPL_DB VXLAN_VRF_TABLE
        if add {
            debug!(
                "Would write VXLAN_VRF_TABLE: {} -> VNI {} via {}",
                vrf_name, vni, tunnel
            );
        } else {
            debug!("Would delete VXLAN_VRF_TABLE: {}", vrf_name);
        }
        Ok(())
    }

    /// Sync all VRF-VNI mappings to APPL_DB
    async fn sync_vxlan_vrf_table(&self, add: bool) -> CfgMgrResult<()> {
        let tunnel = match &self.evpn_vxlan_tunnel {
            Some(t) => t,
            None => return Ok(()),
        };

        for (vrf_name, &vni) in &self.vrf_vni_map {
            self.update_vxlan_vrf_table(vrf_name, vni, tunnel, add)
                .await?;
        }

        info!(
            "Synced {} VRF-VNI mappings to APPL_DB ({})",
            self.vrf_vni_map.len(),
            if add { "add" } else { "delete" }
        );

        Ok(())
    }

    /// Execute shell command (with mock mode support)
    async fn exec(&mut self, cmd: &str) -> CfgMgrResult<()> {
        #[cfg(test)]
        if self.mock_mode {
            self.captured_commands.push(cmd.to_string());
            return Ok(());
        }

        shell::exec(cmd).await?;
        Ok(())
    }

    #[cfg(test)]
    pub fn with_mock_mode(mut self) -> Self {
        self.mock_mode = true;
        self
    }

    #[cfg(test)]
    pub fn captured_commands(&self) -> &[String] {
        &self.captured_commands
    }
}

impl Default for VrfMgr {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Orch for VrfMgr {
    fn name(&self) -> &str {
        "vrfmgr"
    }

    async fn do_task(&mut self) {
        // TODO: Implement event loop processing
        // This will be called by the daemon when there are pending tasks
    }
}

#[async_trait]
impl CfgMgr for VrfMgr {
    fn daemon_name(&self) -> &str {
        "vrfmgrd"
    }

    fn is_warm_restart(&self) -> bool {
        // TODO: Implement warm restart detection
        false
    }

    fn warm_restart_state(&self) -> WarmRestartState {
        WarmRestartState::Disabled
    }

    async fn set_warm_restart_state(&mut self, _state: WarmRestartState) {
        // TODO: Implement warm restart state transitions
    }

    fn config_table_names(&self) -> &[&str] {
        &["VRF", "VXLAN_TUNNEL", "EVPN_NVO", "MGMT_VRF_CONFIG"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vrfmgr_new() {
        let mgr = VrfMgr::new();
        assert_eq!(mgr.vrf_table_map.len(), 0);
        assert_eq!(
            mgr.free_tables.len(),
            (VRF_TABLE_END - VRF_TABLE_START) as usize
        );
        assert_eq!(mgr.vrf_vni_map.len(), 0);
        assert_eq!(mgr.evpn_vxlan_tunnel, None);
    }

    #[test]
    fn test_table_allocation() {
        let mut mgr = VrfMgr::new();
        let table1 = mgr.get_free_table().unwrap();
        let table2 = mgr.get_free_table().unwrap();

        assert_eq!(table1, VRF_TABLE_START);
        assert_eq!(table2, VRF_TABLE_START + 1);

        mgr.recycle_table(table1);
        let table3 = mgr.get_free_table().unwrap();
        assert_eq!(table3, table1);
    }

    #[test]
    fn test_table_exhaustion() {
        let mut mgr = VrfMgr::new();

        // Allocate all tables
        for _ in VRF_TABLE_START..VRF_TABLE_END {
            assert!(mgr.get_free_table().is_some());
        }

        // Should return None when exhausted
        assert_eq!(mgr.get_free_table(), None);
    }

    #[tokio::test]
    async fn test_set_link() {
        let mut mgr = VrfMgr::new().with_mock_mode();

        mgr.set_link("Vrf1").await.unwrap();

        assert_eq!(mgr.vrf_table_map.get("Vrf1"), Some(&VRF_TABLE_START));
        let cmds = mgr.captured_commands();
        assert!(cmds
            .iter()
            .any(|c| c.contains("ip link add") && c.contains("Vrf1")));
        assert!(cmds
            .iter()
            .any(|c| c.contains("ip link set") && c.contains("Vrf1") && c.contains("up")));
    }

    #[tokio::test]
    async fn test_mgmt_vrf_special_handling() {
        let mut mgr = VrfMgr::new().with_mock_mode();

        mgr.set_link(MGMT_VRF_NAME).await.unwrap();

        // Should use MGMT_VRF_TABLE_ID without creating device
        assert_eq!(
            mgr.vrf_table_map.get(MGMT_VRF_NAME),
            Some(&MGMT_VRF_TABLE_ID)
        );
        assert_eq!(mgr.captured_commands().len(), 0); // No shell commands
    }

    #[tokio::test]
    async fn test_del_link() {
        let mut mgr = VrfMgr::new().with_mock_mode();

        mgr.set_link("Vrf1").await.unwrap();
        let table_id = *mgr.vrf_table_map.get("Vrf1").unwrap();

        mgr.del_link("Vrf1").await.unwrap();

        assert!(!mgr.vrf_table_map.contains_key("Vrf1"));
        assert!(mgr.free_tables.contains(&table_id));
    }

    #[tokio::test]
    async fn test_vrf_vni_mapping() {
        let mut mgr = VrfMgr::new().with_mock_mode();

        let fields = vec![("vni".to_string(), "1000".to_string())];
        mgr.process_vxlan_tunnel_set("Vrf1", &fields).await.unwrap();

        assert_eq!(mgr.get_vrf_mapped_vni("Vrf1"), Some(1000));
    }

    #[tokio::test]
    async fn test_evpn_nvo_configuration() {
        let mut mgr = VrfMgr::new().with_mock_mode();

        // Add VRF-VNI mapping first
        let fields = vec![("vni".to_string(), "1000".to_string())];
        mgr.process_vxlan_tunnel_set("Vrf1", &fields).await.unwrap();

        // Configure EVPN NVO
        let nvo_fields = vec![("source_vtep".to_string(), "vtep1".to_string())];
        mgr.process_evpn_nvo_set("nvo1", &nvo_fields).await.unwrap();

        assert_eq!(mgr.evpn_vxlan_tunnel, Some("vtep1".to_string()));
    }

    #[tokio::test]
    async fn test_process_vrf_set() {
        let mut mgr = VrfMgr::new().with_mock_mode();

        let fields = vec![];
        mgr.process_vrf_set("Vrf1", &fields).await.unwrap();

        assert!(mgr.vrf_table_map.contains_key("Vrf1"));
    }

    #[tokio::test]
    async fn test_process_vrf_del() {
        let mut mgr = VrfMgr::new().with_mock_mode();

        // Create VRF first
        let fields = vec![];
        mgr.process_vrf_set("Vrf1", &fields).await.unwrap();

        // Delete VRF
        mgr.process_vrf_del("Vrf1").await.unwrap();

        assert!(!mgr.vrf_table_map.contains_key("Vrf1"));
    }

    #[test]
    fn test_cfgmgr_trait() {
        let mgr = VrfMgr::new();
        assert_eq!(mgr.daemon_name(), "vrfmgrd");
        assert!(!mgr.is_warm_restart());
    }
}
