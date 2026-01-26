//! Interface Manager - Core implementation

use async_trait::async_trait;
use sonic_cfgmgr_common::{
    shell, CfgMgr, CfgMgrResult, FieldValues, FieldValuesExt, WarmRestartState,
};
use sonic_orch_common::Orch;
use sonic_types::IpPrefix;
use tracing::{debug, info};

use crate::tables::*;
use crate::types::*;

/// Interface Manager
///
/// Manages network interface configuration including:
/// - IP addresses (IPv4/IPv6)
/// - VRF binding
/// - Sub-interfaces
/// - MPLS, proxy ARP, gratuitous ARP
/// - Warm restart
pub struct IntfMgr {
    /// Sub-interface tracking
    subintf_list: SubIntfMap,

    /// Loopback interfaces
    loopback_intf_list: LoopbackIntfSet,

    /// Warm restart: pending replay list
    pending_replay_intf_list: PendingReplayIntfSet,

    /// IPv6 link-local mode interfaces
    ipv6_link_local_mode_list: Ipv6LinkLocalModeSet,

    /// Switch type (normal or VOQ)
    switch_type: SwitchType,

    /// Warm restart replay done flag
    replay_done: bool,

    #[cfg(test)]
    mock_mode: bool,
}

impl IntfMgr {
    /// Create a new IntfMgr with specified switch type
    pub fn new(switch_type: SwitchType) -> Self {
        info!("IntfMgr initialized with switch type: {:?}", switch_type);

        Self {
            subintf_list: SubIntfMap::new(),
            loopback_intf_list: LoopbackIntfSet::new(),
            pending_replay_intf_list: PendingReplayIntfSet::new(),
            ipv6_link_local_mode_list: Ipv6LinkLocalModeSet::new(),
            switch_type,
            replay_done: false,
            #[cfg(test)]
            mock_mode: false,
        }
    }

    #[cfg(test)]
    pub fn new_mock(switch_type: SwitchType) -> Self {
        let mut mgr = Self::new(switch_type);
        mgr.mock_mode = true;
        mgr
    }

    /// Check if interface state is OK
    ///
    /// Queries STATE_DB to check if interface is ready
    fn is_intf_state_ok(&self, alias: &str) -> bool {
        // TODO: Query STATE_DB based on interface type
        // Physical → STATE_PORT_TABLE
        // LAG → STATE_LAG_TABLE
        // VLAN → STATE_VLAN_TABLE
        // For now, assume ready in mock mode
        #[cfg(test)]
        if self.mock_mode {
            return true;
        }

        debug!("Checking state for interface {}", alias);
        true // TODO: Implement STATE_DB check
    }

    /// Handle INTERFACE table general config (VRF, MPLS, etc.)
    pub async fn do_intf_general_task(
        &mut self,
        alias: &str,
        op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        if op == "SET" {
            // Handle VRF binding
            if let Some(vrf_name) = values.get_field(intf_fields::VRF_NAME) {
                if !vrf_name.is_empty() {
                    crate::vrf_operations::set_intf_vrf(alias, Some(vrf_name)).await?;
                } else {
                    crate::vrf_operations::set_intf_vrf(alias, None).await?;
                }
            }

            // Handle MPLS
            if let Some(mpls) = values.get_field(intf_fields::MPLS) {
                crate::vrf_operations::set_intf_mpls(alias, mpls).await?;
            }

            // Handle proxy ARP
            if let Some(proxy_arp) = values.get_field(intf_fields::PROXY_ARP) {
                crate::vrf_operations::set_intf_proxy_arp(alias, proxy_arp).await?;
            }

            // Handle gratuitous ARP
            if let Some(grat_arp) = values.get_field(intf_fields::GRAT_ARP) {
                crate::vrf_operations::set_intf_grat_arp(alias, grat_arp).await?;
            }

            // Handle MAC address
            if let Some(mac_addr) = values.get_field(intf_fields::MAC_ADDR) {
                crate::ip_operations::set_intf_mac(alias, mac_addr).await?;
            }

            // Handle IPv6 link-local only mode
            if let Some(ipv6_ll_only) = values.get_field(intf_fields::IPV6_USE_LINK_LOCAL_ONLY) {
                if ipv6_ll_only == "enable" {
                    self.ipv6_link_local_mode_list.insert(alias.to_string());
                } else {
                    self.ipv6_link_local_mode_list.remove(alias);
                }
            }

            // TODO: Write to APPL_DB INTF_TABLE
        } else if op == "DEL" {
            // Clean up interface config
            self.ipv6_link_local_mode_list.remove(alias);
            // TODO: Delete from APPL_DB
        }

        Ok(true)
    }

    /// Handle INTERFACE|<alias>|<ip_prefix> IP address config
    pub async fn do_intf_addr_task(
        &mut self,
        alias: &str,
        ip_prefix_str: &str,
        op: &str,
    ) -> CfgMgrResult<bool> {
        // Parse IP prefix
        let ip_prefix = IpPrefix::parse(ip_prefix_str).map_err(|e| {
            sonic_cfgmgr_common::CfgMgrError::internal(format!("Invalid IP prefix: {}", e))
        })?;

        if op == "SET" {
            // Check if interface is ready
            if !self.is_intf_state_ok(alias) {
                info!("Interface {} is not ready, deferring IP config", alias);
                return Ok(false); // Retry later
            }

            // Add IP address
            crate::ip_operations::set_intf_ip(alias, "add", &ip_prefix, &self.switch_type).await?;

            info!("Added IP address {} to interface {}", ip_prefix_str, alias);

            // TODO: Write to APPL_DB INTF_TABLE with scope and family
        } else if op == "DEL" {
            // Remove IP address
            crate::ip_operations::set_intf_ip(alias, "del", &ip_prefix, &self.switch_type).await?;

            info!(
                "Removed IP address {} from interface {}",
                ip_prefix_str, alias
            );

            // TODO: Delete from APPL_DB INTF_TABLE
        }

        Ok(true)
    }

    /// Handle sub-interface creation
    pub async fn handle_subintf_create(
        &mut self,
        subintf: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        // Parse sub-interface name
        let (parent, vlan_id) = crate::subintf::parse_subintf_name(subintf).ok_or_else(|| {
            sonic_cfgmgr_common::CfgMgrError::internal("Invalid sub-interface name")
        })?;

        // Check if parent interface is ready
        if !self.is_intf_state_ok(&parent) {
            info!(
                "Parent interface {} is not ready, deferring sub-interface creation",
                parent
            );
            return Ok(false); // Retry later
        }

        // Create sub-interface
        crate::subintf_operations::add_host_subintf(&parent, subintf, &vlan_id).await?;

        // Get MTU and admin status
        let mtu = values.get_field(subintf_fields::MTU).unwrap_or_default();
        let admin_status = values
            .get_field(subintf_fields::ADMIN_STATUS)
            .unwrap_or_default();

        // Track in subintf_list
        self.subintf_list.insert(
            subintf.to_string(),
            SubIntfInfo {
                vlan_id,
                mtu: mtu.to_string(),
                admin_status: admin_status.to_string(),
                curr_admin_status: String::new(),
            },
        );

        info!("Created sub-interface {}", subintf);

        // TODO: Set MTU and admin status
        // TODO: Write to STATE_DB INTERFACE_TABLE

        Ok(true)
    }

    /// Handle sub-interface deletion
    pub async fn handle_subintf_delete(&mut self, subintf: &str) -> CfgMgrResult<bool> {
        // Remove sub-interface
        crate::subintf_operations::remove_host_subintf(subintf).await?;

        // Remove from tracking
        self.subintf_list.remove(subintf);

        info!("Deleted sub-interface {}", subintf);

        // TODO: Remove from STATE_DB INTERFACE_TABLE

        Ok(true)
    }

    /// Add loopback interface
    pub async fn add_loopback_intf(&mut self, alias: &str) -> CfgMgrResult<()> {
        let cmd = format!(
            "{} link add {} type dummy",
            IP_CMD,
            shell::shellquote(alias)
        );
        sonic_cfgmgr_common::shell::exec(&cmd).await?;

        // Set loopback up
        let cmd = format!("{} link set {} up", IP_CMD, shell::shellquote(alias));
        sonic_cfgmgr_common::shell::exec(&cmd).await?;

        // Set default MTU
        let cmd = format!(
            "{} link set {} mtu {}",
            IP_CMD,
            shell::shellquote(alias),
            LOOPBACK_DEFAULT_MTU
        );
        sonic_cfgmgr_common::shell::exec(&cmd).await?;

        self.loopback_intf_list.insert(alias.to_string());
        info!("Added loopback interface {}", alias);

        Ok(())
    }

    /// Delete loopback interface
    pub async fn del_loopback_intf(&mut self, alias: &str) -> CfgMgrResult<()> {
        let cmd = format!("{} link del {}", IP_CMD, shell::shellquote(alias));
        sonic_cfgmgr_common::shell::exec(&cmd).await?;

        self.loopback_intf_list.remove(alias);
        info!("Deleted loopback interface {}", alias);

        Ok(())
    }

    /// Build interface replay list for warm restart
    pub fn build_intf_replay_list(&mut self) {
        // TODO: Read all interfaces from CONFIG_DB
        // TODO: Build list of interfaces to replay
        info!("Built warm restart replay list");
    }

    /// Set warm restart done state
    pub fn set_warm_replay_done_state(&mut self) {
        // TODO: Write to STATE_DB WARM_RESTART_TABLE
        self.replay_done = true;
        info!("Warm restart replay complete");
    }
}

#[async_trait]
impl Orch for IntfMgr {
    fn name(&self) -> &str {
        "intfmgr"
    }

    async fn do_task(&mut self) {
        // TODO: Process consumers when integrating with ConsumerStateTable
    }
}

#[async_trait]
impl CfgMgr for IntfMgr {
    fn daemon_name(&self) -> &str {
        "intfmgrd"
    }

    fn is_warm_restart(&self) -> bool {
        // TODO: Check warm restart state
        false
    }

    fn warm_restart_state(&self) -> WarmRestartState {
        if self.replay_done {
            WarmRestartState::Disabled
        } else {
            WarmRestartState::Reconciled
        }
    }

    async fn set_warm_restart_state(&mut self, _state: WarmRestartState) {
        // TODO: Write to STATE_DB WARM_RESTART_TABLE
    }

    fn config_table_names(&self) -> &[&str] {
        &[
            CFG_INTF_TABLE,
            CFG_VLAN_INTF_TABLE,
            CFG_LAG_INTF_TABLE,
            CFG_LOOPBACK_INTF_TABLE,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intf_mgr_new() {
        let mgr = IntfMgr::new(SwitchType::Normal);
        assert_eq!(mgr.switch_type, SwitchType::Normal);
        assert!(mgr.subintf_list.is_empty());
        assert!(mgr.loopback_intf_list.is_empty());
        assert!(!mgr.replay_done);
    }

    #[test]
    fn test_intf_mgr_new_voq() {
        let mgr = IntfMgr::new(SwitchType::Voq);
        assert_eq!(mgr.switch_type, SwitchType::Voq);
    }

    #[test]
    fn test_is_intf_state_ok_mock() {
        let mgr = IntfMgr::new_mock(SwitchType::Normal);
        assert!(mgr.is_intf_state_ok("Ethernet0"));
    }

    #[test]
    fn test_ipv6_link_local_mode_tracking() {
        let mut mgr = IntfMgr::new(SwitchType::Normal);
        mgr.ipv6_link_local_mode_list
            .insert("Ethernet0".to_string());

        assert!(mgr.ipv6_link_local_mode_list.contains("Ethernet0"));

        mgr.ipv6_link_local_mode_list.remove("Ethernet0");
        assert!(!mgr.ipv6_link_local_mode_list.contains("Ethernet0"));
    }

    #[test]
    fn test_subintf_tracking() {
        let mut mgr = IntfMgr::new(SwitchType::Normal);

        let info = SubIntfInfo::new("100".to_string());
        mgr.subintf_list.insert("Ethernet0.100".to_string(), info);

        assert!(mgr.subintf_list.contains_key("Ethernet0.100"));
        assert_eq!(mgr.subintf_list["Ethernet0.100"].vlan_id, "100");
    }
}
