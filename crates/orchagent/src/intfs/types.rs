//! Router interface types and structures.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpPrefix;
use std::collections::HashSet;

/// Router interface type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RifType {
    Port,
    Vlan,
    SubPort,
    Loopback,
}

/// Interface entry (stub).
#[derive(Debug, Clone, Default)]
pub struct IntfsEntry {
    pub ip_addresses: HashSet<IpPrefix>,
    pub ref_count: u32,
    pub vrf_id: RawSaiObjectId,
    pub proxy_arp: bool,
}

impl IntfsEntry {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_counting() {
        let mut entry = IntfsEntry::default();
        assert_eq!(entry.add_ref(), 1);
        assert_eq!(entry.remove_ref().unwrap(), 0);
        assert!(entry.remove_ref().is_err());
    }
}
