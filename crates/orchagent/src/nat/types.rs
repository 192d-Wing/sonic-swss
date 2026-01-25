//! NAT (Network Address Translation) types.

use std::net::{IpAddr, Ipv4Addr};

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NatType {
    Source,
    Destination,
    DoubleNat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NatProtocol {
    Tcp,
    Udp,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NatEntryKey {
    pub src_ip: Ipv4Addr,
    pub dst_ip: Ipv4Addr,
    pub protocol: NatProtocol,
    pub src_port: u16,
    pub dst_port: u16,
}

impl NatEntryKey {
    pub fn new(
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
        protocol: NatProtocol,
        src_port: u16,
        dst_port: u16,
    ) -> Self {
        Self {
            src_ip,
            dst_ip,
            protocol,
            src_port,
            dst_port,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NatEntryConfig {
    pub nat_type: NatType,
    pub translated_src_ip: Option<Ipv4Addr>,
    pub translated_dst_ip: Option<Ipv4Addr>,
    pub translated_src_port: Option<u16>,
    pub translated_dst_port: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct NatEntry {
    pub key: NatEntryKey,
    pub config: NatEntryConfig,
    pub entry_oid: RawSaiObjectId,
}

impl NatEntry {
    pub fn new(key: NatEntryKey, config: NatEntryConfig) -> Self {
        Self {
            key,
            config,
            entry_oid: 0,
        }
    }

    pub fn is_snat(&self) -> bool {
        self.config.nat_type == NatType::Source
    }

    pub fn is_dnat(&self) -> bool {
        self.config.nat_type == NatType::Destination
    }

    pub fn is_double_nat(&self) -> bool {
        self.config.nat_type == NatType::DoubleNat
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NatPoolKey {
    pub pool_name: String,
}

impl NatPoolKey {
    pub fn new(pool_name: String) -> Self {
        Self { pool_name }
    }
}

#[derive(Debug, Clone)]
pub struct NatPoolConfig {
    pub ip_range: (Ipv4Addr, Ipv4Addr),
    pub port_range: Option<(u16, u16)>,
}

#[derive(Debug, Clone)]
pub struct NatPoolEntry {
    pub key: NatPoolKey,
    pub config: NatPoolConfig,
    pub pool_oid: RawSaiObjectId,
}

impl NatPoolEntry {
    pub fn new(key: NatPoolKey, config: NatPoolConfig) -> Self {
        Self {
            key,
            config,
            pool_oid: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NatAclKey {
    pub acl_name: String,
}

impl NatAclKey {
    pub fn new(acl_name: String) -> Self {
        Self { acl_name }
    }
}

#[derive(Debug, Clone)]
pub struct NatAclEntry {
    pub key: NatAclKey,
    pub acl_table_oid: RawSaiObjectId,
    pub acl_entry_oid: RawSaiObjectId,
}

impl NatAclEntry {
    pub fn new(key: NatAclKey) -> Self {
        Self {
            key,
            acl_table_oid: 0,
            acl_entry_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NatStats {
    pub entries_created: u64,
    pub pools_created: u64,
    pub acls_created: u64,
    pub translations: u64,
}
