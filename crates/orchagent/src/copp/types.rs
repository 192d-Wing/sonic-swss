//! CoPP (Control Plane Policing) types.

use std::collections::HashMap;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoppTrapKey {
    pub trap_id: String,
}

impl CoppTrapKey {
    pub fn new(trap_id: String) -> Self {
        Self { trap_id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoppTrapAction {
    Drop,
    Forward,
    Copy,
    CopyCancel,
    Trap,
    Log,
}

#[derive(Debug, Clone)]
pub struct CoppTrapConfig {
    pub trap_action: CoppTrapAction,
    pub trap_priority: Option<u32>,
    pub queue: Option<u8>,
    pub meter_type: Option<String>,
    pub mode: Option<String>,
    pub color: Option<String>,
    pub cbs: Option<u64>,
    pub cir: Option<u64>,
    pub pbs: Option<u64>,
    pub pir: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CoppTrapEntry {
    pub key: CoppTrapKey,
    pub config: CoppTrapConfig,
    pub trap_oid: RawSaiObjectId,
    pub trap_group_oid: RawSaiObjectId,
    pub policer_oid: RawSaiObjectId,
}

impl CoppTrapEntry {
    pub fn new(key: CoppTrapKey, config: CoppTrapConfig) -> Self {
        Self {
            key,
            config,
            trap_oid: 0,
            trap_group_oid: 0,
            policer_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CoppStats {
    pub traps_created: u64,
    pub trap_groups_created: u64,
    pub policers_created: u64,
}
