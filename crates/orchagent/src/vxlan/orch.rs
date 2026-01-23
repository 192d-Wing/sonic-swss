//! VXLAN orchestration logic.

use super::types::{VxlanStats, VxlanTunnelEntry, VxlanTunnelKey, VxlanVlanMapEntry, VxlanVlanMapKey, VxlanVrfMapEntry, VxlanVrfMapKey};
use std::collections::HashMap;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};

#[derive(Debug, Clone, thiserror::Error)]
pub enum VxlanOrchError {
    #[error("Tunnel not found: {0:?}")]
    TunnelNotFound(VxlanTunnelKey),
    #[error("VRF map not found: vni={0}, vrf={1}")]
    VrfMapNotFound(u32, String),
    #[error("VLAN map not found: vni={0}, vlan={1}")]
    VlanMapNotFound(u32, u16),
    #[error("Invalid VNI: {0}")]
    InvalidVni(u32),
    #[error("Invalid IP: {0}")]
    InvalidIp(String),
    #[error("SAI error: {0}")]
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
            let error = VxlanOrchError::SaiError("Tunnel already exists".to_string());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "VxlanOrch",
                "create_tunnel"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(entry.config.tunnel_name.clone())
            .with_object_type("vxlan_tunnel")
            .with_error(error.to_string()));
            return Err(error);
        }

        self.stats.stats.tunnels_created = self.stats.stats.tunnels_created.saturating_add(1);
        self.tunnels.insert(key, entry.clone());

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "VxlanOrch",
            "create_tunnel"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(entry.config.tunnel_name.clone())
        .with_object_type("vxlan_tunnel")
        .with_details(serde_json::json!({
            "tunnel_name": entry.config.tunnel_name,
            "src_ip": entry.config.src_ip.to_string(),
            "dst_ip": entry.config.dst_ip.to_string(),
            "tunnel_oid": entry.tunnel_oid,
            "stats": {
                "tunnels_created": self.stats.stats.tunnels_created
            }
        })));

        Ok(())
    }

    pub fn remove_tunnel(&mut self, key: &VxlanTunnelKey) -> Result<VxlanTunnelEntry, VxlanOrchError> {
        let entry = self.tunnels.remove(key)
            .ok_or_else(|| VxlanOrchError::TunnelNotFound(key.clone()))?;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "VxlanOrch",
            "remove_tunnel"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(entry.config.tunnel_name.clone())
        .with_object_type("vxlan_tunnel")
        .with_details(serde_json::json!({
            "tunnel_name": entry.config.tunnel_name,
            "src_ip": entry.config.src_ip.to_string(),
            "dst_ip": entry.config.dst_ip.to_string(),
            "tunnel_oid": entry.tunnel_oid
        })));

        Ok(entry)
    }

    pub fn add_vrf_map(&mut self, entry: VxlanVrfMapEntry) -> Result<(), VxlanOrchError> {
        let key = entry.key.clone();

        if self.vrf_maps.contains_key(&key) {
            let error = VxlanOrchError::SaiError("VRF map already exists".to_string());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "VxlanOrch",
                "add_vrf_vxlan_map"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(format!("vrf_map_{}_{}", key.vni, key.vrf_name))
            .with_object_type("vrf_vxlan_map")
            .with_error(error.to_string()));
            return Err(error);
        }

        self.stats.stats.vrf_maps_created = self.stats.stats.vrf_maps_created.saturating_add(1);
        self.vrf_maps.insert(key, entry.clone());

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "VxlanOrch",
            "add_vrf_vxlan_map"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("vrf_map_{}_{}", entry.key.vni, entry.key.vrf_name))
        .with_object_type("vrf_vxlan_map")
        .with_details(serde_json::json!({
            "vni": entry.key.vni,
            "vrf_name": entry.key.vrf_name,
            "stats": {
                "vrf_maps_created": self.stats.stats.vrf_maps_created
            }
        })));

        Ok(())
    }

    pub fn remove_vrf_map(&mut self, vni: u32, vrf_name: &str) -> Result<VxlanVrfMapEntry, VxlanOrchError> {
        let key = VxlanVrfMapKey::new(vni, vrf_name.to_string());
        let entry = self.vrf_maps.remove(&key)
            .ok_or_else(|| VxlanOrchError::VrfMapNotFound(vni, vrf_name.to_string()))?;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "VxlanOrch",
            "remove_vrf_vxlan_map"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("vrf_map_{}_{}",vni, vrf_name))
        .with_object_type("vrf_vxlan_map")
        .with_details(serde_json::json!({
            "vni": vni,
            "vrf_name": vrf_name,
            "stats": {
                "vrf_maps_removed": self.stats.stats.vrf_maps_created  // Note: no removal counter in original
            }
        })));

        Ok(entry)
    }

    pub fn get_vrf_map(&self, vni: u32, vrf_name: &str) -> Option<&VxlanVrfMapEntry> {
        let key = VxlanVrfMapKey::new(vni, vrf_name.to_string());
        self.vrf_maps.get(&key)
    }

    pub fn add_vlan_map(&mut self, entry: VxlanVlanMapEntry) -> Result<(), VxlanOrchError> {
        let key = entry.key.clone();

        if self.vlan_maps.contains_key(&key) {
            let error = VxlanOrchError::SaiError("VLAN map already exists".to_string());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "VxlanOrch",
                "add_vlan_vxlan_map"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(format!("vlan_map_{}_{}", key.vni, key.vlan_id))
            .with_object_type("vlan_vxlan_map")
            .with_error(error.to_string()));
            return Err(error);
        }

        self.stats.stats.vlan_maps_created = self.stats.stats.vlan_maps_created.saturating_add(1);
        self.vlan_maps.insert(key, entry.clone());

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "VxlanOrch",
            "add_vlan_vxlan_map"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("vlan_map_{}_{}", entry.key.vni, entry.key.vlan_id))
        .with_object_type("vlan_vxlan_map")
        .with_details(serde_json::json!({
            "vni": entry.key.vni,
            "vlan_id": entry.key.vlan_id,
            "stats": {
                "vlan_maps_created": self.stats.stats.vlan_maps_created
            }
        })));

        Ok(())
    }

    pub fn remove_vlan_map(&mut self, vni: u32, vlan_id: u16) -> Result<VxlanVlanMapEntry, VxlanOrchError> {
        let key = VxlanVlanMapKey::new(vni, vlan_id);
        let entry = self.vlan_maps.remove(&key)
            .ok_or_else(|| VxlanOrchError::VlanMapNotFound(vni, vlan_id))?;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "VxlanOrch",
            "remove_vlan_vxlan_map"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("vlan_map_{}_{}", vni, vlan_id))
        .with_object_type("vlan_vxlan_map")
        .with_details(serde_json::json!({
            "vni": vni,
            "vlan_id": vlan_id,
            "stats": {
                "vlan_maps_removed": self.stats.stats.vlan_maps_created  // Note: no removal counter in original
            }
        })));

        Ok(entry)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tunnel(tunnel_name: &str, src_ip: &str, dst_ip: &str) -> VxlanTunnelEntry {
        let src_addr: std::net::IpAddr = src_ip.parse().unwrap();
        let dst_addr: std::net::IpAddr = dst_ip.parse().unwrap();
        VxlanTunnelEntry {
            key: super::super::types::VxlanTunnelKey::new(src_addr, dst_addr),
            config: super::super::types::VxlanTunnelConfig {
                src_ip: src_addr,
                dst_ip: dst_addr,
                tunnel_name: tunnel_name.to_string(),
            },
            tunnel_oid: 0,
            encap_mapper_oid: 0,
            decap_mapper_oid: 0,
        }
    }

    fn create_test_vrf_map(vni: u32, vrf_name: &str) -> VxlanVrfMapEntry {
        super::super::types::VxlanVrfMapEntry::new(
            super::super::types::VxlanVrfMapKey::new(vni, vrf_name.to_string())
        )
    }

    fn create_test_vlan_map(vni: u32, vlan_id: u16) -> VxlanVlanMapEntry {
        super::super::types::VxlanVlanMapEntry::new(
            super::super::types::VxlanVlanMapKey::new(vni, vlan_id)
        )
    }

    #[test]
    fn test_add_tunnel() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let tunnel = create_test_tunnel("vtep1", "10.0.0.1", "10.0.0.2");

        assert_eq!(orch.tunnel_count(), 0);
        orch.add_tunnel(tunnel).unwrap();
        assert_eq!(orch.tunnel_count(), 1);
        assert_eq!(orch.stats().stats.tunnels_created, 1);
    }

    #[test]
    fn test_add_duplicate_tunnel() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        // Use same src/dst IPs to create truly duplicate keys
        let tunnel1 = create_test_tunnel("vtep1", "10.0.0.1", "10.0.0.2");
        let tunnel2 = create_test_tunnel("vtep2", "10.0.0.1", "10.0.0.2");

        orch.add_tunnel(tunnel1).unwrap();
        let result = orch.add_tunnel(tunnel2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VxlanOrchError::SaiError(_)));
    }

    #[test]
    fn test_remove_tunnel() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let tunnel = create_test_tunnel("vtep1", "10.0.0.1", "10.0.0.2");
        let key = tunnel.key.clone();

        orch.add_tunnel(tunnel).unwrap();
        assert_eq!(orch.tunnel_count(), 1);

        let removed = orch.remove_tunnel(&key).unwrap();
        let expected_src: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        assert_eq!(removed.config.src_ip, expected_src);
        assert_eq!(orch.tunnel_count(), 0);
    }

    #[test]
    fn test_remove_tunnel_not_found() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let src_ip: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        let dst_ip: std::net::IpAddr = "10.0.0.2".parse().unwrap();
        let key = super::super::types::VxlanTunnelKey::new(src_ip, dst_ip);

        let result = orch.remove_tunnel(&key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VxlanOrchError::TunnelNotFound(_)));
    }

    #[test]
    fn test_add_vrf_map() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let vrf_map = create_test_vrf_map(1000, "Vrf_default");

        orch.add_vrf_map(vrf_map).unwrap();
        assert_eq!(orch.stats().stats.vrf_maps_created, 1);

        let retrieved = orch.get_vrf_map(1000, "Vrf_default").unwrap();
        assert_eq!(retrieved.key.vrf_name, "Vrf_default");
    }

    #[test]
    fn test_add_duplicate_vrf_map() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let vrf_map1 = create_test_vrf_map(1000, "Vrf_default");
        let vrf_map2 = create_test_vrf_map(1000, "Vrf_default");

        orch.add_vrf_map(vrf_map1).unwrap();
        let result = orch.add_vrf_map(vrf_map2);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_vrf_map() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let vrf_map = create_test_vrf_map(1000, "Vrf_default");

        orch.add_vrf_map(vrf_map).unwrap();
        let removed = orch.remove_vrf_map(1000, "Vrf_default").unwrap();
        assert_eq!(removed.key.vrf_name, "Vrf_default");

        let result = orch.get_vrf_map(1000, "Vrf_default");
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_vrf_map_not_found() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let result = orch.remove_vrf_map(1000, "Vrf_default");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VxlanOrchError::VrfMapNotFound(_, _)));
    }

    #[test]
    fn test_add_vlan_map() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let vlan_map = create_test_vlan_map(2000, 100);

        orch.add_vlan_map(vlan_map).unwrap();
        assert_eq!(orch.stats().stats.vlan_maps_created, 1);

        let retrieved = orch.get_vlan_map(2000, 100).unwrap();
        assert_eq!(retrieved.key.vlan_id, 100);
    }

    #[test]
    fn test_add_duplicate_vlan_map() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let vlan_map1 = create_test_vlan_map(2000, 100);
        let vlan_map2 = create_test_vlan_map(2000, 100);

        orch.add_vlan_map(vlan_map1).unwrap();
        let result = orch.add_vlan_map(vlan_map2);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_vlan_map() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        let vlan_map = create_test_vlan_map(2000, 100);

        orch.add_vlan_map(vlan_map).unwrap();
        let removed = orch.remove_vlan_map(2000, 100).unwrap();
        assert_eq!(removed.key.vlan_id, 100);

        let result = orch.get_vlan_map(2000, 100);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_maps_by_vni() {
        let mut orch = VxlanOrch::new(VxlanOrchConfig::default());
        orch.add_vrf_map(create_test_vrf_map(1000, "Vrf1")).unwrap();
        orch.add_vrf_map(create_test_vrf_map(1000, "Vrf2")).unwrap();
        orch.add_vlan_map(create_test_vlan_map(1000, 100)).unwrap();
        orch.add_vlan_map(create_test_vlan_map(1000, 200)).unwrap();
        orch.add_vrf_map(create_test_vrf_map(2000, "Vrf3")).unwrap();

        let (vrf_maps, vlan_maps) = orch.get_maps_by_vni(1000);
        assert_eq!(vrf_maps.len(), 2);
        assert_eq!(vlan_maps.len(), 2);

        let (vrf_maps_2000, vlan_maps_2000) = orch.get_maps_by_vni(2000);
        assert_eq!(vrf_maps_2000.len(), 1);
        assert_eq!(vlan_maps_2000.len(), 0);
    }

    #[test]
    fn test_get_maps_by_vni_empty() {
        let orch = VxlanOrch::new(VxlanOrchConfig::default());
        let (vrf_maps, vlan_maps) = orch.get_maps_by_vni(9999);
        assert_eq!(vrf_maps.len(), 0);
        assert_eq!(vlan_maps.len(), 0);
    }
}
