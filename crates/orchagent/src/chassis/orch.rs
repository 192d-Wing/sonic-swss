//! Chassis orchestration logic.

use super::types::{ChassisStats, SystemPortEntry, SystemPortKey};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum ChassisOrchError {
    SystemPortNotFound(SystemPortKey),
    InvalidSwitchId(u32),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct ChassisOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct ChassisOrchStats {
    pub stats: ChassisStats,
    pub errors: u64,
}

pub trait ChassisOrchCallbacks: Send + Sync {}

pub struct ChassisOrch {
    config: ChassisOrchConfig,
    stats: ChassisOrchStats,
    system_ports: HashMap<SystemPortKey, SystemPortEntry>,
}

impl ChassisOrch {
    pub fn new(config: ChassisOrchConfig) -> Self {
        Self {
            config,
            stats: ChassisOrchStats::default(),
            system_ports: HashMap::new(),
        }
    }

    pub fn get_system_port(&self, key: &SystemPortKey) -> Option<&SystemPortEntry> {
        self.system_ports.get(key)
    }

    pub fn stats(&self) -> &ChassisOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::SystemPortConfig;

    #[test]
    fn test_chassis_orch_new() {
        let orch = ChassisOrch::new(ChassisOrchConfig::default());
        assert_eq!(orch.system_ports.len(), 0);
        assert_eq!(orch.stats.stats.system_ports_created, 0);
        assert_eq!(orch.stats.stats.fabric_ports_created, 0);
        assert_eq!(orch.stats.errors, 0);
    }

    #[test]
    fn test_get_system_port_not_found() {
        let orch = ChassisOrch::new(ChassisOrchConfig::default());
        let key = SystemPortKey::new(100);

        let result = orch.get_system_port(&key);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_system_port_found() {
        let mut orch = ChassisOrch::new(ChassisOrchConfig::default());

        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 1,
            core_index: 0,
            core_port_index: 0,
            speed: 100000,
        };
        let entry = SystemPortEntry::new(config);
        let key = entry.key.clone();

        orch.system_ports.insert(key.clone(), entry);

        let result = orch.get_system_port(&key);
        assert!(result.is_some());
        let port = result.unwrap();
        assert_eq!(port.config.system_port_id, 100);
        assert_eq!(port.config.switch_id, 1);
    }

    #[test]
    fn test_stats_returns_reference() {
        let orch = ChassisOrch::new(ChassisOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.errors, 0);
        assert_eq!(stats.stats.system_ports_created, 0);
        assert_eq!(stats.stats.fabric_ports_created, 0);
    }

    #[test]
    fn test_chassis_orch_config_default() {
        let config = ChassisOrchConfig::default();
        let orch = ChassisOrch::new(config);

        assert_eq!(orch.system_ports.len(), 0);
    }

    #[test]
    fn test_multiple_system_ports() {
        let mut orch = ChassisOrch::new(ChassisOrchConfig::default());

        for i in 0..5 {
            let config = SystemPortConfig {
                system_port_id: 100 + i,
                switch_id: 1,
                core_index: i,
                core_port_index: i,
                speed: 100000,
            };
            let entry = SystemPortEntry::new(config);
            orch.system_ports.insert(entry.key.clone(), entry);
        }

        assert_eq!(orch.system_ports.len(), 5);

        for i in 0..5 {
            let key = SystemPortKey::new(100 + i);
            assert!(orch.get_system_port(&key).is_some());
        }
    }

    #[test]
    fn test_system_port_key_equality() {
        let key1 = SystemPortKey::new(100);
        let key2 = SystemPortKey::new(100);
        let key3 = SystemPortKey::new(200);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_chassis_stats_structure() {
        let stats = ChassisOrchStats::default();

        assert_eq!(stats.stats.system_ports_created, 0);
        assert_eq!(stats.stats.fabric_ports_created, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_system_port_entry_creation() {
        let config = SystemPortConfig {
            system_port_id: 100,
            switch_id: 2,
            core_index: 1,
            core_port_index: 5,
            speed: 400000,
        };

        let entry = SystemPortEntry::new(config);

        assert_eq!(entry.key.system_port_id, 100);
        assert_eq!(entry.config.system_port_id, 100);
        assert_eq!(entry.config.switch_id, 2);
        assert_eq!(entry.config.core_index, 1);
        assert_eq!(entry.config.core_port_index, 5);
        assert_eq!(entry.config.speed, 400000);
        assert_eq!(entry.sai_oid, 0);
    }

    #[test]
    fn test_chassis_error_variants() {
        let err1 = ChassisOrchError::SystemPortNotFound(SystemPortKey::new(100));
        let err2 = ChassisOrchError::InvalidSwitchId(99);
        let err3 = ChassisOrchError::SaiError("test error".to_string());

        match err1 {
            ChassisOrchError::SystemPortNotFound(key) => {
                assert_eq!(key.system_port_id, 100);
            }
            _ => panic!("Wrong error variant"),
        }

        match err2 {
            ChassisOrchError::InvalidSwitchId(id) => {
                assert_eq!(id, 99);
            }
            _ => panic!("Wrong error variant"),
        }

        match err3 {
            ChassisOrchError::SaiError(msg) => {
                assert_eq!(msg, "test error");
            }
            _ => panic!("Wrong error variant"),
        }
    }
}
