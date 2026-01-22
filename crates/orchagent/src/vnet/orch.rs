//! VNET orchestration logic.

use super::types::{VnetEntry, VnetKey, VnetRouteEntry, VnetRouteKey, VnetStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum VnetOrchError {
    VnetNotFound(VnetKey),
    RouteNotFound(VnetRouteKey),
    InvalidPrefix(String),
    InvalidEndpoint(String),
    VniNotFound(u32),
    TunnelNotFound(String),
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
            return Err(VnetOrchError::SaiError("VNET already exists".to_string()));
        }

        self.stats.stats.vnets_created = self.stats.stats.vnets_created.saturating_add(1);
        self.vnets.insert(key, entry);

        Ok(())
    }

    pub fn remove_vnet(&mut self, key: &VnetKey) -> Result<VnetEntry, VnetOrchError> {
        // Check if any routes exist for this VNET
        let has_routes = self.routes
            .keys()
            .any(|route_key| route_key.vnet_name == key.vnet_name);

        if has_routes {
            return Err(VnetOrchError::SaiError(
                format!("VNET {} still has routes", key.vnet_name)
            ));
        }

        self.vnets.remove(key)
            .ok_or_else(|| VnetOrchError::VnetNotFound(key.clone()))
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
            return Err(VnetOrchError::SaiError("Route already exists".to_string()));
        }

        self.stats.stats.routes_created = self.stats.stats.routes_created.saturating_add(1);
        self.routes.insert(key, entry);

        Ok(())
    }

    pub fn remove_route(&mut self, key: &VnetRouteKey) -> Result<VnetRouteEntry, VnetOrchError> {
        self.routes.remove(key)
            .ok_or_else(|| VnetOrchError::RouteNotFound(key.clone()))
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
