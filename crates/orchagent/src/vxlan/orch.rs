//! VXLAN orchestration logic.

use super::types::{VxlanStats, VxlanTunnelEntry, VxlanTunnelKey, VxlanVlanMapEntry, VxlanVlanMapKey, VxlanVrfMapEntry, VxlanVrfMapKey};
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
    vrf_maps: HashMap<VxlanVrfMapKey, VxlanVrfMapEntry>,
    vlan_maps: HashMap<VxlanVlanMapKey, VxlanVlanMapEntry>,
}

impl VxlanOrch {
    pub fn new(config: VxlanOrchConfig) -> Self {
        Self {
            config,
            stats: VxlanOrchStats::default(),
            tunnels: HashMap::new(),
            vrf_maps: HashMap::new(),
            vlan_maps: HashMap::new(),
        }
    }

    pub fn get_tunnel(&self, key: &VxlanTunnelKey) -> Option<&VxlanTunnelEntry> {
        self.tunnels.get(key)
    }

    pub fn add_tunnel(&mut self, entry: VxlanTunnelEntry) -> Result<(), VxlanOrchError> {
        let key = entry.key.clone();

        if self.tunnels.contains_key(&key) {
            return Err(VxlanOrchError::SaiError("Tunnel already exists".to_string()));
        }

        self.stats.stats.tunnels_created = self.stats.stats.tunnels_created.saturating_add(1);
        self.tunnels.insert(key, entry);

        Ok(())
    }

    pub fn remove_tunnel(&mut self, key: &VxlanTunnelKey) -> Result<VxlanTunnelEntry, VxlanOrchError> {
        self.tunnels.remove(key)
            .ok_or_else(|| VxlanOrchError::TunnelNotFound(key.clone()))
    }

    pub fn add_vrf_map(&mut self, entry: VxlanVrfMapEntry) -> Result<(), VxlanOrchError> {
        let key = entry.key.clone();

        if self.vrf_maps.contains_key(&key) {
            return Err(VxlanOrchError::SaiError("VRF map already exists".to_string()));
        }

        self.stats.stats.vrf_maps_created = self.stats.stats.vrf_maps_created.saturating_add(1);
        self.vrf_maps.insert(key, entry);

        Ok(())
    }

    pub fn remove_vrf_map(&mut self, vni: u32, vrf_name: &str) -> Result<VxlanVrfMapEntry, VxlanOrchError> {
        let key = VxlanVrfMapKey::new(vni, vrf_name.to_string());
        self.vrf_maps.remove(&key)
            .ok_or_else(|| VxlanOrchError::VrfMapNotFound(vni, vrf_name.to_string()))
    }

    pub fn get_vrf_map(&self, vni: u32, vrf_name: &str) -> Option<&VxlanVrfMapEntry> {
        let key = VxlanVrfMapKey::new(vni, vrf_name.to_string());
        self.vrf_maps.get(&key)
    }

    pub fn add_vlan_map(&mut self, entry: VxlanVlanMapEntry) -> Result<(), VxlanOrchError> {
        let key = entry.key.clone();

        if self.vlan_maps.contains_key(&key) {
            return Err(VxlanOrchError::SaiError("VLAN map already exists".to_string()));
        }

        self.stats.stats.vlan_maps_created = self.stats.stats.vlan_maps_created.saturating_add(1);
        self.vlan_maps.insert(key, entry);

        Ok(())
    }

    pub fn remove_vlan_map(&mut self, vni: u32, vlan_id: u16) -> Result<VxlanVlanMapEntry, VxlanOrchError> {
        let key = VxlanVlanMapKey::new(vni, vlan_id);
        self.vlan_maps.remove(&key)
            .ok_or_else(|| VxlanOrchError::VlanMapNotFound(vni, vlan_id))
    }

    pub fn get_vlan_map(&self, vni: u32, vlan_id: u16) -> Option<&VxlanVlanMapEntry> {
        let key = VxlanVlanMapKey::new(vni, vlan_id);
        self.vlan_maps.get(&key)
    }

    pub fn get_maps_by_vni(&self, vni: u32) -> (Vec<&VxlanVrfMapEntry>, Vec<&VxlanVlanMapEntry>) {
        let vrf_maps: Vec<_> = self.vrf_maps
            .values()
            .filter(|entry| entry.key.vni == vni)
            .collect();

        let vlan_maps: Vec<_> = self.vlan_maps
            .values()
            .filter(|entry| entry.key.vni == vni)
            .collect();

        (vrf_maps, vlan_maps)
    }

    pub fn tunnel_count(&self) -> usize {
        self.tunnels.len()
    }

    pub fn stats(&self) -> &VxlanOrchStats {
        &self.stats
    }
}
