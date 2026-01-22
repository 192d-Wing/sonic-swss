//! MUX cable orchestration types.

use std::collections::HashMap;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MuxState {
    Active,
    Standby,
    Unknown,
}

impl Default for MuxState {
    fn default() -> Self {
        MuxState::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxCableType {
    ActiveActive,
    ActiveStandby,
}

impl Default for MuxCableType {
    fn default() -> Self {
        MuxCableType::ActiveStandby
    }
}

#[derive(Debug, Clone)]
pub struct MuxPortConfig {
    pub server_ipv4: Option<String>,
    pub server_ipv6: Option<String>,
    pub soc_ipv4: Option<String>,
    pub cable_type: MuxCableType,
}

impl Default for MuxPortConfig {
    fn default() -> Self {
        Self {
            server_ipv4: None,
            server_ipv6: None,
            soc_ipv4: None,
            cable_type: MuxCableType::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MuxPortEntry {
    pub port_name: String,
    pub config: MuxPortConfig,
    pub state: MuxState,
    pub tunnel_oid: RawSaiObjectId,
    pub acl_handler_oid: RawSaiObjectId,
}

impl MuxPortEntry {
    pub fn new(port_name: String, config: MuxPortConfig) -> Self {
        Self {
            port_name,
            config,
            state: MuxState::default(),
            tunnel_oid: 0,
            acl_handler_oid: 0,
        }
    }

    pub fn set_state(&mut self, state: MuxState) {
        self.state = state;
    }

    pub fn is_active(&self) -> bool {
        self.state == MuxState::Active
    }

    pub fn is_standby(&self) -> bool {
        self.state == MuxState::Standby
    }
}

#[derive(Debug, Clone)]
pub struct MuxNeighborConfig {
    pub neighbor: String,
    pub address: String,
}

#[derive(Debug, Clone)]
pub struct MuxNeighborEntry {
    pub port_name: String,
    pub config: MuxNeighborConfig,
    pub neigh_oid: RawSaiObjectId,
}

impl MuxNeighborEntry {
    pub fn new(port_name: String, config: MuxNeighborConfig) -> Self {
        Self {
            port_name,
            config,
            neigh_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MuxStats {
    pub state_changes: u64,
    pub active_transitions: u64,
    pub standby_transitions: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxStateChange {
    ToActive,
    ToStandby,
    ToUnknown,
}
