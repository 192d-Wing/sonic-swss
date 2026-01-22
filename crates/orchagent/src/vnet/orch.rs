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

    pub fn get_route(&self, key: &VnetRouteKey) -> Option<&VnetRouteEntry> {
        self.routes.get(key)
    }

    pub fn stats(&self) -> &VnetOrchStats {
        &self.stats
    }
}
