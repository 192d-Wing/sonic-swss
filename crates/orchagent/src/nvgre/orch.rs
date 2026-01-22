//! NVGRE tunnel orchestration logic.

use super::types::{MapType, NvgreTunnelConfig, NvgreTunnelMapConfig, NvgreTunnelMapEntry, TunnelSaiIds, NVGRE_VSID_MAX_VALUE};
use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpAddress;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum NvgreOrchError {
    TunnelNotFound(String),
    TunnelExists(String),
    MapEntryNotFound(String),
    MapEntryExists(String),
    VlanNotFound(u16),
    InvalidVsid(u32),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct NvgreOrchConfig {
    pub enable_encap: bool,
    pub enable_decap: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NvgreOrchStats {
    pub tunnels_created: u64,
    pub tunnels_removed: u64,
    pub map_entries_created: u64,
    pub map_entries_removed: u64,
}

pub trait NvgreOrchCallbacks: Send + Sync {
    fn create_tunnel_map(&self, map_type: MapType, is_encap: bool) -> Result<RawSaiObjectId, String>;
    fn remove_tunnel_map(&self, oid: RawSaiObjectId) -> Result<(), String>;
    fn create_tunnel(&self, src_ip: &IpAddress, tunnel_ids: &TunnelSaiIds, underlay_rif: RawSaiObjectId) -> Result<RawSaiObjectId, String>;
    fn remove_tunnel(&self, oid: RawSaiObjectId) -> Result<(), String>;
    fn create_tunnel_termination(&self, tunnel_id: RawSaiObjectId, src_ip: &IpAddress, vr_id: RawSaiObjectId) -> Result<RawSaiObjectId, String>;
    fn remove_tunnel_termination(&self, oid: RawSaiObjectId) -> Result<(), String>;
    fn create_tunnel_map_entry(&self, map_type: MapType, vsid: u32, vlan_id: u16, is_encap: bool, encap_map_id: RawSaiObjectId, decap_map_id: RawSaiObjectId) -> Result<RawSaiObjectId, String>;
    fn remove_tunnel_map_entry(&self, oid: RawSaiObjectId) -> Result<(), String>;
    fn get_vlan_oid(&self, vlan_id: u16) -> Option<RawSaiObjectId>;
    fn get_underlay_rif(&self) -> RawSaiObjectId;
    fn get_virtual_router_id(&self) -> RawSaiObjectId;
}

pub struct NvgreTunnel {
    pub name: String,
    pub src_ip: IpAddress,
    pub tunnel_ids: TunnelSaiIds,
    pub map_entries: HashMap<String, NvgreTunnelMapEntry>,
}

impl NvgreTunnel {
    pub fn new(name: String, src_ip: IpAddress) -> Self {
        Self {
            name,
            src_ip,
            tunnel_ids: TunnelSaiIds::new(),
            map_entries: HashMap::new(),
        }
    }

    pub fn has_map_entry(&self, name: &str) -> bool {
        self.map_entries.contains_key(name)
    }

    pub fn add_map_entry(&mut self, name: String, entry: NvgreTunnelMapEntry) {
        self.map_entries.insert(name, entry);
    }

    pub fn remove_map_entry(&mut self, name: &str) -> Option<NvgreTunnelMapEntry> {
        self.map_entries.remove(name)
    }
}

pub struct NvgreOrch {
    config: NvgreOrchConfig,
    stats: NvgreOrchStats,
    callbacks: Option<Arc<dyn NvgreOrchCallbacks>>,
    tunnels: HashMap<String, NvgreTunnel>,
}

impl NvgreOrch {
    pub fn new(config: NvgreOrchConfig) -> Self {
        Self {
            config,
            stats: NvgreOrchStats::default(),
            callbacks: None,
            tunnels: HashMap::new(),
        }
    }

    pub fn set_callbacks(&mut self, callbacks: Arc<dyn NvgreOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    pub fn tunnel_exists(&self, name: &str) -> bool {
        self.tunnels.contains_key(name)
    }

    pub fn get_tunnel(&self, name: &str) -> Option<&NvgreTunnel> {
        self.tunnels.get(name)
    }

    pub fn get_tunnel_mut(&mut self, name: &str) -> Option<&mut NvgreTunnel> {
        self.tunnels.get_mut(name)
    }

    pub fn create_tunnel(&mut self, config: NvgreTunnelConfig) -> Result<(), NvgreOrchError> {
        if self.tunnels.contains_key(&config.name) {
            return Err(NvgreOrchError::TunnelExists(config.name.clone()));
        }

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| NvgreOrchError::SaiError("No callbacks set".to_string()))?;

        let mut tunnel = NvgreTunnel::new(config.name.clone(), config.src_ip.clone());

        // Create mappers for VLAN and BRIDGE
        for map_type in &[MapType::Vlan, MapType::Bridge] {
            let encap_id = callbacks.create_tunnel_map(*map_type, true)
                .map_err(NvgreOrchError::SaiError)?;
            let decap_id = callbacks.create_tunnel_map(*map_type, false)
                .map_err(NvgreOrchError::SaiError)?;

            tunnel.tunnel_ids.tunnel_encap_id.insert(*map_type, encap_id);
            tunnel.tunnel_ids.tunnel_decap_id.insert(*map_type, decap_id);
        }

        // Create tunnel
        let underlay_rif = callbacks.get_underlay_rif();
        tunnel.tunnel_ids.tunnel_id = callbacks.create_tunnel(&config.src_ip, &tunnel.tunnel_ids, underlay_rif)
            .map_err(NvgreOrchError::SaiError)?;

        // Create termination
        let vr_id = callbacks.get_virtual_router_id();
        tunnel.tunnel_ids.tunnel_term_id = callbacks.create_tunnel_termination(tunnel.tunnel_ids.tunnel_id, &config.src_ip, vr_id)
            .map_err(NvgreOrchError::SaiError)?;

        self.tunnels.insert(config.name.clone(), tunnel);
        self.stats.tunnels_created += 1;

        Ok(())
    }

    pub fn remove_tunnel(&mut self, name: &str) -> Result<(), NvgreOrchError> {
        let tunnel = self.tunnels.remove(name)
            .ok_or_else(|| NvgreOrchError::TunnelNotFound(name.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| NvgreOrchError::SaiError("No callbacks set".to_string()))?;

        // Remove all map entries first
        for (_, entry) in tunnel.map_entries {
            let _ = callbacks.remove_tunnel_map_entry(entry.map_entry_id);
        }

        // Remove termination
        let _ = callbacks.remove_tunnel_termination(tunnel.tunnel_ids.tunnel_term_id);

        // Remove tunnel
        let _ = callbacks.remove_tunnel(tunnel.tunnel_ids.tunnel_id);

        // Remove mappers
        for (_, oid) in tunnel.tunnel_ids.tunnel_encap_id {
            let _ = callbacks.remove_tunnel_map(oid);
        }
        for (_, oid) in tunnel.tunnel_ids.tunnel_decap_id {
            let _ = callbacks.remove_tunnel_map(oid);
        }

        self.stats.tunnels_removed += 1;

        Ok(())
    }

    pub fn add_tunnel_map(&mut self, config: NvgreTunnelMapConfig) -> Result<(), NvgreOrchError> {
        config.validate_vsid().map_err(|e| NvgreOrchError::InvalidVsid(config.vsid))?;

        let tunnel = self.tunnels.get_mut(&config.tunnel_name)
            .ok_or_else(|| NvgreOrchError::TunnelNotFound(config.tunnel_name.clone()))?;

        if tunnel.has_map_entry(&config.map_entry_name) {
            return Err(NvgreOrchError::MapEntryExists(config.map_entry_name.clone()));
        }

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| NvgreOrchError::SaiError("No callbacks set".to_string()))?;

        // Validate VLAN exists
        callbacks.get_vlan_oid(config.vlan_id)
            .ok_or(NvgreOrchError::VlanNotFound(config.vlan_id))?;

        let encap_map_id = *tunnel.tunnel_ids.tunnel_encap_id.get(&MapType::Vlan)
            .ok_or_else(|| NvgreOrchError::SaiError("No encap map".to_string()))?;
        let decap_map_id = *tunnel.tunnel_ids.tunnel_decap_id.get(&MapType::Vlan)
            .ok_or_else(|| NvgreOrchError::SaiError("No decap map".to_string()))?;

        let map_entry_id = callbacks.create_tunnel_map_entry(
            MapType::Vlan,
            config.vsid,
            config.vlan_id,
            false,
            encap_map_id,
            decap_map_id,
        ).map_err(NvgreOrchError::SaiError)?;

        let entry = NvgreTunnelMapEntry::new(map_entry_id, config.vlan_id, config.vsid);
        tunnel.add_map_entry(config.map_entry_name.clone(), entry);

        self.stats.map_entries_created += 1;

        Ok(())
    }

    pub fn remove_tunnel_map(&mut self, tunnel_name: &str, map_entry_name: &str) -> Result<(), NvgreOrchError> {
        let tunnel = self.tunnels.get_mut(tunnel_name)
            .ok_or_else(|| NvgreOrchError::TunnelNotFound(tunnel_name.to_string()))?;

        let entry = tunnel.remove_map_entry(map_entry_name)
            .ok_or_else(|| NvgreOrchError::MapEntryNotFound(map_entry_name.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| NvgreOrchError::SaiError("No callbacks set".to_string()))?;

        callbacks.remove_tunnel_map_entry(entry.map_entry_id)
            .map_err(NvgreOrchError::SaiError)?;

        self.stats.map_entries_removed += 1;

        Ok(())
    }

    pub fn stats(&self) -> &NvgreOrchStats {
        &self.stats
    }

    pub fn tunnel_count(&self) -> usize {
        self.tunnels.len()
    }
}
