//! MPLS route orchestration logic.

use super::types::{MplsRouteEntry, MplsRouteKey, MplsRouteStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum MplsRouteOrchError {
    RouteNotFound(MplsRouteKey),
    InvalidLabel(u32),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct MplsRouteOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct MplsRouteOrchStats {
    pub stats: MplsRouteStats,
    pub errors: u64,
}

pub trait MplsRouteOrchCallbacks: Send + Sync {}

pub struct MplsRouteOrch {
    config: MplsRouteOrchConfig,
    stats: MplsRouteOrchStats,
    routes: HashMap<MplsRouteKey, MplsRouteEntry>,
}

impl MplsRouteOrch {
    pub fn new(config: MplsRouteOrchConfig) -> Self {
        Self {
            config,
            stats: MplsRouteOrchStats::default(),
            routes: HashMap::new(),
        }
    }

    pub fn get_route(&self, key: &MplsRouteKey) -> Option<&MplsRouteEntry> {
        self.routes.get(key)
    }

    pub fn stats(&self) -> &MplsRouteOrchStats {
        &self.stats
    }
}
