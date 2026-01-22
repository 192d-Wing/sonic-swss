//! Policy-Based Hashing types.

use std::collections::HashSet;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PbhHashField {
    InnerDstIpv4,
    InnerSrcIpv4,
    InnerDstIpv6,
    InnerSrcIpv6,
    InnerL4DstPort,
    InnerL4SrcPort,
    InnerIpProtocol,
}

#[derive(Debug, Clone)]
pub struct PbhHashConfig {
    pub hash_field_list: Vec<PbhHashField>,
}

#[derive(Debug, Clone)]
pub struct PbhHashEntry {
    pub name: String,
    pub config: PbhHashConfig,
    pub sai_oid: RawSaiObjectId,
}

impl PbhHashEntry {
    pub fn new(name: String, config: PbhHashConfig) -> Self {
        Self {
            name,
            config,
            sai_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PbhTableInterface {
    Port,
    PortChannel,
    Vlan,
}

#[derive(Debug, Clone)]
pub struct PbhTableConfig {
    pub interface_list: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PbhTableEntry {
    pub name: String,
    pub config: PbhTableConfig,
    pub sai_oid: RawSaiObjectId,
}

impl PbhTableEntry {
    pub fn new(name: String, config: PbhTableConfig) -> Self {
        Self {
            name,
            config,
            sai_oid: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PbhRuleConfig {
    pub priority: u32,
    pub gre_key: Option<String>,
    pub ether_type: Option<String>,
    pub ip_protocol: Option<String>,
    pub ipv6_next_header: Option<String>,
    pub l4_dst_port: Option<u16>,
    pub inner_ether_type: Option<String>,
    pub hash: String,
    pub packet_action: PbhPacketAction,
    pub flow_counter: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbhPacketAction {
    SetEcmpHash,
    SetLagHash,
}

#[derive(Debug, Clone)]
pub struct PbhRuleEntry {
    pub table_name: String,
    pub rule_name: String,
    pub config: PbhRuleConfig,
    pub sai_oid: RawSaiObjectId,
}

impl PbhRuleEntry {
    pub fn new(table_name: String, rule_name: String, config: PbhRuleConfig) -> Self {
        Self {
            table_name,
            rule_name,
            config,
            sai_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PbhStats {
    pub hashes_created: u64,
    pub tables_created: u64,
    pub rules_created: u64,
}
