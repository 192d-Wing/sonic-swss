//! Neighbor (ARP/NDP) types.

use std::net::IpAddr;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NeighborKey {
    pub interface: String,
    pub ip: IpAddr,
}

impl NeighborKey {
    pub fn new(interface: String, ip: IpAddr) -> Self {
        Self { interface, ip }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacAddress {
    bytes: [u8; 6],
}

impl MacAddress {
    pub fn new(bytes: [u8; 6]) -> Self {
        Self { bytes }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(format!("Invalid MAC address format: {}", s));
        }
        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16)
                .map_err(|_| format!("Invalid hex in MAC: {}", part))?;
        }
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.bytes
    }
}

#[derive(Debug, Clone)]
pub struct NeighborEntry {
    pub key: NeighborKey,
    pub mac: MacAddress,
    pub neigh_oid: RawSaiObjectId,
    pub encap_index: u32,
}

impl NeighborEntry {
    pub fn new(key: NeighborKey, mac: MacAddress) -> Self {
        Self {
            key,
            mac,
            neigh_oid: 0,
            encap_index: 0,
        }
    }

    pub fn is_ipv4(&self) -> bool {
        matches!(self.key.ip, IpAddr::V4(_))
    }

    pub fn is_ipv6(&self) -> bool {
        matches!(self.key.ip, IpAddr::V6(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeighborType {
    Dynamic,
    Static,
}

#[derive(Debug, Clone)]
pub struct NeighborConfig {
    pub neigh_type: NeighborType,
    pub family: Option<String>,
}

impl Default for NeighborConfig {
    fn default() -> Self {
        Self {
            neigh_type: NeighborType::Dynamic,
            family: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NeighborStats {
    pub neighbors_added: u64,
    pub neighbors_removed: u64,
    pub neighbors_updated: u64,
    pub ipv4_neighbors: u64,
    pub ipv6_neighbors: u64,
}
