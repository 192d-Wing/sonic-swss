//! Chassis management types for modular systems.

use std::collections::HashMap;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemPortKey {
    pub system_port_id: u32,
}

impl SystemPortKey {
    pub fn new(system_port_id: u32) -> Self {
        Self { system_port_id }
    }
}

#[derive(Debug, Clone)]
pub struct SystemPortConfig {
    pub system_port_id: u32,
    pub switch_id: u32,
    pub core_index: u32,
    pub core_port_index: u32,
    pub speed: u32,
}

#[derive(Debug, Clone)]
pub struct SystemPortEntry {
    pub key: SystemPortKey,
    pub config: SystemPortConfig,
    pub sai_oid: RawSaiObjectId,
}

impl SystemPortEntry {
    pub fn new(config: SystemPortConfig) -> Self {
        let key = SystemPortKey::new(config.system_port_id);
        Self {
            key,
            config,
            sai_oid: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FabricPortKey {
    pub fabric_port_id: u32,
}

impl FabricPortKey {
    pub fn new(fabric_port_id: u32) -> Self {
        Self { fabric_port_id }
    }
}

#[derive(Debug, Clone)]
pub struct FabricPortEntry {
    pub key: FabricPortKey,
    pub isolate: bool,
    pub sai_oid: RawSaiObjectId,
}

impl FabricPortEntry {
    pub fn new(fabric_port_id: u32) -> Self {
        Self {
            key: FabricPortKey::new(fabric_port_id),
            isolate: false,
            sai_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChassisStats {
    pub system_ports_created: u64,
    pub fabric_ports_created: u64,
}
