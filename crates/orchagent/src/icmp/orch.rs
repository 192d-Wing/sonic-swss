//! ICMP echo orchestration logic.

use super::types::{IcmpEchoEntry, IcmpEchoKey, IcmpStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum IcmpOrchError {
    EntryNotFound(IcmpEchoKey),
}

#[derive(Debug, Clone, Default)]
pub struct IcmpOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct IcmpOrchStats {
    pub stats: IcmpStats,
}

pub trait IcmpOrchCallbacks: Send + Sync {}

pub struct IcmpOrch {
    config: IcmpOrchConfig,
    stats: IcmpOrchStats,
    entries: HashMap<IcmpEchoKey, IcmpEchoEntry>,
}

impl IcmpOrch {
    pub fn new(config: IcmpOrchConfig) -> Self {
        Self {
            config,
            stats: IcmpOrchStats::default(),
            entries: HashMap::new(),
        }
    }

    pub fn get_entry(&self, key: &IcmpEchoKey) -> Option<&IcmpEchoEntry> {
        self.entries.get(key)
    }

    pub fn stats(&self) -> &IcmpOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_icmp_orch_new() {
        let config = IcmpOrchConfig::default();
        let orch = IcmpOrch::new(config);

        assert_eq!(orch.entries.len(), 0);
        assert_eq!(orch.stats.stats.entries_added, 0);
        assert_eq!(orch.stats.stats.entries_removed, 0);
    }

    #[test]
    fn test_icmp_orch_new_with_default_config() {
        let orch = IcmpOrch::new(IcmpOrchConfig::default());

        // Verify initial state
        assert_eq!(orch.entries.len(), 0);
        let stats = orch.stats();
        assert_eq!(stats.stats.entries_added, 0);
        assert_eq!(stats.stats.entries_removed, 0);
    }

    #[test]
    fn test_get_entry_empty_orch() {
        let orch = IcmpOrch::new(IcmpOrchConfig::default());

        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );

        assert!(orch.get_entry(&key).is_none());
    }

    #[test]
    fn test_icmp_echo_key_creation_ipv4() {
        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
        );

        assert_eq!(key.vrf_name, "default");
        assert_eq!(key.ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
    }

    #[test]
    fn test_icmp_echo_key_creation_ipv6() {
        let key = IcmpEchoKey::new(
            "Vrf-RED".to_string(),
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        );

        assert_eq!(key.vrf_name, "Vrf-RED");
        assert_eq!(key.ip, IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)));
    }

    #[test]
    fn test_icmp_echo_key_equality() {
        let key1 = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let key2 = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let key3 = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
        );

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_icmp_stats_default() {
        let stats = IcmpOrchStats::default();

        assert_eq!(stats.stats.entries_added, 0);
        assert_eq!(stats.stats.entries_removed, 0);
    }

    #[test]
    fn test_icmp_orch_config_clone() {
        let config1 = IcmpOrchConfig::default();
        let config2 = config1.clone();

        // Both configs should be valid (no panic)
        let _orch1 = IcmpOrch::new(config1);
        let _orch2 = IcmpOrch::new(config2);
    }

    #[test]
    fn test_icmp_orch_error_entry_not_found() {
        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let error = IcmpOrchError::EntryNotFound(key.clone());

        match error {
            IcmpOrchError::EntryNotFound(k) => {
                assert_eq!(k.vrf_name, "default");
                assert_eq!(k.ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
            }
        }
    }

    #[test]
    fn test_icmp_orch_error_clone() {
        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let error1 = IcmpOrchError::EntryNotFound(key);
        let error2 = error1.clone();

        // Verify both errors are identical
        match (error1, error2) {
            (IcmpOrchError::EntryNotFound(k1), IcmpOrchError::EntryNotFound(k2)) => {
                assert_eq!(k1, k2);
            }
        }
    }
}
