//! FDB orchestration logic.

use super::types::{FdbEntry, FdbFlushStats, FdbKey, RawSaiObjectId};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FdbOrchError {
    EntryNotFound(FdbKey),
    PortNotFound(String),
    VlanNotFound(u16),
    InvalidMacAddress(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct FdbOrchConfig {
    pub aging_time: u32,
    pub enable_flush_on_port_down: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FdbOrchStats {
    pub entries_added: u64,
    pub entries_removed: u64,
    pub entries_updated: u64,
    pub flush_stats: FdbFlushStats,
}

pub trait FdbOrchCallbacks: Send + Sync {
    fn on_fdb_entry_added(&self, entry: &FdbEntry);
    fn on_fdb_entry_removed(&self, key: &FdbKey);
    fn on_fdb_flush(&self, port: Option<&str>, vlan: Option<u16>);
}

pub struct FdbOrch {
    config: FdbOrchConfig,
    stats: FdbOrchStats,
    entries: HashMap<FdbKey, FdbEntry>,
    vlan_to_vlan_oid: HashMap<u16, RawSaiObjectId>,
}

impl FdbOrch {
    pub fn new(config: FdbOrchConfig) -> Self {
        Self {
            config,
            stats: FdbOrchStats::default(),
            entries: HashMap::new(),
            vlan_to_vlan_oid: HashMap::new(),
        }
    }

    pub fn get_entry(&self, key: &FdbKey) -> Option<&FdbEntry> {
        self.entries.get(key)
    }

    pub fn stats(&self) -> &FdbOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{MacAddress, FdbEntryType, FdbOrigin};

    #[test]
    fn test_new_fdb_orch_with_default_config() {
        let config = FdbOrchConfig::default();
        let orch = FdbOrch::new(config);

        assert_eq!(orch.stats().entries_added, 0);
        assert_eq!(orch.stats().entries_removed, 0);
        assert_eq!(orch.stats().entries_updated, 0);
    }

    #[test]
    fn test_new_fdb_orch_with_custom_config() {
        let config = FdbOrchConfig {
            aging_time: 300,
            enable_flush_on_port_down: true,
        };
        let orch = FdbOrch::new(config.clone());

        assert_eq!(orch.config.aging_time, 300);
        assert_eq!(orch.config.enable_flush_on_port_down, true);
        assert_eq!(orch.stats().entries_added, 0);
    }

    #[test]
    fn test_get_entry_returns_none_for_nonexistent_key() {
        let config = FdbOrchConfig::default();
        let orch = FdbOrch::new(config);
        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);

        assert!(orch.get_entry(&key).is_none());
    }

    #[test]
    fn test_fdb_orch_config_default() {
        let config = FdbOrchConfig::default();

        assert_eq!(config.aging_time, 0);
        assert_eq!(config.enable_flush_on_port_down, false);
    }

    #[test]
    fn test_fdb_orch_config_clone() {
        let config = FdbOrchConfig {
            aging_time: 600,
            enable_flush_on_port_down: true,
        };
        let cloned = config.clone();

        assert_eq!(cloned.aging_time, 600);
        assert_eq!(cloned.enable_flush_on_port_down, true);
    }

    #[test]
    fn test_fdb_orch_stats_default() {
        let stats = FdbOrchStats::default();

        assert_eq!(stats.entries_added, 0);
        assert_eq!(stats.entries_removed, 0);
        assert_eq!(stats.entries_updated, 0);
    }

    #[test]
    fn test_fdb_orch_stats_clone() {
        let stats = FdbOrchStats {
            entries_added: 10,
            entries_removed: 5,
            entries_updated: 3,
            flush_stats: Default::default(),
        };
        let cloned = stats.clone();

        assert_eq!(cloned.entries_added, 10);
        assert_eq!(cloned.entries_removed, 5);
        assert_eq!(cloned.entries_updated, 3);
    }

    #[test]
    fn test_fdb_orch_error_entry_not_found() {
        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let err = FdbOrchError::EntryNotFound(key);

        assert!(matches!(err, FdbOrchError::EntryNotFound(_)));
    }

    #[test]
    fn test_fdb_orch_error_port_not_found() {
        let err = FdbOrchError::PortNotFound("Ethernet0".to_string());

        assert!(matches!(err, FdbOrchError::PortNotFound(_)));
    }

    #[test]
    fn test_fdb_orch_error_vlan_not_found() {
        let err = FdbOrchError::VlanNotFound(100);

        assert!(matches!(err, FdbOrchError::VlanNotFound(_)));
    }

    #[test]
    fn test_fdb_orch_error_invalid_mac_address() {
        let err = FdbOrchError::InvalidMacAddress("invalid".to_string());

        assert!(matches!(err, FdbOrchError::InvalidMacAddress(_)));
    }

    #[test]
    fn test_fdb_orch_error_sai_error() {
        let err = FdbOrchError::SaiError("SAI error".to_string());

        assert!(matches!(err, FdbOrchError::SaiError(_)));
    }

    #[test]
    fn test_fdb_orch_error_clone() {
        let err = FdbOrchError::PortNotFound("Ethernet1".to_string());
        let cloned = err.clone();

        assert!(matches!(cloned, FdbOrchError::PortNotFound(_)));
    }

    #[test]
    fn test_multiple_fdb_orch_instances() {
        let config1 = FdbOrchConfig {
            aging_time: 300,
            enable_flush_on_port_down: true,
        };
        let config2 = FdbOrchConfig::default();

        let orch1 = FdbOrch::new(config1);
        let orch2 = FdbOrch::new(config2);

        assert_eq!(orch1.config.aging_time, 300);
        assert_eq!(orch2.config.aging_time, 0);
    }
}
