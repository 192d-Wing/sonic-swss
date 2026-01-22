//! Buffer pool and queue types.

use std::collections::HashMap;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferPoolType {
    Ingress,
    Egress,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferPoolMode {
    Static,
    Dynamic,
}

#[derive(Debug, Clone)]
pub struct BufferPoolConfig {
    pub pool_type: BufferPoolType,
    pub mode: BufferPoolMode,
    pub size: u64,
    pub threshold_mode: ThresholdMode,
    pub xoff_threshold: Option<u64>,
    pub xon_threshold: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThresholdMode {
    Static,
    Dynamic,
}

#[derive(Debug, Clone)]
pub struct BufferPoolEntry {
    pub name: String,
    pub config: BufferPoolConfig,
    pub sai_oid: RawSaiObjectId,
    pub ref_count: u32,
}

impl BufferPoolEntry {
    pub fn new(name: String, config: BufferPoolConfig) -> Self {
        Self {
            name,
            config,
            sai_oid: 0,
            ref_count: 0,
        }
    }

    pub fn add_ref(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_add(1);
        self.ref_count
    }

    pub fn remove_ref(&mut self) -> Result<u32, String> {
        if self.ref_count == 0 {
            return Err("Reference count already 0".to_string());
        }
        self.ref_count -= 1;
        Ok(self.ref_count)
    }
}

#[derive(Debug, Clone)]
pub struct BufferProfileConfig {
    pub pool_name: String,
    pub size: u64,
    pub dynamic_threshold: Option<i8>,
    pub static_threshold: Option<u64>,
    pub xoff_threshold: Option<u64>,
    pub xon_threshold: Option<u64>,
    pub xon_offset: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct BufferProfileEntry {
    pub name: String,
    pub config: BufferProfileConfig,
    pub sai_oid: RawSaiObjectId,
    pub ref_count: u32,
}

impl BufferProfileEntry {
    pub fn new(name: String, config: BufferProfileConfig) -> Self {
        Self {
            name,
            config,
            sai_oid: 0,
            ref_count: 0,
        }
    }

    pub fn add_ref(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_add(1);
        self.ref_count
    }

    pub fn remove_ref(&mut self) -> Result<u32, String> {
        if self.ref_count == 0 {
            return Err("Reference count already 0".to_string());
        }
        self.ref_count -= 1;
        Ok(self.ref_count)
    }
}

#[derive(Debug, Clone)]
pub struct PriorityGroupConfig {
    pub buffer_profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IngressPriorityGroupEntry {
    pub port_name: String,
    pub priority_group_index: u8,
    pub config: PriorityGroupConfig,
    pub sai_oid: RawSaiObjectId,
}

#[derive(Debug, Clone)]
pub struct BufferQueueConfig {
    pub buffer_profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BufferQueueEntry {
    pub port_name: String,
    pub queue_index: u8,
    pub config: BufferQueueConfig,
    pub sai_oid: RawSaiObjectId,
}

#[derive(Debug, Clone, Default)]
pub struct BufferStats {
    pub pools_created: u64,
    pub profiles_created: u64,
    pub pg_bindings: u64,
    pub queue_bindings: u64,
}
