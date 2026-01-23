//! VNET orchestration logic.

use super::types::{VnetEntry, VnetKey, VnetRouteEntry, VnetRouteKey, VnetStats};
use std::collections::HashMap;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};

#[derive(Debug, Clone, thiserror::Error)]
pub enum VnetOrchError {
    #[error("VNET not found: {0:?}")]
    VnetNotFound(VnetKey),
    #[error("Route not found: {0:?}")]
    RouteNotFound(VnetRouteKey),
    #[error("Invalid prefix: {0}")]
    InvalidPrefix(String),
    #[error("Invalid endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("VNI not found: {0}")]
    VniNotFound(u32),
    #[error("Tunnel not found: {0}")]
    TunnelNotFound(String),
    #[error("SAI error: {0}")]
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct VnetOrchConfig {
    pub enable_bfd: bool,
    pub enable_monitoring: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VnetOrchStats {
    pub stats: VnetStats,
    pub errors: u64,
}

pub trait VnetOrchCallbacks: Send + Sync {
    fn on_vnet_created(&self, entry: &VnetEntry);
    fn on_vnet_removed(&self, key: &VnetKey);
    fn on_route_created(&self, entry: &VnetRouteEntry);
    fn on_route_removed(&self, key: &VnetRouteKey);
}

pub struct VnetOrch {
    config: VnetOrchConfig,
    stats: VnetOrchStats,
    vnets: HashMap<VnetKey, VnetEntry>,
    routes: HashMap<VnetRouteKey, VnetRouteEntry>,
}

impl VnetOrch {
    pub fn new(config: VnetOrchConfig) -> Self {
        Self {
            config,
            stats: VnetOrchStats::default(),
            vnets: HashMap::new(),
            routes: HashMap::new(),
        }
    }

    pub fn get_vnet(&self, key: &VnetKey) -> Option<&VnetEntry> {
        self.vnets.get(key)
    }

    pub fn add_vnet(&mut self, entry: VnetEntry) -> Result<(), VnetOrchError> {
        let key = entry.key.clone();

        if self.vnets.contains_key(&key) {
            let error = VnetOrchError::SaiError("VNET already exists".to_string());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "VnetOrch",
                "add_vnet"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(entry.config.vnet_name.clone())
            .with_object_type("vnet")
            .with_error(error.to_string()));
            return Err(error);
        }

        self.stats.stats.vnets_created = self.stats.stats.vnets_created.saturating_add(1);
        self.vnets.insert(key, entry.clone());

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "VnetOrch",
            "add_vnet"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(entry.config.vnet_name.clone())
        .with_object_type("vnet")
        .with_details(serde_json::json!({
            "vnet_name": entry.config.vnet_name,
            "vni": entry.config.vni,
            "vxlan_tunnel": entry.config.vxlan_tunnel,
            "stats": {
                "vnets_created": self.stats.stats.vnets_created
            }
        })));

        Ok(())
    }

    pub fn remove_vnet(&mut self, key: &VnetKey) -> Result<VnetEntry, VnetOrchError> {
        // Check if any routes exist for this VNET
        let has_routes = self.routes
            .keys()
            .any(|route_key| route_key.vnet_name == key.vnet_name);

        if has_routes {
            let error = VnetOrchError::SaiError(
                format!("VNET {} still has routes", key.vnet_name)
            );
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceDelete,
                "VnetOrch",
                "remove_vnet"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(key.vnet_name.clone())
            .with_object_type("vnet")
            .with_error(error.to_string()));
            return Err(error);
        }

        let entry = self.vnets.remove(key)
            .ok_or_else(|| VnetOrchError::VnetNotFound(key.clone()))?;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "VnetOrch",
            "remove_vnet"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(key.vnet_name.clone())
        .with_object_type("vnet")
        .with_details(serde_json::json!({
            "vnet_name": key.vnet_name,
            "vni": entry.config.vni
        })));

        Ok(entry)
    }

    pub fn get_route(&self, key: &VnetRouteKey) -> Option<&VnetRouteEntry> {
        self.routes.get(key)
    }

    pub fn add_route(&mut self, entry: VnetRouteEntry) -> Result<(), VnetOrchError> {
        let key = entry.key.clone();

        // Verify VNET exists
        let vnet_key = VnetKey::new(key.vnet_name.clone());
        if !self.vnets.contains_key(&vnet_key) {
            return Err(VnetOrchError::VnetNotFound(vnet_key));
        }

        if self.routes.contains_key(&key) {
            let error = VnetOrchError::SaiError("Route already exists".to_string());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "VnetOrch",
                "add_vnet_route"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(format!("{}/{}", key.vnet_name, key.prefix))
            .with_object_type("vnet_route")
            .with_error(error.to_string()));
            return Err(error);
        }

        self.stats.stats.routes_created = self.stats.stats.routes_created.saturating_add(1);
        self.routes.insert(key, entry.clone());

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "VnetOrch",
            "add_vnet_route"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("{}/{}", entry.key.vnet_name, entry.key.prefix))
        .with_object_type("vnet_route")
        .with_details(serde_json::json!({
            "vnet_name": entry.key.vnet_name,
            "prefix": entry.key.prefix,
            "route_type": format!("{:?}", entry.config.route_type),
            "stats": {
                "routes_created": self.stats.stats.routes_created
            }
        })));

        Ok(())
    }

    pub fn remove_route(&mut self, key: &VnetRouteKey) -> Result<VnetRouteEntry, VnetOrchError> {
        let entry = self.routes.remove(key)
            .ok_or_else(|| VnetOrchError::RouteNotFound(key.clone()))?;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "VnetOrch",
            "remove_vnet_route"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("{}/{}", key.vnet_name, key.prefix))
        .with_object_type("vnet_route")
        .with_details(serde_json::json!({
            "vnet_name": key.vnet_name,
            "prefix": key.prefix,
            "route_type": format!("{:?}", entry.config.route_type)
        })));

        Ok(entry)
    }

    pub fn get_routes_for_vnet(&self, vnet_name: &str) -> Vec<&VnetRouteEntry> {
        self.routes
            .values()
            .filter(|entry| entry.key.vnet_name == vnet_name)
            .collect()
    }

    pub fn get_tunnel_routes(&self) -> Vec<&VnetRouteEntry> {
        self.routes
            .values()
            .filter(|entry| entry.is_tunnel_route())
            .collect()
    }

    pub fn get_vnet_routes(&self) -> Vec<&VnetRouteEntry> {
        self.routes
            .values()
            .filter(|entry| entry.is_vnet_route())
            .collect()
    }

    pub fn vnet_count(&self) -> usize {
        self.vnets.len()
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn stats(&self) -> &VnetOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{VnetConfig, VnetRouteConfig, VnetRouteType};
    use std::net::IpAddr;

    fn create_test_vnet(name: &str, vni: Option<u32>) -> VnetEntry {
        VnetEntry::new(VnetConfig {
            vnet_name: name.to_string(),
            vni,
            vxlan_tunnel: Some("tunnel0".to_string()),
            scope: None,
            advertise_prefix: false,
        })
    }

    fn create_test_route(vnet_name: &str, prefix: &str, route_type: VnetRouteType) -> VnetRouteEntry {
        let key = VnetRouteKey::new(vnet_name.to_string(), prefix.to_string());
        let config = VnetRouteConfig {
            route_type,
            endpoint: None,
            endpoint_monitor: None,
            mac_address: None,
            vni: None,
            peer_list: vec![],
        };
        VnetRouteEntry::new(key, config)
    }

    fn create_test_tunnel_route(vnet_name: &str, prefix: &str, endpoint: &str) -> VnetRouteEntry {
        let key = VnetRouteKey::new(vnet_name.to_string(), prefix.to_string());
        let config = VnetRouteConfig {
            route_type: VnetRouteType::Tunnel,
            endpoint: Some(endpoint.parse::<IpAddr>().unwrap()),
            endpoint_monitor: None,
            mac_address: None,
            vni: Some(1000),
            peer_list: vec![],
        };
        VnetRouteEntry::new(key, config)
    }

    #[test]
    fn test_add_vnet() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        let vnet = create_test_vnet("Vnet1", Some(100));

        assert_eq!(orch.vnet_count(), 0);
        let result = orch.add_vnet(vnet.clone());
        assert!(result.is_ok());
        assert_eq!(orch.vnet_count(), 1);
        assert_eq!(orch.stats().stats.vnets_created, 1);

        // Verify we can retrieve the VNET
        let key = VnetKey::new("Vnet1".to_string());
        let retrieved = orch.get_vnet(&key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().config.vnet_name, "Vnet1");
    }

    #[test]
    fn test_add_duplicate_vnet() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        let vnet1 = create_test_vnet("Vnet1", Some(100));
        let vnet2 = create_test_vnet("Vnet1", Some(200));

        // First add should succeed
        let result1 = orch.add_vnet(vnet1);
        assert!(result1.is_ok());
        assert_eq!(orch.vnet_count(), 1);

        // Second add with same name should fail
        let result2 = orch.add_vnet(vnet2);
        assert!(result2.is_err());
        assert_eq!(orch.vnet_count(), 1);

        // Verify error is SaiError about duplicate
        match result2.unwrap_err() {
            VnetOrchError::SaiError(msg) => {
                assert!(msg.contains("already exists"));
            }
            _ => panic!("Expected SaiError"),
        }
    }

    #[test]
    fn test_remove_vnet() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        let vnet = create_test_vnet("Vnet1", Some(100));
        let key = vnet.key.clone();

        // Add then remove
        orch.add_vnet(vnet).unwrap();
        assert_eq!(orch.vnet_count(), 1);

        let result = orch.remove_vnet(&key);
        assert!(result.is_ok());
        assert_eq!(orch.vnet_count(), 0);

        // Verify VNET is gone
        assert!(orch.get_vnet(&key).is_none());
    }

    #[test]
    fn test_remove_vnet_with_routes() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        let vnet = create_test_vnet("Vnet1", Some(100));
        let key = vnet.key.clone();

        // Add VNET and a route
        orch.add_vnet(vnet).unwrap();
        let route = create_test_route("Vnet1", "10.0.0.0/24", VnetRouteType::Direct);
        orch.add_route(route).unwrap();

        // Attempt to remove VNET should fail due to existing route
        let result = orch.remove_vnet(&key);
        assert!(result.is_err());
        assert_eq!(orch.vnet_count(), 1);

        // Verify error message mentions routes
        match result.unwrap_err() {
            VnetOrchError::SaiError(msg) => {
                assert!(msg.contains("still has routes"));
            }
            _ => panic!("Expected SaiError about routes"),
        }
    }

    #[test]
    fn test_add_route() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        let vnet = create_test_vnet("Vnet1", Some(100));

        // Add VNET first
        orch.add_vnet(vnet).unwrap();

        // Add route
        let route = create_test_route("Vnet1", "10.0.0.0/24", VnetRouteType::Direct);
        let result = orch.add_route(route);
        assert!(result.is_ok());
        assert_eq!(orch.route_count(), 1);
        assert_eq!(orch.stats().stats.routes_created, 1);

        // Verify we can retrieve the route
        let route_key = VnetRouteKey::new("Vnet1".to_string(), "10.0.0.0/24".to_string());
        let retrieved = orch.get_route(&route_key);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_add_route_without_vnet() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());

        // Try to add route without VNET
        let route = create_test_route("Vnet1", "10.0.0.0/24", VnetRouteType::Direct);
        let result = orch.add_route(route);

        assert!(result.is_err());
        assert_eq!(orch.route_count(), 0);

        // Verify error is VnetNotFound
        match result.unwrap_err() {
            VnetOrchError::VnetNotFound(key) => {
                assert_eq!(key.vnet_name, "Vnet1");
            }
            _ => panic!("Expected VnetNotFound error"),
        }
    }

    #[test]
    fn test_remove_route() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        let vnet = create_test_vnet("Vnet1", Some(100));
        orch.add_vnet(vnet).unwrap();

        // Add and remove route
        let route = create_test_route("Vnet1", "10.0.0.0/24", VnetRouteType::Direct);
        let route_key = route.key.clone();
        orch.add_route(route).unwrap();
        assert_eq!(orch.route_count(), 1);

        let result = orch.remove_route(&route_key);
        assert!(result.is_ok());
        assert_eq!(orch.route_count(), 0);

        // Verify route is gone
        assert!(orch.get_route(&route_key).is_none());
    }

    #[test]
    fn test_get_routes_for_vnet() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());

        // Create two VNETs
        orch.add_vnet(create_test_vnet("Vnet1", Some(100))).unwrap();
        orch.add_vnet(create_test_vnet("Vnet2", Some(200))).unwrap();

        // Add routes to different VNETs
        orch.add_route(create_test_route("Vnet1", "10.0.0.0/24", VnetRouteType::Direct)).unwrap();
        orch.add_route(create_test_route("Vnet1", "10.0.1.0/24", VnetRouteType::Direct)).unwrap();
        orch.add_route(create_test_route("Vnet2", "10.0.2.0/24", VnetRouteType::Direct)).unwrap();

        // Get routes for Vnet1
        let vnet1_routes = orch.get_routes_for_vnet("Vnet1");
        assert_eq!(vnet1_routes.len(), 2);

        // Get routes for Vnet2
        let vnet2_routes = orch.get_routes_for_vnet("Vnet2");
        assert_eq!(vnet2_routes.len(), 1);

        // Get routes for non-existent VNET
        let vnet3_routes = orch.get_routes_for_vnet("Vnet3");
        assert_eq!(vnet3_routes.len(), 0);
    }

    #[test]
    fn test_get_tunnel_routes() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        orch.add_vnet(create_test_vnet("Vnet1", Some(100))).unwrap();

        // Add different types of routes
        orch.add_route(create_test_route("Vnet1", "10.0.0.0/24", VnetRouteType::Direct)).unwrap();
        orch.add_route(create_test_tunnel_route("Vnet1", "10.0.1.0/24", "192.168.1.1")).unwrap();
        orch.add_route(create_test_tunnel_route("Vnet1", "10.0.2.0/24", "192.168.1.2")).unwrap();
        orch.add_route(create_test_route("Vnet1", "10.0.3.0/24", VnetRouteType::Vnet)).unwrap();

        // Get only tunnel routes
        let tunnel_routes = orch.get_tunnel_routes();
        assert_eq!(tunnel_routes.len(), 2);

        // Verify all returned routes are tunnel routes
        for route in tunnel_routes {
            assert!(route.is_tunnel_route());
            assert_eq!(route.config.route_type, VnetRouteType::Tunnel);
        }
    }

    #[test]
    fn test_get_vnet_routes() {
        let mut orch = VnetOrch::new(VnetOrchConfig::default());
        orch.add_vnet(create_test_vnet("Vnet1", Some(100))).unwrap();

        // Add different types of routes
        orch.add_route(create_test_route("Vnet1", "10.0.0.0/24", VnetRouteType::Direct)).unwrap();
        orch.add_route(create_test_tunnel_route("Vnet1", "10.0.1.0/24", "192.168.1.1")).unwrap();
        orch.add_route(create_test_route("Vnet1", "10.0.2.0/24", VnetRouteType::Vnet)).unwrap();
        orch.add_route(create_test_route("Vnet1", "10.0.3.0/24", VnetRouteType::Vnet)).unwrap();

        // Get only VNET routes
        let vnet_routes = orch.get_vnet_routes();
        assert_eq!(vnet_routes.len(), 2);

        // Verify all returned routes are VNET routes
        for route in vnet_routes {
            assert!(route.is_vnet_route());
            assert_eq!(route.config.route_type, VnetRouteType::Vnet);
        }
    }
}
