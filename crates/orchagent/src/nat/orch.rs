//! NAT orchestration logic.

use super::types::{NatEntry, NatEntryKey, NatPoolEntry, NatPoolKey, NatStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum NatOrchError {
    EntryNotFound(NatEntryKey),
    PoolNotFound(NatPoolKey),
    AclNotFound(String),
    InvalidIpRange(String),
    InvalidPortRange(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct NatOrchConfig {
    pub enable_hairpin: bool,
    pub tcp_timeout: u32,
    pub udp_timeout: u32,
}

impl NatOrchConfig {
    pub fn with_timeouts(mut self, tcp: u32, udp: u32) -> Self {
        self.tcp_timeout = tcp;
        self.udp_timeout = udp;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct NatOrchStats {
    pub stats: NatStats,
    pub errors: u64,
}

pub trait NatOrchCallbacks: Send + Sync {
    fn on_entry_created(&self, entry: &NatEntry);
    fn on_entry_removed(&self, key: &NatEntryKey);
    fn on_pool_created(&self, pool: &NatPoolEntry);
    fn on_pool_removed(&self, key: &NatPoolKey);
}

pub struct NatOrch {
    config: NatOrchConfig,
    stats: NatOrchStats,
    entries: HashMap<NatEntryKey, NatEntry>,
    pools: HashMap<NatPoolKey, NatPoolEntry>,
}

impl NatOrch {
    pub fn new(config: NatOrchConfig) -> Self {
        Self {
            config,
            stats: NatOrchStats::default(),
            entries: HashMap::new(),
            pools: HashMap::new(),
        }
    }

    pub fn get_entry(&self, key: &NatEntryKey) -> Option<&NatEntry> {
        self.entries.get(key)
    }

    pub fn add_entry(&mut self, entry: NatEntry) -> Result<(), NatOrchError> {
        let key = entry.key.clone();

        if self.entries.contains_key(&key) {
            return Err(NatOrchError::SaiError("NAT entry already exists".to_string()));
        }

        self.stats.stats.entries_created = self.stats.stats.entries_created.saturating_add(1);
        self.entries.insert(key, entry);

        Ok(())
    }

    pub fn remove_entry(&mut self, key: &NatEntryKey) -> Result<NatEntry, NatOrchError> {
        self.entries.remove(key)
            .ok_or_else(|| NatOrchError::EntryNotFound(key.clone()))
    }

    pub fn get_snat_entries(&self) -> Vec<&NatEntry> {
        self.entries
            .values()
            .filter(|entry| entry.is_snat())
            .collect()
    }

    pub fn get_dnat_entries(&self) -> Vec<&NatEntry> {
        self.entries
            .values()
            .filter(|entry| entry.is_dnat())
            .collect()
    }

    pub fn get_double_nat_entries(&self) -> Vec<&NatEntry> {
        self.entries
            .values()
            .filter(|entry| entry.is_double_nat())
            .collect()
    }

    pub fn get_pool(&self, key: &NatPoolKey) -> Option<&NatPoolEntry> {
        self.pools.get(key)
    }

    pub fn add_pool(&mut self, entry: NatPoolEntry) -> Result<(), NatOrchError> {
        let key = entry.key.clone();

        if self.pools.contains_key(&key) {
            return Err(NatOrchError::SaiError("NAT pool already exists".to_string()));
        }

        // Validate IP range
        let (start, end) = entry.config.ip_range;
        if start > end {
            return Err(NatOrchError::InvalidIpRange(
                format!("Start IP {} > End IP {}", start, end)
            ));
        }

        // Validate port range if present
        if let Some((start_port, end_port)) = entry.config.port_range {
            if start_port > end_port {
                return Err(NatOrchError::InvalidPortRange(
                    format!("Start port {} > End port {}", start_port, end_port)
                ));
            }
        }

        self.stats.stats.pools_created = self.stats.stats.pools_created.saturating_add(1);
        self.pools.insert(key, entry);

        Ok(())
    }

    pub fn remove_pool(&mut self, key: &NatPoolKey) -> Result<NatPoolEntry, NatOrchError> {
        self.pools.remove(key)
            .ok_or_else(|| NatOrchError::PoolNotFound(key.clone()))
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    pub fn stats(&self) -> &NatOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{NatEntryConfig, NatPoolConfig, NatType, NatProtocol};
    use std::net::Ipv4Addr;

    fn create_test_nat_entry(
        src_ip: &str,
        dst_ip: &str,
        nat_type: NatType,
        translated_src_ip: Option<&str>,
    ) -> NatEntry {
        let key = NatEntryKey::new(
            src_ip.parse().unwrap(),
            dst_ip.parse().unwrap(),
            NatProtocol::Tcp,
            1024,
            80,
        );
        let config = NatEntryConfig {
            nat_type,
            translated_src_ip: translated_src_ip.map(|ip| ip.parse().unwrap()),
            translated_dst_ip: None,
            translated_src_port: None,
            translated_dst_port: None,
        };
        NatEntry::new(key, config)
    }

    fn create_test_pool(
        pool_name: &str,
        start_ip: &str,
        end_ip: &str,
        port_range: Option<(u16, u16)>,
    ) -> NatPoolEntry {
        let key = NatPoolKey::new(pool_name.to_string());
        let config = NatPoolConfig {
            ip_range: (start_ip.parse().unwrap(), end_ip.parse().unwrap()),
            port_range,
        };
        NatPoolEntry::new(key, config)
    }

    #[test]
    fn test_add_entry() {
        let mut orch = NatOrch::new(NatOrchConfig::default());
        let entry = create_test_nat_entry("10.0.0.1", "192.168.1.1", NatType::Source, Some("1.1.1.1"));

        assert_eq!(orch.entry_count(), 0);
        orch.add_entry(entry.clone()).unwrap();
        assert_eq!(orch.entry_count(), 1);
        assert_eq!(orch.stats().stats.entries_created, 1);

        // Verify entry can be retrieved
        let retrieved = orch.get_entry(&entry.key).unwrap();
        assert_eq!(retrieved.key.src_ip, "10.0.0.1".parse::<Ipv4Addr>().unwrap());
    }

    #[test]
    fn test_add_duplicate_entry() {
        let mut orch = NatOrch::new(NatOrchConfig::default());
        let entry1 = create_test_nat_entry("10.0.0.1", "192.168.1.1", NatType::Source, Some("1.1.1.1"));
        let entry2 = create_test_nat_entry("10.0.0.1", "192.168.1.1", NatType::Source, Some("2.2.2.2"));

        orch.add_entry(entry1).unwrap();
        assert_eq!(orch.entry_count(), 1);

        // Adding duplicate should fail
        let result = orch.add_entry(entry2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NatOrchError::SaiError(_)));
        assert_eq!(orch.entry_count(), 1);
    }

    #[test]
    fn test_remove_entry() {
        let mut orch = NatOrch::new(NatOrchConfig::default());
        let entry = create_test_nat_entry("10.0.0.1", "192.168.1.1", NatType::Source, Some("1.1.1.1"));
        let key = entry.key.clone();

        orch.add_entry(entry).unwrap();
        assert_eq!(orch.entry_count(), 1);

        let removed = orch.remove_entry(&key).unwrap();
        assert_eq!(removed.key.src_ip, "10.0.0.1".parse::<Ipv4Addr>().unwrap());
        assert_eq!(orch.entry_count(), 0);

        // Removing again should fail
        let result = orch.remove_entry(&key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NatOrchError::EntryNotFound(_)));
    }

    #[test]
    fn test_get_snat_entries() {
        let mut orch = NatOrch::new(NatOrchConfig::default());

        // Add SNAT entries
        orch.add_entry(create_test_nat_entry("10.0.0.1", "192.168.1.1", NatType::Source, Some("1.1.1.1"))).unwrap();
        orch.add_entry(create_test_nat_entry("10.0.0.2", "192.168.1.2", NatType::Source, Some("1.1.1.2"))).unwrap();

        // Add DNAT entry
        orch.add_entry(create_test_nat_entry("10.0.0.3", "192.168.1.3", NatType::Destination, None)).unwrap();

        // Add Double NAT entry
        orch.add_entry(create_test_nat_entry("10.0.0.4", "192.168.1.4", NatType::DoubleNat, Some("1.1.1.4"))).unwrap();

        let snat_entries = orch.get_snat_entries();
        assert_eq!(snat_entries.len(), 2);

        // Verify all returned entries are SNAT
        for entry in snat_entries {
            assert!(entry.is_snat());
        }
    }

    #[test]
    fn test_get_dnat_entries() {
        let mut orch = NatOrch::new(NatOrchConfig::default());

        // Add SNAT entry
        orch.add_entry(create_test_nat_entry("10.0.0.1", "192.168.1.1", NatType::Source, Some("1.1.1.1"))).unwrap();

        // Add DNAT entries
        orch.add_entry(create_test_nat_entry("10.0.0.2", "192.168.1.2", NatType::Destination, None)).unwrap();
        orch.add_entry(create_test_nat_entry("10.0.0.3", "192.168.1.3", NatType::Destination, None)).unwrap();

        // Add Double NAT entry
        orch.add_entry(create_test_nat_entry("10.0.0.4", "192.168.1.4", NatType::DoubleNat, Some("1.1.1.4"))).unwrap();

        let dnat_entries = orch.get_dnat_entries();
        assert_eq!(dnat_entries.len(), 2);

        // Verify all returned entries are DNAT
        for entry in dnat_entries {
            assert!(entry.is_dnat());
        }
    }

    #[test]
    fn test_get_double_nat_entries() {
        let mut orch = NatOrch::new(NatOrchConfig::default());

        // Add SNAT entry
        orch.add_entry(create_test_nat_entry("10.0.0.1", "192.168.1.1", NatType::Source, Some("1.1.1.1"))).unwrap();

        // Add DNAT entry
        orch.add_entry(create_test_nat_entry("10.0.0.2", "192.168.1.2", NatType::Destination, None)).unwrap();

        // Add Double NAT entries
        orch.add_entry(create_test_nat_entry("10.0.0.3", "192.168.1.3", NatType::DoubleNat, Some("1.1.1.3"))).unwrap();
        orch.add_entry(create_test_nat_entry("10.0.0.4", "192.168.1.4", NatType::DoubleNat, Some("1.1.1.4"))).unwrap();
        orch.add_entry(create_test_nat_entry("10.0.0.5", "192.168.1.5", NatType::DoubleNat, Some("1.1.1.5"))).unwrap();

        let double_nat_entries = orch.get_double_nat_entries();
        assert_eq!(double_nat_entries.len(), 3);

        // Verify all returned entries are Double NAT
        for entry in double_nat_entries {
            assert!(entry.is_double_nat());
        }
    }

    #[test]
    fn test_add_pool() {
        let mut orch = NatOrch::new(NatOrchConfig::default());
        let pool = create_test_pool("pool1", "1.1.1.1", "1.1.1.10", Some((1024, 2048)));

        assert_eq!(orch.pool_count(), 0);
        orch.add_pool(pool.clone()).unwrap();
        assert_eq!(orch.pool_count(), 1);
        assert_eq!(orch.stats().stats.pools_created, 1);

        // Verify pool can be retrieved
        let retrieved = orch.get_pool(&pool.key).unwrap();
        assert_eq!(retrieved.config.ip_range.0, "1.1.1.1".parse::<Ipv4Addr>().unwrap());
        assert_eq!(retrieved.config.ip_range.1, "1.1.1.10".parse::<Ipv4Addr>().unwrap());
    }

    #[test]
    fn test_add_pool_invalid_ip_range() {
        let mut orch = NatOrch::new(NatOrchConfig::default());
        // Create pool with start IP > end IP
        let pool = create_test_pool("pool1", "1.1.1.10", "1.1.1.1", None);

        let result = orch.add_pool(pool);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NatOrchError::InvalidIpRange(_)));
        assert_eq!(orch.pool_count(), 0);
    }

    #[test]
    fn test_add_pool_invalid_port_range() {
        let mut orch = NatOrch::new(NatOrchConfig::default());
        // Create pool with start port > end port
        let pool = create_test_pool("pool1", "1.1.1.1", "1.1.1.10", Some((2048, 1024)));

        let result = orch.add_pool(pool);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NatOrchError::InvalidPortRange(_)));
        assert_eq!(orch.pool_count(), 0);
    }

    #[test]
    fn test_remove_pool() {
        let mut orch = NatOrch::new(NatOrchConfig::default());
        let pool = create_test_pool("pool1", "1.1.1.1", "1.1.1.10", Some((1024, 2048)));
        let key = pool.key.clone();

        orch.add_pool(pool).unwrap();
        assert_eq!(orch.pool_count(), 1);

        let removed = orch.remove_pool(&key).unwrap();
        assert_eq!(removed.key.pool_name, "pool1");
        assert_eq!(orch.pool_count(), 0);

        // Removing again should fail
        let result = orch.remove_pool(&key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NatOrchError::PoolNotFound(_)));
    }
}
