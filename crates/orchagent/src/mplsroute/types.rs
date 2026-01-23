//! MPLS route types.

pub type RawSaiObjectId = u64;
pub type MplsLabel = u32;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MplsRouteKey {
    pub label: MplsLabel,
}

impl MplsRouteKey {
    pub fn new(label: MplsLabel) -> Self {
        Self { label }
    }

    pub fn validate_label(&self) -> Result<(), String> {
        if self.label > 1_048_575 {
            Err(format!("Invalid MPLS label {}, max is 1048575", self.label))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MplsAction {
    Pop,
    Swap,
    Push,
}

#[derive(Debug, Clone)]
pub struct MplsRouteConfig {
    pub action: MplsAction,
    pub next_hop: Option<String>,
    pub swap_label: Option<MplsLabel>,
    pub push_labels: Vec<MplsLabel>,
}

#[derive(Debug, Clone)]
pub struct MplsRouteEntry {
    pub key: MplsRouteKey,
    pub config: MplsRouteConfig,
    pub route_oid: RawSaiObjectId,
    pub nh_oid: RawSaiObjectId,
}

impl MplsRouteEntry {
    pub fn new(key: MplsRouteKey, config: MplsRouteConfig) -> Self {
        Self {
            key,
            config,
            route_oid: 0,
            nh_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MplsRouteStats {
    pub routes_created: u64,
    pub routes_removed: u64,
}
