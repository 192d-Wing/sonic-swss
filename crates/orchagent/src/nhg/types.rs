//! Next Hop Group types and structures.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::{IpAddress, MacAddress};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

/// MPLS label stack.
pub type LabelStack = Vec<u32>;

/// Next hop key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NextHopKey {
    pub ip_address: IpAddress,
    pub alias: String,
    pub vni: u32,
    pub mac_address: Option<MacAddress>,
    pub label_stack: LabelStack,
    pub weight: u32,
    pub srv6_segment: Option<String>,
    pub srv6_source: Option<String>,
    pub srv6_vpn_sid: Option<String>,
}

impl NextHopKey {
    pub fn new(ip: IpAddress, alias: String) -> Self {
        Self {
            ip_address: ip,
            alias,
            vni: 0,
            mac_address: None,
            label_stack: Vec::new(),
            weight: 0,
            srv6_segment: None,
            srv6_source: None,
            srv6_vpn_sid: None,
        }
    }
}

/// Next hop group key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextHopGroupKey {
    pub nexthops: HashSet<NextHopKey>,
    pub overlay_nexthops: bool,
    pub srv6_nexthops: bool,
    pub srv6_vpn: bool,
}

impl NextHopGroupKey {
    pub fn new() -> Self {
        Self {
            nexthops: HashSet::new(),
            overlay_nexthops: false,
            srv6_nexthops: false,
            srv6_vpn: false,
        }
    }

    pub fn add_nexthop(&mut self, nh: NextHopKey) {
        self.nexthops.insert(nh);
    }

    pub fn is_empty(&self) -> bool {
        self.nexthops.is_empty()
    }

    pub fn size(&self) -> usize {
        self.nexthops.len()
    }
}

impl Default for NextHopGroupKey {
    fn default() -> Self {
        Self::new()
    }
}

/// Next hop group member.
#[derive(Debug, Clone)]
pub struct NextHopGroupMember {
    pub key: NextHopKey,
    pub gm_id: RawSaiObjectId,
    pub nh_id: RawSaiObjectId,
}

impl NextHopGroupMember {
    pub fn new(key: NextHopKey) -> Self {
        Self {
            key,
            gm_id: 0,
            nh_id: 0,
        }
    }

    pub fn is_synced(&self) -> bool {
        self.gm_id != 0
    }
}

/// Next hop group entry.
#[derive(Debug, Clone)]
pub struct NextHopGroupEntry {
    pub key: NextHopGroupKey,
    pub id: RawSaiObjectId,
    pub members: HashMap<NextHopKey, NextHopGroupMember>,
    pub is_temp: bool,
    pub is_recursive: bool,
}

impl NextHopGroupEntry {
    pub fn new(key: NextHopGroupKey) -> Self {
        Self {
            key,
            id: 0,
            members: HashMap::new(),
            is_temp: false,
            is_recursive: false,
        }
    }

    pub fn is_synced(&self) -> bool {
        self.id != 0
    }
}

/// NHG entry with reference counting.
#[derive(Debug, Clone)]
pub struct NhgEntry {
    pub nhg: NextHopGroupEntry,
    pub ref_count: u32,
}

impl NhgEntry {
    pub fn new(nhg: NextHopGroupEntry) -> Self {
        Self { nhg, ref_count: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nexthop_key() {
        let nh = NextHopKey::new(
            IpAddress::from_str("192.168.1.1").unwrap(),
            "Ethernet0".to_string(),
        );
        assert_eq!(nh.vni, 0);
        assert_eq!(nh.weight, 0);
    }

    #[test]
    fn test_nhg_key() {
        let mut key = NextHopGroupKey::new();
        assert!(key.is_empty());

        key.add_nexthop(NextHopKey::new(
            IpAddress::from_str("10.0.0.1").unwrap(),
            "Ethernet0".to_string(),
        ));

        assert_eq!(key.size(), 1);
    }

    #[test]
    fn test_nhg_entry() {
        let nhg = NextHopGroupEntry::new(NextHopGroupKey::new());
        assert!(!nhg.is_synced());
        assert!(!nhg.is_temp);
    }
}
