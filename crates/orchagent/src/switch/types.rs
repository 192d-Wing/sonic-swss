//! Switch-level configuration and capability types.

use std::collections::HashMap;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwitchHashAlgorithm {
    Crc,
    Xor,
    Random,
    CrcCcitt,
    Crc32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwitchHashField {
    SrcMac,
    DstMac,
    SrcIp,
    DstIp,
    L4SrcPort,
    L4DstPort,
    IpProtocol,
    InPort,
}

#[derive(Debug, Clone)]
pub struct SwitchHashConfig {
    pub algorithm: SwitchHashAlgorithm,
    pub fields: Vec<SwitchHashField>,
    pub seed: u32,
}

impl Default for SwitchHashConfig {
    fn default() -> Self {
        Self {
            algorithm: SwitchHashAlgorithm::Crc,
            fields: vec![
                SwitchHashField::SrcIp,
                SwitchHashField::DstIp,
                SwitchHashField::L4SrcPort,
                SwitchHashField::L4DstPort,
                SwitchHashField::IpProtocol,
            ],
            seed: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SwitchCapabilities {
    pub max_ports: u32,
    pub max_vlans: u32,
    pub max_vrfs: u32,
    pub max_nexthop_groups: u32,
    pub max_nexthops: u32,
    pub max_ecmp_paths: u32,
    pub max_acl_tables: u32,
    pub max_acl_entries: u32,
    pub supported_hash_algorithms: Vec<SwitchHashAlgorithm>,
}

impl Default for SwitchCapabilities {
    fn default() -> Self {
        Self {
            max_ports: 256,
            max_vlans: 4096,
            max_vrfs: 1000,
            max_nexthop_groups: 1024,
            max_nexthops: 16384,
            max_ecmp_paths: 64,
            max_acl_tables: 256,
            max_acl_entries: 4096,
            supported_hash_algorithms: vec![
                SwitchHashAlgorithm::Crc,
                SwitchHashAlgorithm::Xor,
                SwitchHashAlgorithm::Random,
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SwitchConfig {
    pub ecmp_hash: SwitchHashConfig,
    pub lag_hash: SwitchHashConfig,
    pub fdb_aging_time: u32,
    pub vxlan_port: u16,
    pub crm_polling_interval: u32,
    pub tunnel_types: Vec<String>,
}

impl Default for SwitchConfig {
    fn default() -> Self {
        Self {
            ecmp_hash: SwitchHashConfig::default(),
            lag_hash: SwitchHashConfig::default(),
            fdb_aging_time: 600,
            vxlan_port: 4789,
            crm_polling_interval: 300,
            tunnel_types: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SwitchState {
    pub switch_oid: RawSaiObjectId,
    pub cpu_port_oid: RawSaiObjectId,
    pub default_vlan_oid: RawSaiObjectId,
    pub default_1q_bridge_oid: RawSaiObjectId,
    pub capabilities: SwitchCapabilities,
    pub attributes: HashMap<String, String>,
}

impl Default for SwitchState {
    fn default() -> Self {
        Self {
            switch_oid: 0,
            cpu_port_oid: 0,
            default_vlan_oid: 0,
            default_1q_bridge_oid: 0,
            capabilities: SwitchCapabilities::default(),
            attributes: HashMap::new(),
        }
    }
}
