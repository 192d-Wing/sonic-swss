//! QoS (Quality of Service) types.

use std::collections::HashMap;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QosMapType {
    DscpToTc,
    DscpToQueue,
    DscpToColor,
    TcToQueue,
    TcToPg,
    PfcPriorityToQueue,
    DscpToFc,
    ExpToFc,
}

#[derive(Debug, Clone)]
pub struct QosMapEntry {
    pub name: String,
    pub map_type: QosMapType,
    pub mappings: HashMap<u8, u8>,
    pub sai_oid: RawSaiObjectId,
}

impl QosMapEntry {
    pub fn new(name: String, map_type: QosMapType) -> Self {
        Self {
            name,
            map_type,
            mappings: HashMap::new(),
            sai_oid: 0,
        }
    }

    pub fn add_mapping(&mut self, from: u8, to: u8) {
        self.mappings.insert(from, to);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerType {
    Strict,
    Dwrr,
    Wrr,
}

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub scheduler_type: SchedulerType,
    pub weight: u8,
    pub meter_type: Option<MeterType>,
    pub cir: Option<u64>,
    pub cbs: Option<u64>,
    pub pir: Option<u64>,
    pub pbs: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterType {
    Bytes,
    Packets,
}

#[derive(Debug, Clone)]
pub struct SchedulerEntry {
    pub name: String,
    pub config: SchedulerConfig,
    pub sai_oid: RawSaiObjectId,
}

impl SchedulerEntry {
    pub fn new(name: String, config: SchedulerConfig) -> Self {
        Self {
            name,
            config,
            sai_oid: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WredProfile {
    pub name: String,
    pub green_enable: bool,
    pub green_min_threshold: Option<u32>,
    pub green_max_threshold: Option<u32>,
    pub green_drop_probability: Option<u8>,
    pub yellow_enable: bool,
    pub yellow_min_threshold: Option<u32>,
    pub yellow_max_threshold: Option<u32>,
    pub yellow_drop_probability: Option<u8>,
    pub red_enable: bool,
    pub red_min_threshold: Option<u32>,
    pub red_max_threshold: Option<u32>,
    pub red_drop_probability: Option<u8>,
    pub ecn_mark: Option<String>,
    pub sai_oid: RawSaiObjectId,
}

impl WredProfile {
    pub fn new(name: String) -> Self {
        Self {
            name,
            green_enable: false,
            green_min_threshold: None,
            green_max_threshold: None,
            green_drop_probability: None,
            yellow_enable: false,
            yellow_min_threshold: None,
            yellow_max_threshold: None,
            yellow_drop_probability: None,
            red_enable: false,
            red_min_threshold: None,
            red_max_threshold: None,
            red_drop_probability: None,
            ecn_mark: None,
            sai_oid: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TcToQueueMapEntry {
    pub tc: u8,
    pub queue: u8,
}

#[derive(Debug, Clone, Default)]
pub struct QosStats {
    pub maps_created: u64,
    pub schedulers_created: u64,
    pub wred_profiles_created: u64,
}
