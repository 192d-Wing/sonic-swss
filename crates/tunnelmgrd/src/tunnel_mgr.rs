//! Tunnel Manager - Core tunnel lifecycle and route management

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use sonic_cfgmgr_common::{
    shell, CfgMgr, CfgMgrError, CfgMgrResult, FieldValues, FieldValuesExt, WarmRestartState,
};
use sonic_orch_common::Orch;
use tracing::{info, warn};

use crate::commands::*;
use crate::tables::{
    decap_term_fields, tunnel_fields, CFG_LOOPBACK_INTERFACE_TABLE, CFG_TUNNEL_TABLE,
};
use crate::types::*;

/// Tunnel Manager
///
/// Manages IP-in-IP tunnel lifecycle, route management, and APPL_DB synchronization
pub struct TunnelMgr {
    /// Tunnel configuration cache
    tunnel_cache: HashMap<String, TunnelInfo>,

    /// Loopback interface IP cache
    intf_cache: HashMap<String, IpPrefix>,

    /// Peer switch IP address (remote tunnel endpoint)
    peer_ip: Option<String>,

    /// Warm restart replay list
    tunnel_replay: HashSet<String>,

    /// Warm restart completion flag
    replay_done: bool,

    #[cfg(test)]
    mock_mode: bool,

    #[cfg(test)]
    captured_commands: Vec<String>,
}

impl TunnelMgr {
    /// Create a new TunnelMgr instance
    pub fn new() -> Self {
        info!("TunnelMgr initialized");

        Self {
            tunnel_cache: HashMap::new(),
            intf_cache: HashMap::new(),
            peer_ip: None,
            tunnel_replay: HashSet::new(),
            replay_done: false,
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            captured_commands: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn new_mock() -> Self {
        let mut mgr = Self::new();
        mgr.mock_mode = true;
        mgr
    }

    #[cfg(test)]
    pub fn with_peer_ip(mut self, peer_ip: String) -> Self {
        self.peer_ip = Some(peer_ip);
        self
    }

    /// Initialize peer IP from CONFIG_DB
    pub async fn init_peer_ip(&mut self) -> CfgMgrResult<()> {
        // TODO: Read from PEER_SWITCH table in CONFIG_DB
        // For now, this will be called during doTask
        Ok(())
    }

    /// Initialize warm restart replay list
    pub async fn init_warm_restart(&mut self) -> CfgMgrResult<()> {
        // TODO: Read all tunnel keys from CONFIG_DB
        // and populate tunnel_replay
        Ok(())
    }

    /// Cleanup existing tunnel interface on startup
    pub async fn cleanup_tunnel_interface(&mut self) -> CfgMgrResult<()> {
        let cmd = build_del_tunnel_cmd();
        // Ignore errors - tunnel may not exist
        let _ = self.exec(&cmd).await;
        Ok(())
    }

    /// Execute shell command (or capture in mock mode)
    async fn exec(&mut self, cmd: &str) -> CfgMgrResult<String> {
        #[cfg(test)]
        if self.mock_mode {
            self.captured_commands.push(cmd.to_string());
            return Ok(String::new());
        }

        shell::exec_or_throw(cmd).await
    }

    /// Handle TUNNEL table SET/DEL operations
    pub async fn do_tunnel_task(
        &mut self,
        tunnel_name: &str,
        op: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        if op == "SET" {
            self.do_tunnel_add(tunnel_name, values).await
        } else if op == "DEL" {
            self.do_tunnel_del(tunnel_name).await
        } else {
            Err(CfgMgrError::invalid_config(
                "op",
                format!("Unknown operation: {}", op),
            ))
        }
    }

    async fn do_tunnel_add(
        &mut self,
        tunnel_name: &str,
        values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        let dst_ip = values
            .get_field(tunnel_fields::DST_IP)
            .ok_or_else(|| CfgMgrError::invalid_config("dst_ip", "Missing dst_ip field"))?
            .to_string();

        let tunnel_type = values
            .get_field(tunnel_fields::TUNNEL_TYPE)
            .ok_or_else(|| CfgMgrError::invalid_config("tunnel_type", "Missing tunnel_type"))?
            .to_string();

        let src_ip = values
            .get_field(tunnel_fields::SRC_IP)
            .map(|s| s.to_string());

        // Only handle IPINIP tunnels
        if tunnel_type != TUNNEL_TYPE_IPINIP {
            info!(
                "Skipping non-IPINIP tunnel {} (type: {})",
                tunnel_name, tunnel_type
            );
            return Ok(true);
        }

        let mut tunnel_info = TunnelInfo::new(tunnel_type, dst_ip.clone()).with_src_ip(src_ip);

        // Set remote IP from peer if available
        if let Some(peer_ip) = &self.peer_ip {
            tunnel_info = tunnel_info.with_remote_ip(peer_ip.clone());

            // Configure Linux tunnel interface
            if !self.config_ip_tunnel(&tunnel_info).await? {
                return Ok(false); // Retry
            }
        } else {
            warn!("Peer/Remote IP not configured for tunnel {}", tunnel_name);
        }

        // Write to APPL_DB (skip if in warm restart replay)
        if !self.tunnel_replay.contains(tunnel_name) {
            self.write_tunnel_to_appl_db(tunnel_name, values, &tunnel_info)
                .await?;
        }

        // Update cache and remove from replay list
        self.tunnel_cache
            .insert(tunnel_name.to_string(), tunnel_info);
        self.tunnel_replay.remove(tunnel_name);

        info!("Tunnel {} configured", tunnel_name);
        Ok(true)
    }

    async fn do_tunnel_del(&mut self, tunnel_name: &str) -> CfgMgrResult<bool> {
        let tunnel_info = self
            .tunnel_cache
            .get(tunnel_name)
            .ok_or_else(|| CfgMgrError::invalid_config(tunnel_name, "Tunnel not found"))?
            .clone();

        if tunnel_info.tunnel_type == TUNNEL_TYPE_IPINIP {
            // Delete from APPL_DB
            self.delete_tunnel_from_appl_db(tunnel_name, &tunnel_info.dst_ip)
                .await?;
        }

        self.tunnel_cache.remove(tunnel_name);
        info!("Tunnel {} deleted", tunnel_name);
        Ok(true)
    }

    /// Write tunnel to APPL_DB
    async fn write_tunnel_to_appl_db(
        &mut self,
        tunnel_name: &str,
        values: &FieldValues,
        tunnel_info: &TunnelInfo,
    ) -> CfgMgrResult<()> {
        // TODO: Use ProducerStateTable to write to APP_TUNNEL_DECAP_TABLE
        // Filter out dst_ip field (only include tunnel_type, src_ip)
        let _filtered_values: Vec<_> = values
            .iter()
            .filter(|(k, _)| k != tunnel_fields::DST_IP)
            .collect();

        // Write decap term entry
        let _term_key = format!("{}:{}", tunnel_name, tunnel_info.dst_ip);

        // TODO: Use ProducerStateTable to write to APP_TUNNEL_DECAP_TERM_TABLE
        // with P2P/P2MP term_type based on src_ip presence
        let term_type = if tunnel_info.is_p2p() {
            decap_term_fields::TERM_TYPE_P2P
        } else {
            decap_term_fields::TERM_TYPE_P2MP
        };

        info!(
            "Would write tunnel {} to APPL_DB (term_type: {})",
            tunnel_name, term_type
        );
        Ok(())
    }

    /// Delete tunnel from APPL_DB
    async fn delete_tunnel_from_appl_db(
        &mut self,
        tunnel_name: &str,
        dst_ip: &str,
    ) -> CfgMgrResult<()> {
        let _term_key = format!("{}:{}", tunnel_name, dst_ip);
        // TODO: Use ProducerStateTable to delete from both tables
        info!("Would delete tunnel {} from APPL_DB", tunnel_name);
        Ok(())
    }

    /// Configure Linux IP-in-IP tunnel interface
    async fn config_ip_tunnel(&mut self, info: &TunnelInfo) -> CfgMgrResult<bool> {
        // Create tunnel device
        let cmd = build_add_tunnel_cmd(info);
        if let Err(e) = self.exec(&cmd).await {
            warn!(
                "Failed to create tunnel (dst: {}, remote: {}): {}",
                info.dst_ip, info.remote_ip, e
            );
            // Continue anyway - may already exist
        }

        // Bring tunnel interface up
        let cmd = build_set_tunnel_up_cmd();
        if let Err(e) = self.exec(&cmd).await {
            warn!(
                "Failed to bring up tunnel (dst: {}, remote: {}): {}",
                info.dst_ip, info.remote_ip, e
            );
        }

        // Assign loopback IP to tunnel if available
        if let Some(lpbk_ip) = self.intf_cache.get(LOOPBACK_SRC).cloned() {
            let cmd = build_add_tunnel_address_cmd(&lpbk_ip.to_string());
            if let Err(e) = self.exec(&cmd).await {
                warn!("Failed to assign IP {} to tunnel: {}", lpbk_ip, e);
            }
        }

        Ok(true)
    }

    /// Handle LOOPBACK_INTERFACE table updates
    pub async fn do_loopback_intf_task(
        &mut self,
        key: &str,
        _values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        // Key format: "Loopback3|10.0.0.1/32"
        let parts: Vec<&str> = key.split('|').collect();

        // Skip entries with just interface name
        if parts.len() == 1 {
            return Ok(true);
        }

        let alias = parts[0];
        let ip_prefix: IpPrefix = parts[1].parse().map_err(|_| {
            CfgMgrError::invalid_config("ip_prefix", format!("Invalid IP prefix: {}", parts[1]))
        })?;

        self.intf_cache.insert(alias.to_string(), ip_prefix.clone());

        // If this is Loopback3 and we have an active tunnel, assign the IP
        if alias == LOOPBACK_SRC && !self.tunnel_cache.is_empty() {
            let cmd = build_add_tunnel_address_cmd(&ip_prefix.to_string());
            if let Err(e) = self.exec(&cmd).await {
                warn!("Failed to assign IP {} to tunnel: {}", ip_prefix, e);
            }
        }

        info!("Loopback interface {} saved: {}", alias, ip_prefix);
        Ok(true)
    }

    /// Handle APP_TUNNEL_ROUTE_TABLE updates (from orchagent)
    pub async fn do_tunnel_route_task(
        &mut self,
        prefix_str: &str,
        op: &str,
        _values: &FieldValues,
    ) -> CfgMgrResult<bool> {
        let prefix: IpPrefix = prefix_str.parse().map_err(|_| {
            CfgMgrError::invalid_config("prefix", format!("Invalid IP prefix: {}", prefix_str))
        })?;

        if op == "SET" {
            let cmd = build_add_tunnel_route_cmd(&prefix);
            if let Err(e) = self.exec(&cmd).await {
                warn!("Failed to add route {}: {}", prefix, e);
            } else {
                info!("Route {} added through tunnel", prefix);
            }
        } else if op == "DEL" {
            let cmd = build_del_tunnel_route_cmd(&prefix);
            if let Err(e) = self.exec(&cmd).await {
                warn!("Failed to delete route {}: {}", prefix, e);
            } else {
                info!("Route {} deleted from tunnel", prefix);
            }
        }

        Ok(true)
    }

    /// Finalize warm restart
    fn finalize_warm_restart(&mut self) {
        self.replay_done = true;
        info!("Warm restart replay complete");
        // TODO: Set warm restart state to REPLAYED and RECONCILED
    }

    /// Check if warm restart replay is complete
    pub fn check_replay_done(&mut self) {
        if !self.replay_done && self.tunnel_replay.is_empty() {
            self.finalize_warm_restart();
        }
    }

    #[cfg(test)]
    pub fn get_captured_commands(&self) -> &[String] {
        &self.captured_commands
    }
}

impl Default for TunnelMgr {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Orch for TunnelMgr {
    fn name(&self) -> &str {
        "tunnelmgr"
    }

    async fn do_task(&mut self) {
        // TODO: Process consumers
        // This will be implemented when integrating with ConsumerStateTable
    }
}

#[async_trait]
impl CfgMgr for TunnelMgr {
    fn daemon_name(&self) -> &str {
        "tunnelmgrd"
    }

    fn is_warm_restart(&self) -> bool {
        !self.tunnel_replay.is_empty()
    }

    fn warm_restart_state(&self) -> WarmRestartState {
        if self.replay_done {
            WarmRestartState::Reconciled
        } else if !self.tunnel_replay.is_empty() {
            WarmRestartState::Replayed
        } else {
            WarmRestartState::Disabled
        }
    }

    async fn set_warm_restart_state(&mut self, _state: WarmRestartState) {
        // TODO: Write to STATE_DB WARM_RESTART_TABLE
    }

    fn config_table_names(&self) -> &[&str] {
        &[CFG_TUNNEL_TABLE, CFG_LOOPBACK_INTERFACE_TABLE]
    }

    fn is_replay_done(&self) -> bool {
        self.replay_done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tunnel_fields(dst_ip: &str, tunnel_type: &str, src_ip: Option<&str>) -> FieldValues {
        let mut fvs = vec![
            ("dst_ip".to_string(), dst_ip.to_string()),
            ("tunnel_type".to_string(), tunnel_type.to_string()),
        ];
        if let Some(src) = src_ip {
            fvs.push(("src_ip".to_string(), src.to_string()));
        }
        fvs
    }

    #[tokio::test]
    async fn test_tunnel_add_p2mp() {
        let mut mgr = TunnelMgr::new_mock().with_peer_ip("10.1.0.33".to_string());

        let fvs = make_tunnel_fields("10.1.0.32", "IPINIP", None);
        let result = mgr.do_tunnel_add("MuxTunnel0", &fvs).await.unwrap();

        assert!(result);
        assert!(mgr.tunnel_cache.contains_key("MuxTunnel0"));

        let info = mgr.tunnel_cache.get("MuxTunnel0").unwrap();
        assert_eq!(info.dst_ip, "10.1.0.32");
        assert_eq!(info.remote_ip, "10.1.0.33");
        assert!(!info.is_p2p());

        // Check commands
        let cmds = mgr.get_captured_commands();
        assert!(cmds.iter().any(|c| c.contains("ip tunnel add")));
        assert!(cmds.iter().any(|c| c.contains("ip link set dev tun0 up")));
    }

    #[tokio::test]
    async fn test_tunnel_add_p2p() {
        let mut mgr = TunnelMgr::new_mock().with_peer_ip("10.1.0.33".to_string());

        let fvs = make_tunnel_fields("10.1.0.32", "IPINIP", Some("10.0.0.1"));
        let result = mgr.do_tunnel_add("MuxTunnel0", &fvs).await.unwrap();

        assert!(result);
        let info = mgr.tunnel_cache.get("MuxTunnel0").unwrap();
        assert!(info.is_p2p());
        assert_eq!(info.src_ip, Some("10.0.0.1".to_string()));
    }

    #[tokio::test]
    async fn test_tunnel_del() {
        let mut mgr = TunnelMgr::new_mock();

        // Add tunnel first
        let info = TunnelInfo::new("IPINIP".to_string(), "10.1.0.32".to_string());
        mgr.tunnel_cache.insert("MuxTunnel0".to_string(), info);

        let result = mgr.do_tunnel_del("MuxTunnel0").await.unwrap();
        assert!(result);
        assert!(!mgr.tunnel_cache.contains_key("MuxTunnel0"));
    }

    #[tokio::test]
    async fn test_loopback_intf_add() {
        let mut mgr = TunnelMgr::new_mock();

        let result = mgr
            .do_loopback_intf_task("Loopback3|10.0.0.1/32", &vec![])
            .await
            .unwrap();

        assert!(result);
        assert!(mgr.intf_cache.contains_key("Loopback3"));

        let ip = mgr.intf_cache.get("Loopback3").unwrap();
        assert_eq!(ip.to_string(), "10.0.0.1/32");
    }

    #[tokio::test]
    async fn test_loopback_intf_skip_interface_only() {
        let mut mgr = TunnelMgr::new_mock();

        let result = mgr
            .do_loopback_intf_task("Loopback3", &vec![])
            .await
            .unwrap();

        assert!(result);
        assert!(!mgr.intf_cache.contains_key("Loopback3"));
    }

    #[tokio::test]
    async fn test_tunnel_route_add() {
        let mut mgr = TunnelMgr::new_mock();

        let result = mgr
            .do_tunnel_route_task("192.168.1.0/24", "SET", &vec![])
            .await
            .unwrap();

        assert!(result);
        let cmds = mgr.get_captured_commands();
        assert!(cmds.iter().any(|c| c.contains("ip route replace")));
        assert!(cmds
            .iter()
            .any(|c| c.contains("\"192.168.1.0/24\"") || c.contains("192.168.1.0/24")));
    }

    #[tokio::test]
    async fn test_tunnel_route_del() {
        let mut mgr = TunnelMgr::new_mock();

        let result = mgr
            .do_tunnel_route_task("192.168.1.0/24", "DEL", &vec![])
            .await
            .unwrap();

        assert!(result);
        let cmds = mgr.get_captured_commands();
        assert!(cmds.iter().any(|c| c.contains("ip route del")));
    }

    #[tokio::test]
    async fn test_tunnel_route_ipv6() {
        let mut mgr = TunnelMgr::new_mock();

        let result = mgr
            .do_tunnel_route_task("2001:db8::/32", "SET", &vec![])
            .await
            .unwrap();

        assert!(result);
        let cmds = mgr.get_captured_commands();
        assert!(cmds.iter().any(|c| c.contains("ip -6 route replace")));
    }

    #[tokio::test]
    async fn test_warm_restart_state() {
        let mut mgr = TunnelMgr::new();

        // Initially disabled
        assert_eq!(mgr.warm_restart_state(), WarmRestartState::Disabled);

        // With replay items
        mgr.tunnel_replay.insert("MuxTunnel0".to_string());
        assert_eq!(mgr.warm_restart_state(), WarmRestartState::Replayed);

        // After finalization
        mgr.finalize_warm_restart();
        assert_eq!(mgr.warm_restart_state(), WarmRestartState::Reconciled);
    }

    #[tokio::test]
    async fn test_check_replay_done() {
        let mut mgr = TunnelMgr::new();

        mgr.tunnel_replay.insert("MuxTunnel0".to_string());
        assert!(!mgr.replay_done);

        // Clear replay list
        mgr.tunnel_replay.clear();
        mgr.check_replay_done();

        assert!(mgr.replay_done);
    }
}
