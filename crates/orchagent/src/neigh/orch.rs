//! Neighbor orchestration logic.

use super::types::{NeighborEntry, NeighborKey, NeighborStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum NeighOrchError {
    NeighborNotFound(NeighborKey),
    InvalidMac(String),
    InvalidIp(String),
    InterfaceNotFound(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct NeighOrchConfig {
    pub enable_kernel_sync: bool,
    pub restore_neighbors: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NeighOrchStats {
    pub stats: NeighborStats,
    pub errors: u64,
}

pub trait NeighOrchCallbacks: Send + Sync {
    fn on_neighbor_added(&self, entry: &NeighborEntry);
    fn on_neighbor_removed(&self, key: &NeighborKey);
    fn on_neighbor_updated(&self, entry: &NeighborEntry);
}

pub struct NeighOrch {
    config: NeighOrchConfig,
    stats: NeighOrchStats,
    neighbors: HashMap<NeighborKey, NeighborEntry>,
}

impl NeighOrch {
    pub fn new(config: NeighOrchConfig) -> Self {
        Self {
            config,
            stats: NeighOrchStats::default(),
            neighbors: HashMap::new(),
        }
    }

    pub fn get_neighbor(&self, key: &NeighborKey) -> Option<&NeighborEntry> {
        self.neighbors.get(key)
    }

    pub fn add_neighbor(&mut self, entry: NeighborEntry) -> Result<(), NeighOrchError> {
        let key = entry.key.clone();

        if self.neighbors.contains_key(&key) {
            return self.update_neighbor(entry);
        }

        // Update stats based on IP version
        if entry.is_ipv4() {
            self.stats.stats.ipv4_neighbors = self.stats.stats.ipv4_neighbors.saturating_add(1);
        } else {
            self.stats.stats.ipv6_neighbors = self.stats.stats.ipv6_neighbors.saturating_add(1);
        }

        self.stats.stats.neighbors_added = self.stats.stats.neighbors_added.saturating_add(1);
        self.neighbors.insert(key, entry);

        Ok(())
    }

    pub fn remove_neighbor(&mut self, key: &NeighborKey) -> Result<NeighborEntry, NeighOrchError> {
        let entry = self.neighbors.remove(key)
            .ok_or_else(|| NeighOrchError::NeighborNotFound(key.clone()))?;

        // Update stats based on IP version
        if entry.is_ipv4() {
            self.stats.stats.ipv4_neighbors = self.stats.stats.ipv4_neighbors.saturating_sub(1);
        } else {
            self.stats.stats.ipv6_neighbors = self.stats.stats.ipv6_neighbors.saturating_sub(1);
        }

        self.stats.stats.neighbors_removed = self.stats.stats.neighbors_removed.saturating_add(1);

        Ok(entry)
    }

    pub fn update_neighbor(&mut self, entry: NeighborEntry) -> Result<(), NeighOrchError> {
        let key = entry.key.clone();

        if !self.neighbors.contains_key(&key) {
            return Err(NeighOrchError::NeighborNotFound(key));
        }

        self.stats.stats.neighbors_updated = self.stats.stats.neighbors_updated.saturating_add(1);
        self.neighbors.insert(key, entry);

        Ok(())
    }

    pub fn get_neighbors_by_interface(&self, interface: &str) -> Vec<&NeighborEntry> {
        self.neighbors
            .values()
            .filter(|entry| entry.key.interface == interface)
            .collect()
    }

    pub fn clear_interface(&mut self, interface: &str) -> usize {
        let keys_to_remove: Vec<_> = self.neighbors
            .keys()
            .filter(|key| key.interface == interface)
            .cloned()
            .collect();

        let count = keys_to_remove.len();
        for key in keys_to_remove {
            let _ = self.remove_neighbor(&key);
        }

        count
    }

    pub fn neighbor_count(&self) -> usize {
        self.neighbors.len()
    }

    pub fn stats(&self) -> &NeighOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::NeighborConfig;

    fn create_test_ipv4_neighbor(ip: &str, interface: &str, mac: &str) -> NeighborEntry {
        use super::super::types::MacAddress;
        let ip_addr: std::net::IpAddr = ip.parse().unwrap();
        let mac_addr = MacAddress::from_str(mac).unwrap();
        NeighborEntry::new(
            super::super::types::NeighborKey::new(interface.to_string(), ip_addr),
            mac_addr
        )
    }

    fn create_test_ipv6_neighbor(ip: &str, interface: &str, mac: &str) -> NeighborEntry {
        use super::super::types::MacAddress;
        let ip_addr: std::net::IpAddr = ip.parse().unwrap();
        let mac_addr = MacAddress::from_str(mac).unwrap();
        NeighborEntry::new(
            super::super::types::NeighborKey::new(interface.to_string(), ip_addr),
            mac_addr
        )
    }

    #[test]
    fn test_add_neighbor_ipv4_stats() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        let neighbor = create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55");

        assert_eq!(orch.stats().stats.ipv4_neighbors, 0);
        orch.add_neighbor(neighbor).unwrap();
        assert_eq!(orch.stats().stats.ipv4_neighbors, 1);
        assert_eq!(orch.stats().stats.neighbors_added, 1);
    }

    #[test]
    fn test_add_neighbor_ipv6_stats() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        let neighbor = create_test_ipv6_neighbor("fe80::1", "Ethernet0", "00:11:22:33:44:55");

        assert_eq!(orch.stats().stats.ipv6_neighbors, 0);
        orch.add_neighbor(neighbor).unwrap();
        assert_eq!(orch.stats().stats.ipv6_neighbors, 1);
        assert_eq!(orch.stats().stats.neighbors_added, 1);
    }

    #[test]
    fn test_add_duplicate_neighbor_updates() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        let neighbor1 = create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55");
        let neighbor2 = create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:66");

        orch.add_neighbor(neighbor1).unwrap();
        assert_eq!(orch.neighbor_count(), 1);
        assert_eq!(orch.stats().stats.neighbors_added, 1);

        // Adding duplicate should trigger update
        orch.add_neighbor(neighbor2).unwrap();
        assert_eq!(orch.neighbor_count(), 1);
        assert_eq!(orch.stats().stats.neighbors_updated, 1);

        // Verify MAC was updated
        let ip_addr: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        let key = super::super::types::NeighborKey::new("Ethernet0".to_string(), ip_addr);
        let entry = orch.get_neighbor(&key).unwrap();
        let expected_mac = super::super::types::MacAddress::from_str("00:11:22:33:44:66").unwrap();
        assert_eq!(entry.mac.as_bytes(), expected_mac.as_bytes());
    }

    #[test]
    fn test_remove_neighbor_not_found() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        let ip_addr: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        let key = super::super::types::NeighborKey::new("Ethernet0".to_string(), ip_addr);

        let result = orch.remove_neighbor(&key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NeighOrchError::NeighborNotFound(_)));
    }

    #[test]
    fn test_remove_neighbor_updates_stats() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        let neighbor = create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55");
        let key = neighbor.key.clone();

        orch.add_neighbor(neighbor).unwrap();
        assert_eq!(orch.stats().stats.ipv4_neighbors, 1);

        orch.remove_neighbor(&key).unwrap();
        assert_eq!(orch.stats().stats.ipv4_neighbors, 0);
        assert_eq!(orch.stats().stats.neighbors_removed, 1);
    }

    #[test]
    fn test_clear_interface() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55")).unwrap();
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.2", "Ethernet0", "00:11:22:33:44:56")).unwrap();
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.3", "Ethernet4", "00:11:22:33:44:57")).unwrap();

        assert_eq!(orch.neighbor_count(), 3);

        let removed = orch.clear_interface("Ethernet0");
        assert_eq!(removed, 2);
        assert_eq!(orch.neighbor_count(), 1);

        // Verify only Ethernet4 neighbor remains
        let neighbors = orch.get_neighbors_by_interface("Ethernet4");
        assert_eq!(neighbors.len(), 1);
    }

    #[test]
    fn test_get_neighbors_by_interface() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55")).unwrap();
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.2", "Ethernet0", "00:11:22:33:44:56")).unwrap();
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.3", "Ethernet4", "00:11:22:33:44:57")).unwrap();

        let eth0_neighbors = orch.get_neighbors_by_interface("Ethernet0");
        assert_eq!(eth0_neighbors.len(), 2);

        let eth4_neighbors = orch.get_neighbors_by_interface("Ethernet4");
        assert_eq!(eth4_neighbors.len(), 1);

        let eth8_neighbors = orch.get_neighbors_by_interface("Ethernet8");
        assert_eq!(eth8_neighbors.len(), 0);
    }

    #[test]
    fn test_update_neighbor_not_found() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        let neighbor = create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55");

        let result = orch.update_neighbor(neighbor);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NeighOrchError::NeighborNotFound(_)));
    }

    #[test]
    fn test_neighbor_count() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        assert_eq!(orch.neighbor_count(), 0);

        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55")).unwrap();
        assert_eq!(orch.neighbor_count(), 1);

        orch.add_neighbor(create_test_ipv6_neighbor("fe80::1", "Ethernet0", "00:11:22:33:44:56")).unwrap();
        assert_eq!(orch.neighbor_count(), 2);
    }

    #[test]
    fn test_mixed_ipv4_ipv6_stats() {
        let mut orch = NeighOrch::new(NeighOrchConfig::default());
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.1", "Ethernet0", "00:11:22:33:44:55")).unwrap();
        orch.add_neighbor(create_test_ipv4_neighbor("10.0.0.2", "Ethernet0", "00:11:22:33:44:56")).unwrap();
        orch.add_neighbor(create_test_ipv6_neighbor("fe80::1", "Ethernet0", "00:11:22:33:44:57")).unwrap();

        assert_eq!(orch.stats().stats.ipv4_neighbors, 2);
        assert_eq!(orch.stats().stats.ipv6_neighbors, 1);
        assert_eq!(orch.neighbor_count(), 3);
    }
}
