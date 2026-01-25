//! FDB (Forwarding Database) types.

use std::sync::atomic::AtomicU32;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FdbEntryType {
    Dynamic,
    Static,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FdbOrigin {
    Learned,
    Provisioned,
    Advertised,
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

    pub fn to_string(&self) -> String {
        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.bytes[0],
            self.bytes[1],
            self.bytes[2],
            self.bytes[3],
            self.bytes[4],
            self.bytes[5]
        )
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FdbKey {
    pub mac: MacAddress,
    pub vlan_id: u16,
}

impl FdbKey {
    pub fn new(mac: MacAddress, vlan_id: u16) -> Self {
        Self { mac, vlan_id }
    }
}

#[derive(Debug, Clone)]
pub struct FdbEntry {
    pub key: FdbKey,
    pub port_name: String,
    pub bridge_port_oid: RawSaiObjectId,
    pub entry_type: FdbEntryType,
    pub origin: FdbOrigin,
    pub remote_ip: Option<String>,
    pub esi: Option<String>,
    pub vni: Option<u32>,
}

impl FdbEntry {
    pub fn new(key: FdbKey, port_name: String) -> Self {
        Self {
            key,
            port_name,
            bridge_port_oid: 0,
            entry_type: FdbEntryType::Dynamic,
            origin: FdbOrigin::Learned,
            remote_ip: None,
            esi: None,
            vni: None,
        }
    }

    pub fn is_remote(&self) -> bool {
        self.remote_ip.is_some()
    }

    pub fn is_tunnel(&self) -> bool {
        self.vni.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct VlanMemberEntry {
    pub vlan_id: u16,
    pub port_name: String,
    pub tagging_mode: VlanTaggingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlanTaggingMode {
    Tagged,
    Untagged,
    PriorityTagged,
}

#[derive(Debug, Default)]
pub struct FdbFlushStats {
    pub port_flushes: AtomicU32,
    pub vlan_flushes: AtomicU32,
    pub total_entries_flushed: AtomicU32,
}

impl Clone for FdbFlushStats {
    fn clone(&self) -> Self {
        Self {
            port_flushes: AtomicU32::new(
                self.port_flushes.load(std::sync::atomic::Ordering::Relaxed),
            ),
            vlan_flushes: AtomicU32::new(
                self.vlan_flushes.load(std::sync::atomic::Ordering::Relaxed),
            ),
            total_entries_flushed: AtomicU32::new(
                self.total_entries_flushed
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}
