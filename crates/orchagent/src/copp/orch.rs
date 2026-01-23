//! CoPP orchestration logic.

use super::types::{CoppStats, CoppTrapEntry, CoppTrapKey};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum CoppOrchError {
    TrapNotFound(CoppTrapKey),
    InvalidQueue(u8),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct CoppOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct CoppOrchStats {
    pub stats: CoppStats,
    pub errors: u64,
}

pub trait CoppOrchCallbacks: Send + Sync {}

pub struct CoppOrch {
    config: CoppOrchConfig,
    stats: CoppOrchStats,
    traps: HashMap<CoppTrapKey, CoppTrapEntry>,
}

impl CoppOrch {
    pub fn new(config: CoppOrchConfig) -> Self {
        Self {
            config,
            stats: CoppOrchStats::default(),
            traps: HashMap::new(),
        }
    }

    pub fn get_trap(&self, key: &CoppTrapKey) -> Option<&CoppTrapEntry> {
        self.traps.get(key)
    }

    pub fn stats(&self) -> &CoppOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{CoppTrapAction, CoppTrapConfig};

    #[test]
    fn test_copp_orch_new() {
        let orch = CoppOrch::new(CoppOrchConfig::default());
        assert_eq!(orch.traps.len(), 0);
        assert_eq!(orch.stats.stats.traps_created, 0);
        assert_eq!(orch.stats.stats.trap_groups_created, 0);
        assert_eq!(orch.stats.stats.policers_created, 0);
        assert_eq!(orch.stats.errors, 0);
    }

    #[test]
    fn test_get_trap_not_found() {
        let orch = CoppOrch::new(CoppOrchConfig::default());
        let key = CoppTrapKey::new("bgp".to_string());

        let result = orch.get_trap(&key);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_trap_found() {
        let mut orch = CoppOrch::new(CoppOrchConfig::default());

        let key = CoppTrapKey::new("bgp".to_string());
        let config = CoppTrapConfig {
            trap_action: CoppTrapAction::Trap,
            trap_priority: Some(4),
            queue: Some(4),
            meter_type: Some("packets".to_string()),
            mode: Some("sr_tcm".to_string()),
            color: Some("aware".to_string()),
            cbs: Some(600),
            cir: Some(600),
            pbs: Some(600),
            pir: Some(600),
        };
        let entry = CoppTrapEntry::new(key.clone(), config);

        orch.traps.insert(key.clone(), entry);

        let result = orch.get_trap(&key);
        assert!(result.is_some());
        let trap = result.unwrap();
        assert_eq!(trap.key.trap_id, "bgp");
        assert_eq!(trap.config.trap_action, CoppTrapAction::Trap);
        assert_eq!(trap.config.queue, Some(4));
    }

    #[test]
    fn test_stats_returns_reference() {
        let orch = CoppOrch::new(CoppOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.errors, 0);
        assert_eq!(stats.stats.traps_created, 0);
        assert_eq!(stats.stats.trap_groups_created, 0);
        assert_eq!(stats.stats.policers_created, 0);
    }

    #[test]
    fn test_copp_orch_config_default() {
        let config = CoppOrchConfig::default();
        let orch = CoppOrch::new(config);

        assert_eq!(orch.traps.len(), 0);
    }

    #[test]
    fn test_multiple_trap_configurations() {
        let mut orch = CoppOrch::new(CoppOrchConfig::default());

        let trap_names = vec!["bgp", "arp", "lacp", "lldp", "dhcp"];

        for (i, name) in trap_names.iter().enumerate() {
            let key = CoppTrapKey::new(name.to_string());
            let config = CoppTrapConfig {
                trap_action: CoppTrapAction::Trap,
                trap_priority: Some(i as u32),
                queue: Some((i % 8) as u8),
                meter_type: Some("packets".to_string()),
                mode: Some("sr_tcm".to_string()),
                color: Some("aware".to_string()),
                cbs: Some(600),
                cir: Some(600),
                pbs: Some(600),
                pir: Some(600),
            };
            let entry = CoppTrapEntry::new(key.clone(), config);
            orch.traps.insert(key, entry);
        }

        assert_eq!(orch.traps.len(), 5);

        for name in trap_names {
            let key = CoppTrapKey::new(name.to_string());
            assert!(orch.get_trap(&key).is_some());
        }
    }

    #[test]
    fn test_trap_key_equality() {
        let key1 = CoppTrapKey::new("bgp".to_string());
        let key2 = CoppTrapKey::new("bgp".to_string());
        let key3 = CoppTrapKey::new("arp".to_string());

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_copp_stats_structure() {
        let stats = CoppOrchStats::default();

        assert_eq!(stats.stats.traps_created, 0);
        assert_eq!(stats.stats.trap_groups_created, 0);
        assert_eq!(stats.stats.policers_created, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_trap_entry_creation() {
        let key = CoppTrapKey::new("bgp".to_string());
        let config = CoppTrapConfig {
            trap_action: CoppTrapAction::Trap,
            trap_priority: Some(4),
            queue: Some(4),
            meter_type: Some("packets".to_string()),
            mode: Some("sr_tcm".to_string()),
            color: Some("aware".to_string()),
            cbs: Some(600),
            cir: Some(600),
            pbs: Some(600),
            pir: Some(600),
        };

        let entry = CoppTrapEntry::new(key, config);

        assert_eq!(entry.key.trap_id, "bgp");
        assert_eq!(entry.config.trap_action, CoppTrapAction::Trap);
        assert_eq!(entry.config.trap_priority, Some(4));
        assert_eq!(entry.config.queue, Some(4));
        assert_eq!(entry.trap_oid, 0);
        assert_eq!(entry.trap_group_oid, 0);
        assert_eq!(entry.policer_oid, 0);
    }

    #[test]
    fn test_trap_action_variants() {
        let actions = vec![
            CoppTrapAction::Drop,
            CoppTrapAction::Forward,
            CoppTrapAction::Copy,
            CoppTrapAction::CopyCancel,
            CoppTrapAction::Trap,
            CoppTrapAction::Log,
        ];

        for action in actions {
            let key = CoppTrapKey::new("test".to_string());
            let config = CoppTrapConfig {
                trap_action: action,
                trap_priority: None,
                queue: None,
                meter_type: None,
                mode: None,
                color: None,
                cbs: None,
                cir: None,
                pbs: None,
                pir: None,
            };

            let entry = CoppTrapEntry::new(key, config);
            assert_eq!(entry.config.trap_action, action);
        }
    }

    #[test]
    fn test_copp_error_variants() {
        let err1 = CoppOrchError::TrapNotFound(CoppTrapKey::new("bgp".to_string()));
        let err2 = CoppOrchError::InvalidQueue(10);
        let err3 = CoppOrchError::SaiError("test error".to_string());

        match err1 {
            CoppOrchError::TrapNotFound(key) => {
                assert_eq!(key.trap_id, "bgp");
            }
            _ => panic!("Wrong error variant"),
        }

        match err2 {
            CoppOrchError::InvalidQueue(q) => {
                assert_eq!(q, 10);
            }
            _ => panic!("Wrong error variant"),
        }

        match err3 {
            CoppOrchError::SaiError(msg) => {
                assert_eq!(msg, "test error");
            }
            _ => panic!("Wrong error variant"),
        }
    }
}
