//! ICMP echo (ping) responder types.

use std::net::IpAddr;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IcmpEchoKey {
    pub vrf_name: String,
    pub ip: IpAddr,
}

impl IcmpEchoKey {
    pub fn new(vrf_name: String, ip: IpAddr) -> Self {
        Self { vrf_name, ip }
    }
}

#[derive(Debug, Clone)]
pub struct IcmpEchoEntry {
    pub key: IcmpEchoKey,
    pub mode: IcmpMode,
}

impl IcmpEchoEntry {
    pub fn new(key: IcmpEchoKey, mode: IcmpMode) -> Self {
        Self { key, mode }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcmpMode {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Default)]
pub struct IcmpStats {
    pub entries_added: u64,
    pub entries_removed: u64,
}
