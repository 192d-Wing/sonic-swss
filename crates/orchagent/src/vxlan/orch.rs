//! VXLAN orchestration logic.

use super::types::{VxlanStats, VxlanTunnelEntry, VxlanTunnelKey, VxlanVlanMapEntry, VxlanVrfMapEntry};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum VxlanOrchError {
    TunnelNotFound(VxlanTunnelKey),
    VrfMapNotFound(u32, String),
    VlanMapNotFound(u32, u16),
    InvalidVni(u32),
    InvalidIp(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct VxlanOrchConfig {
    pub evpn_nvo_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct VxlanOrchStats {
    pub stats: VxlanStats,
    pub errors: u64,
}

pub trait VxlanOrchCallbacks: Send + Sync {
    fn on_tunnel_created(&self, entry: &VxlanTunnelEntry);
    fn on_tunnel_removed(&self, key: &VxlanTunnelKey);
    fn on_vrf_map_created(&self, entry: &VxlanVrfMapEntry);
    fn on_vrf_map_removed(&self, vni: u32, vrf_name: &str);
    fn on_vlan_map_created(&self, entry: &VxlanVlanMapEntry);
    fn on_vlan_map_removed(&self, vni: u32, vlan_id: u16);
}

pub struct VxlanOrch {
    config: VxlanOrchConfig,
    stats: VxlanOrchStats,
    tunnels: HashMap<VxlanTunnelKey, VxlanTunnelEntry>,
}

impl VxlanOrch {
    pub fn new(config: VxlanOrchConfig) -> Self {
        Self {
            config,
            stats: VxlanOrchStats::default(),
            tunnels: HashMap::new(),
        }
    }

    pub fn get_tunnel(&self, key: &VxlanTunnelKey) -> Option<&VxlanTunnelEntry> {
        self.tunnels.get(key)
    }

    pub fn stats(&self) -> &VxlanOrchStats {
        &self.stats
    }
}
