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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::sync::Mutex;

    /// Mock callbacks for testing
    struct MockCallbacks {
        next_oid: Mutex<RawSaiObjectId>,
        fail_create_tunnel: Mutex<bool>,
        fail_create_map_entry: Mutex<bool>,
        vlan_exists: Mutex<HashMap<u16, RawSaiObjectId>>,
    }

    impl MockCallbacks {
        fn new() -> Self {
            let mut vlan_exists = HashMap::new();
            // Pre-populate some VLANs
            vlan_exists.insert(100, 0x10000);
            vlan_exists.insert(200, 0x20000);
            vlan_exists.insert(300, 0x30000);

            Self {
                next_oid: Mutex::new(0x1000),
                fail_create_tunnel: Mutex::new(false),
                fail_create_map_entry: Mutex::new(false),
                vlan_exists: Mutex::new(vlan_exists),
            }
        }

        fn next_oid(&self) -> RawSaiObjectId {
            let mut oid = self.next_oid.lock().unwrap();
            let current = *oid;
            *oid += 1;
            current
        }

        fn set_fail_create_tunnel(&self, fail: bool) {
            *self.fail_create_tunnel.lock().unwrap() = fail;
        }

        fn set_fail_create_map_entry(&self, fail: bool) {
            *self.fail_create_map_entry.lock().unwrap() = fail;
        }

        fn add_vlan(&self, vlan_id: u16, oid: RawSaiObjectId) {
            self.vlan_exists.lock().unwrap().insert(vlan_id, oid);
        }
    }

    impl NvgreOrchCallbacks for MockCallbacks {
        fn create_tunnel_map(&self, _map_type: MapType, _is_encap: bool) -> Result<RawSaiObjectId, String> {
            Ok(self.next_oid())
        }

        fn remove_tunnel_map(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn create_tunnel(&self, _src_ip: &IpAddress, _tunnel_ids: &TunnelSaiIds, _underlay_rif: RawSaiObjectId) -> Result<RawSaiObjectId, String> {
            if *self.fail_create_tunnel.lock().unwrap() {
                return Err("Mock tunnel creation failure".to_string());
            }
            Ok(self.next_oid())
        }

        fn remove_tunnel(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn create_tunnel_termination(&self, _tunnel_id: RawSaiObjectId, _src_ip: &IpAddress, _vr_id: RawSaiObjectId) -> Result<RawSaiObjectId, String> {
            Ok(self.next_oid())
        }

        fn remove_tunnel_termination(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn create_tunnel_map_entry(&self, _map_type: MapType, _vsid: u32, _vlan_id: u16, _is_encap: bool, _encap_map_id: RawSaiObjectId, _decap_map_id: RawSaiObjectId) -> Result<RawSaiObjectId, String> {
            if *self.fail_create_map_entry.lock().unwrap() {
                return Err("Mock map entry creation failure".to_string());
            }
            Ok(self.next_oid())
        }

        fn remove_tunnel_map_entry(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn get_vlan_oid(&self, vlan_id: u16) -> Option<RawSaiObjectId> {
            self.vlan_exists.lock().unwrap().get(&vlan_id).copied()
        }

        fn get_underlay_rif(&self) -> RawSaiObjectId {
            0xFFFF
        }

        fn get_virtual_router_id(&self) -> RawSaiObjectId {
            0xFFFE
        }
    }

    fn create_test_orch() -> NvgreOrch {
        let config = NvgreOrchConfig {
            enable_encap: true,
            enable_decap: true,
        };
        let mut orch = NvgreOrch::new(config);
        let callbacks = Arc::new(MockCallbacks::new());
        orch.set_callbacks(callbacks);
        orch
    }

    fn test_ip_v4(a: u8, b: u8, c: u8, d: u8) -> IpAddress {
        IpAddress::V4(Ipv4Addr::new(a, b, c, d).into())
    }

    // ========== NVGRE Tunnel Management Tests ==========

    #[test]
    fn test_create_tunnel_success() {
        let mut orch = create_test_orch();
        let config = NvgreTunnelConfig::new(
            "tunnel1".to_string(),
            test_ip_v4(10, 0, 0, 1),
        );

        let result = orch.create_tunnel(config);
        assert!(result.is_ok());
        assert!(orch.tunnel_exists("tunnel1"));
        assert_eq!(orch.tunnel_count(), 1);
        assert_eq!(orch.stats().tunnels_created, 1);

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.name, "tunnel1");
        assert_eq!(tunnel.src_ip, test_ip_v4(10, 0, 0, 1));
        assert!(tunnel.tunnel_ids.tunnel_id != 0);
        assert!(tunnel.tunnel_ids.tunnel_term_id != 0);
        assert!(tunnel.tunnel_ids.tunnel_encap_id.contains_key(&MapType::Vlan));
        assert!(tunnel.tunnel_ids.tunnel_encap_id.contains_key(&MapType::Bridge));
    }

    #[test]
    fn test_create_tunnel_duplicate() {
        let mut orch = create_test_orch();
        let config = NvgreTunnelConfig::new(
            "tunnel1".to_string(),
            test_ip_v4(10, 0, 0, 1),
        );

        // First creation should succeed
        assert!(orch.create_tunnel(config.clone()).is_ok());

        // Second creation should fail
        let result = orch.create_tunnel(config);
        assert!(matches!(result, Err(NvgreOrchError::TunnelExists(_))));
    }

    #[test]
    fn test_create_multiple_tunnels() {
        let mut orch = create_test_orch();

        let config1 = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        let config2 = NvgreTunnelConfig::new("tunnel2".to_string(), test_ip_v4(10, 0, 0, 2));
        let config3 = NvgreTunnelConfig::new("tunnel3".to_string(), test_ip_v4(10, 0, 0, 3));

        assert!(orch.create_tunnel(config1).is_ok());
        assert!(orch.create_tunnel(config2).is_ok());
        assert!(orch.create_tunnel(config3).is_ok());

        assert_eq!(orch.tunnel_count(), 3);
        assert_eq!(orch.stats().tunnels_created, 3);
    }

    #[test]
    fn test_remove_tunnel_success() {
        let mut orch = create_test_orch();
        let config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));

        orch.create_tunnel(config).unwrap();
        assert!(orch.tunnel_exists("tunnel1"));

        let result = orch.remove_tunnel("tunnel1");
        assert!(result.is_ok());
        assert!(!orch.tunnel_exists("tunnel1"));
        assert_eq!(orch.tunnel_count(), 0);
        assert_eq!(orch.stats().tunnels_removed, 1);
    }

    #[test]
    fn test_remove_tunnel_not_found() {
        let mut orch = create_test_orch();
        let result = orch.remove_tunnel("nonexistent");
        assert!(matches!(result, Err(NvgreOrchError::TunnelNotFound(_))));
    }

    #[test]
    fn test_remove_tunnel_with_map_entries() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add map entries
        let map_config1 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        let map_config2 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map2".to_string(), 200, 2000);
        orch.add_tunnel_map(map_config1).unwrap();
        orch.add_tunnel_map(map_config2).unwrap();

        // Remove tunnel should clean up all map entries
        let result = orch.remove_tunnel("tunnel1");
        assert!(result.is_ok());
        assert!(!orch.tunnel_exists("tunnel1"));
        assert_eq!(orch.stats().tunnels_removed, 1);
    }

    #[test]
    fn test_tunnel_configuration_with_different_ips() {
        let mut orch = create_test_orch();

        // IPv4 addresses
        let config1 = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(192, 168, 1, 1));
        let config2 = NvgreTunnelConfig::new("tunnel2".to_string(), test_ip_v4(172, 16, 0, 1));

        assert!(orch.create_tunnel(config1).is_ok());
        assert!(orch.create_tunnel(config2).is_ok());

        assert_eq!(orch.get_tunnel("tunnel1").unwrap().src_ip, test_ip_v4(192, 168, 1, 1));
        assert_eq!(orch.get_tunnel("tunnel2").unwrap().src_ip, test_ip_v4(172, 16, 0, 1));
    }

    // ========== VSID Configuration Tests ==========

    #[test]
    fn test_add_tunnel_map_success() {
        let mut orch = create_test_orch();

        // Create tunnel first
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add map entry
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        let result = orch.add_tunnel_map(map_config);

        assert!(result.is_ok());
        assert_eq!(orch.stats().map_entries_created, 1);

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert!(tunnel.has_map_entry("map1"));
    }

    #[test]
    fn test_add_tunnel_map_tunnel_not_found() {
        let mut orch = create_test_orch();
        let map_config = NvgreTunnelMapConfig::new("nonexistent".to_string(), "map1".to_string(), 100, 1000);
        let result = orch.add_tunnel_map(map_config);
        assert!(matches!(result, Err(NvgreOrchError::TunnelNotFound(_))));
    }

    #[test]
    fn test_add_tunnel_map_duplicate() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add first map entry
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        orch.add_tunnel_map(map_config.clone()).unwrap();

        // Try to add duplicate
        let result = orch.add_tunnel_map(map_config);
        assert!(matches!(result, Err(NvgreOrchError::MapEntryExists(_))));
    }

    #[test]
    fn test_vsid_to_vlan_mapping() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add multiple VSID to VLAN mappings
        let map_config1 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        let map_config2 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map2".to_string(), 200, 2000);
        let map_config3 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map3".to_string(), 300, 3000);

        orch.add_tunnel_map(map_config1).unwrap();
        orch.add_tunnel_map(map_config2).unwrap();
        orch.add_tunnel_map(map_config3).unwrap();

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.map_entries.len(), 3);
        assert_eq!(tunnel.map_entries.get("map1").unwrap().vsid, 1000);
        assert_eq!(tunnel.map_entries.get("map1").unwrap().vlan_id, 100);
        assert_eq!(tunnel.map_entries.get("map2").unwrap().vsid, 2000);
        assert_eq!(tunnel.map_entries.get("map2").unwrap().vlan_id, 200);
    }

    #[test]
    fn test_vsid_range_validation_zero() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Try to add map with VSID = 0 (reserved)
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 0);
        let result = orch.add_tunnel_map(map_config);
        assert!(matches!(result, Err(NvgreOrchError::InvalidVsid(0))));
    }

    #[test]
    fn test_vsid_range_validation_max_exceeded() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Try to add map with VSID > max
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, NVGRE_VSID_MAX_VALUE + 1);
        let result = orch.add_tunnel_map(map_config);
        assert!(matches!(result, Err(NvgreOrchError::InvalidVsid(_))));
    }

    #[test]
    fn test_vsid_range_validation_max_valid() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add map with VSID at max value (should succeed)
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, NVGRE_VSID_MAX_VALUE);
        let result = orch.add_tunnel_map(map_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_vsid_range_validation_min_valid() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add map with VSID = 1 (minimum valid)
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1);
        let result = orch.add_tunnel_map(map_config);
        assert!(result.is_ok());
    }

    // ========== Tunnel Map Management Tests ==========

    #[test]
    fn test_remove_tunnel_map_success() {
        let mut orch = create_test_orch();

        // Create tunnel and add map
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        orch.add_tunnel_map(map_config).unwrap();

        // Remove map entry
        let result = orch.remove_tunnel_map("tunnel1", "map1");
        assert!(result.is_ok());
        assert_eq!(orch.stats().map_entries_removed, 1);

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert!(!tunnel.has_map_entry("map1"));
    }

    #[test]
    fn test_remove_tunnel_map_not_found() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Try to remove non-existent map
        let result = orch.remove_tunnel_map("tunnel1", "nonexistent");
        assert!(matches!(result, Err(NvgreOrchError::MapEntryNotFound(_))));
    }

    #[test]
    fn test_remove_tunnel_map_tunnel_not_found() {
        let mut orch = create_test_orch();
        let result = orch.remove_tunnel_map("nonexistent", "map1");
        assert!(matches!(result, Err(NvgreOrchError::TunnelNotFound(_))));
    }

    #[test]
    fn test_multiple_mappings_per_tunnel() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add 10 map entries (all using VLAN 100 which exists in mock, but different VSIDs)
        for i in 0..10 {
            let map_config = NvgreTunnelMapConfig::new(
                "tunnel1".to_string(),
                format!("map{}", i),
                100, // Use same VLAN for all
                1000 + i,
            );
            orch.add_tunnel_map(map_config).unwrap();
        }

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.map_entries.len(), 10);
        assert_eq!(orch.stats().map_entries_created, 10);

        // Remove half of them
        for i in 0..5 {
            orch.remove_tunnel_map("tunnel1", &format!("map{}", i)).unwrap();
        }

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.map_entries.len(), 5);
        assert_eq!(orch.stats().map_entries_removed, 5);
    }

    #[test]
    fn test_map_entry_creation_removal_lifecycle() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add and remove map entries in sequence
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        orch.add_tunnel_map(map_config.clone()).unwrap();
        orch.remove_tunnel_map("tunnel1", "map1").unwrap();

        // Should be able to add it again after removal
        let result = orch.add_tunnel_map(map_config);
        assert!(result.is_ok());

        assert_eq!(orch.stats().map_entries_created, 2);
        assert_eq!(orch.stats().map_entries_removed, 1);
    }

    // ========== VLAN Validation Tests ==========

    #[test]
    fn test_vlan_not_found() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Try to add map with non-existent VLAN
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 999, 1000);
        let result = orch.add_tunnel_map(map_config);
        assert!(matches!(result, Err(NvgreOrchError::VlanNotFound(999))));
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_create_tunnel_without_callbacks() {
        let config = NvgreOrchConfig {
            enable_encap: true,
            enable_decap: true,
        };
        let mut orch = NvgreOrch::new(config);

        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        let result = orch.create_tunnel(tunnel_config);
        assert!(matches!(result, Err(NvgreOrchError::SaiError(_))));
    }

    #[test]
    fn test_sai_tunnel_creation_failure() {
        let config = NvgreOrchConfig {
            enable_encap: true,
            enable_decap: true,
        };
        let mut orch = NvgreOrch::new(config);
        let mock_callbacks = Arc::new(MockCallbacks::new());
        mock_callbacks.set_fail_create_tunnel(true);
        orch.set_callbacks(mock_callbacks);

        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        let result = orch.create_tunnel(tunnel_config);
        assert!(matches!(result, Err(NvgreOrchError::SaiError(_))));

        // Tunnel should not be created
        assert!(!orch.tunnel_exists("tunnel1"));
        assert_eq!(orch.tunnel_count(), 0);
    }

    #[test]
    fn test_sai_map_entry_creation_failure() {
        let config = NvgreOrchConfig {
            enable_encap: true,
            enable_decap: true,
        };
        let mut orch = NvgreOrch::new(config);
        let mock_callbacks = Arc::new(MockCallbacks::new());
        mock_callbacks.set_fail_create_map_entry(true);
        orch.set_callbacks(mock_callbacks);

        // Create tunnel (should succeed)
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Try to add map entry (should fail)
        let map_config = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        let result = orch.add_tunnel_map(map_config);
        assert!(matches!(result, Err(NvgreOrchError::SaiError(_))));

        // Map entry should not be created
        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert!(!tunnel.has_map_entry("map1"));
    }

    // ========== Statistics Tests ==========

    #[test]
    fn test_statistics_tracking() {
        let mut orch = create_test_orch();

        // Initial stats
        assert_eq!(orch.stats().tunnels_created, 0);
        assert_eq!(orch.stats().tunnels_removed, 0);
        assert_eq!(orch.stats().map_entries_created, 0);
        assert_eq!(orch.stats().map_entries_removed, 0);

        // Create tunnels
        let config1 = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        let config2 = NvgreTunnelConfig::new("tunnel2".to_string(), test_ip_v4(10, 0, 0, 2));
        orch.create_tunnel(config1).unwrap();
        orch.create_tunnel(config2).unwrap();

        assert_eq!(orch.stats().tunnels_created, 2);

        // Add map entries
        let map_config1 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 100, 1000);
        let map_config2 = NvgreTunnelMapConfig::new("tunnel2".to_string(), "map2".to_string(), 200, 2000);
        orch.add_tunnel_map(map_config1).unwrap();
        orch.add_tunnel_map(map_config2).unwrap();

        assert_eq!(orch.stats().map_entries_created, 2);

        // Remove one map entry
        orch.remove_tunnel_map("tunnel1", "map1").unwrap();
        assert_eq!(orch.stats().map_entries_removed, 1);

        // Remove one tunnel
        orch.remove_tunnel("tunnel2").unwrap();
        assert_eq!(orch.stats().tunnels_removed, 1);

        // Final counts
        assert_eq!(orch.tunnel_count(), 1);
    }

    #[test]
    fn test_tunnel_count_accuracy() {
        let mut orch = create_test_orch();

        assert_eq!(orch.tunnel_count(), 0);

        for i in 0..5 {
            let config = NvgreTunnelConfig::new(format!("tunnel{}", i), test_ip_v4(10, 0, 0, i as u8 + 1));
            orch.create_tunnel(config).unwrap();
        }

        assert_eq!(orch.tunnel_count(), 5);

        for i in 0..3 {
            orch.remove_tunnel(&format!("tunnel{}", i)).unwrap();
        }

        assert_eq!(orch.tunnel_count(), 2);
    }

    #[test]
    fn test_map_entry_count_per_tunnel() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add varying number of map entries
        for i in 0..15 {
            let map_config = NvgreTunnelMapConfig::new(
                "tunnel1".to_string(),
                format!("map{}", i),
                100,
                1000 + i,
            );
            orch.add_tunnel_map(map_config).unwrap();
        }

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.map_entries.len(), 15);
        assert_eq!(orch.stats().map_entries_created, 15);
    }

    // ========== Edge Cases Tests ==========

    #[test]
    fn test_empty_tunnel_no_vsids() {
        let mut orch = create_test_orch();

        // Create tunnel without any map entries
        let config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(config).unwrap();

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.map_entries.len(), 0);

        // Should be able to remove empty tunnel
        let result = orch.remove_tunnel("tunnel1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_tunnel_with_maximum_vsids() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add many map entries (simulating maximum)
        for i in 0..100 {
            let map_config = NvgreTunnelMapConfig::new(
                "tunnel1".to_string(),
                format!("map{}", i),
                100,
                1000 + i,
            );
            orch.add_tunnel_map(map_config).unwrap();
        }

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.map_entries.len(), 100);

        // Should still be able to remove tunnel with many entries
        let result = orch.remove_tunnel("tunnel1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_tunnel_src_ip_different_ranges() {
        let mut orch = create_test_orch();

        // Test various IP ranges
        let test_cases = vec![
            (test_ip_v4(10, 0, 0, 1), "private_10"),
            (test_ip_v4(172, 16, 0, 1), "private_172"),
            (test_ip_v4(192, 168, 1, 1), "private_192"),
            (test_ip_v4(203, 0, 113, 1), "public"),
        ];

        for (ip, name) in test_cases {
            let config = NvgreTunnelConfig::new(format!("tunnel_{}", name), ip.clone());
            assert!(orch.create_tunnel(config).is_ok());

            let tunnel = orch.get_tunnel(&format!("tunnel_{}", name)).unwrap();
            assert_eq!(tunnel.src_ip, ip);
        }

        assert_eq!(orch.tunnel_count(), 4);
    }

    #[test]
    fn test_get_tunnel_mut() {
        let mut orch = create_test_orch();

        // Create tunnel
        let config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(config).unwrap();

        // Get mutable reference and modify
        let tunnel = orch.get_tunnel_mut("tunnel1").unwrap();
        tunnel.add_map_entry(
            "direct_map".to_string(),
            NvgreTunnelMapEntry::new(0x9999, 500, 5000),
        );

        // Verify modification
        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert!(tunnel.has_map_entry("direct_map"));
        assert_eq!(tunnel.map_entries.get("direct_map").unwrap().vsid, 5000);
    }

    #[test]
    fn test_get_tunnel_nonexistent() {
        let orch = create_test_orch();
        assert!(orch.get_tunnel("nonexistent").is_none());
        assert!(!orch.tunnel_exists("nonexistent"));
    }

    #[test]
    fn test_tunnel_names_case_sensitive() {
        let mut orch = create_test_orch();

        let config1 = NvgreTunnelConfig::new("Tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        let config2 = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 2));

        orch.create_tunnel(config1).unwrap();
        orch.create_tunnel(config2).unwrap();

        assert_eq!(orch.tunnel_count(), 2);
        assert!(orch.tunnel_exists("Tunnel1"));
        assert!(orch.tunnel_exists("tunnel1"));
    }

    #[test]
    fn test_map_entry_names_case_sensitive() {
        let mut orch = create_test_orch();

        // Create tunnel
        let tunnel_config = NvgreTunnelConfig::new("tunnel1".to_string(), test_ip_v4(10, 0, 0, 1));
        orch.create_tunnel(tunnel_config).unwrap();

        // Add map entries with different case
        let map_config1 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "Map1".to_string(), 100, 1000);
        let map_config2 = NvgreTunnelMapConfig::new("tunnel1".to_string(), "map1".to_string(), 200, 2000);

        orch.add_tunnel_map(map_config1).unwrap();
        orch.add_tunnel_map(map_config2).unwrap();

        let tunnel = orch.get_tunnel("tunnel1").unwrap();
        assert_eq!(tunnel.map_entries.len(), 2);
        assert!(tunnel.has_map_entry("Map1"));
        assert!(tunnel.has_map_entry("map1"));
    }

    #[test]
    fn test_config_defaults() {
        let config = NvgreOrchConfig::default();
        assert!(!config.enable_encap);
        assert!(!config.enable_decap);
    }

    #[test]
    fn test_stats_defaults() {
        let stats = NvgreOrchStats::default();
        assert_eq!(stats.tunnels_created, 0);
        assert_eq!(stats.tunnels_removed, 0);
        assert_eq!(stats.map_entries_created, 0);
        assert_eq!(stats.map_entries_removed, 0);
    }

    #[test]
    fn test_complex_scenario_multiple_tunnels_and_maps() {
        let mut orch = create_test_orch();

        // Create 3 tunnels
        for i in 1..=3 {
            let config = NvgreTunnelConfig::new(
                format!("tunnel{}", i),
                test_ip_v4(10, 0, 0, i as u8),
            );
            orch.create_tunnel(config).unwrap();
        }

        // Add 3 maps to each tunnel
        for i in 1..=3 {
            for j in 1..=3 {
                let map_config = NvgreTunnelMapConfig::new(
                    format!("tunnel{}", i),
                    format!("map{}_{}", i, j),
                    100,
                    (i * 1000 + j * 100) as u32,
                );
                orch.add_tunnel_map(map_config).unwrap();
            }
        }

        // Verify state
        assert_eq!(orch.tunnel_count(), 3);
        assert_eq!(orch.stats().map_entries_created, 9);

        for i in 1..=3 {
            let tunnel = orch.get_tunnel(&format!("tunnel{}", i)).unwrap();
            assert_eq!(tunnel.map_entries.len(), 3);
        }

        // Remove middle tunnel
        orch.remove_tunnel("tunnel2").unwrap();
        assert_eq!(orch.tunnel_count(), 2);

        // Verify remaining tunnels
        assert!(orch.tunnel_exists("tunnel1"));
        assert!(!orch.tunnel_exists("tunnel2"));
        assert!(orch.tunnel_exists("tunnel3"));
    }
}
