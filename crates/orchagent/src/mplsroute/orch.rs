//! MPLS route orchestration logic.

use super::types::{MplsRouteEntry, MplsRouteKey, MplsRouteStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum MplsRouteOrchError {
    RouteNotFound(MplsRouteKey),
    InvalidLabel(u32),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct MplsRouteOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct MplsRouteOrchStats {
    pub stats: MplsRouteStats,
    pub errors: u64,
}

pub trait MplsRouteOrchCallbacks: Send + Sync {}

pub struct MplsRouteOrch {
    config: MplsRouteOrchConfig,
    stats: MplsRouteOrchStats,
    routes: HashMap<MplsRouteKey, MplsRouteEntry>,
}

impl MplsRouteOrch {
    pub fn new(config: MplsRouteOrchConfig) -> Self {
        Self {
            config,
            stats: MplsRouteOrchStats::default(),
            routes: HashMap::new(),
        }
    }

    pub fn get_route(&self, key: &MplsRouteKey) -> Option<&MplsRouteEntry> {
        self.routes.get(key)
    }

    pub fn stats(&self) -> &MplsRouteOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{MplsAction, MplsRouteConfig};

    #[test]
    fn test_mpls_route_orch_new() {
        let orch = MplsRouteOrch::new(MplsRouteOrchConfig::default());
        assert_eq!(orch.routes.len(), 0);
        assert_eq!(orch.stats.stats.routes_created, 0);
        assert_eq!(orch.stats.stats.routes_removed, 0);
        assert_eq!(orch.stats.errors, 0);
    }

    #[test]
    fn test_get_route_not_found() {
        let orch = MplsRouteOrch::new(MplsRouteOrchConfig::default());
        let key = MplsRouteKey::new(100);

        let result = orch.get_route(&key);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_route_found() {
        let mut orch = MplsRouteOrch::new(MplsRouteOrchConfig::default());

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };
        let entry = MplsRouteEntry::new(key.clone(), config);

        orch.routes.insert(key.clone(), entry);

        let result = orch.get_route(&key);
        assert!(result.is_some());
        let route = result.unwrap();
        assert_eq!(route.key.label, 100);
        assert_eq!(route.config.action, MplsAction::Pop);
    }

    #[test]
    fn test_stats_returns_reference() {
        let orch = MplsRouteOrch::new(MplsRouteOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.errors, 0);
        assert_eq!(stats.stats.routes_created, 0);
        assert_eq!(stats.stats.routes_removed, 0);
    }

    #[test]
    fn test_mpls_route_orch_config_default() {
        let config = MplsRouteOrchConfig::default();
        let orch = MplsRouteOrch::new(config);

        assert_eq!(orch.routes.len(), 0);
    }

    #[test]
    fn test_multiple_mpls_routes() {
        let mut orch = MplsRouteOrch::new(MplsRouteOrchConfig::default());

        for i in 100..105 {
            let key = MplsRouteKey::new(i);
            let config = MplsRouteConfig {
                action: MplsAction::Pop,
                next_hop: Some(format!("10.0.0.{}", i - 99)),
                swap_label: None,
                push_labels: vec![],
            };
            let entry = MplsRouteEntry::new(key.clone(), config);
            orch.routes.insert(key, entry);
        }

        assert_eq!(orch.routes.len(), 5);

        for i in 100..105 {
            let key = MplsRouteKey::new(i);
            assert!(orch.get_route(&key).is_some());
        }
    }

    #[test]
    fn test_mpls_route_key_equality() {
        let key1 = MplsRouteKey::new(100);
        let key2 = MplsRouteKey::new(100);
        let key3 = MplsRouteKey::new(200);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_mpls_route_stats_structure() {
        let stats = MplsRouteOrchStats::default();

        assert_eq!(stats.stats.routes_created, 0);
        assert_eq!(stats.stats.routes_removed, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_mpls_route_entry_pop_action() {
        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        let entry = MplsRouteEntry::new(key, config);

        assert_eq!(entry.key.label, 100);
        assert_eq!(entry.config.action, MplsAction::Pop);
        assert_eq!(entry.config.next_hop, Some("10.0.0.1".to_string()));
        assert_eq!(entry.config.swap_label, None);
        assert_eq!(entry.config.push_labels.len(), 0);
        assert_eq!(entry.route_oid, 0);
        assert_eq!(entry.nh_oid, 0);
    }

    #[test]
    fn test_mpls_route_entry_swap_action() {
        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Swap,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: Some(200),
            push_labels: vec![],
        };

        let entry = MplsRouteEntry::new(key, config);

        assert_eq!(entry.config.action, MplsAction::Swap);
        assert_eq!(entry.config.swap_label, Some(200));
    }

    #[test]
    fn test_mpls_route_entry_push_action() {
        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Push,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![200, 300, 400],
        };

        let entry = MplsRouteEntry::new(key, config);

        assert_eq!(entry.config.action, MplsAction::Push);
        assert_eq!(entry.config.push_labels, vec![200, 300, 400]);
        assert_eq!(entry.config.push_labels.len(), 3);
    }

    #[test]
    fn test_mpls_label_validation_valid() {
        let key = MplsRouteKey::new(100);
        let result = key.validate_label();
        assert!(result.is_ok());

        let key = MplsRouteKey::new(1_048_575); // max valid label
        let result = key.validate_label();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mpls_label_validation_invalid() {
        let key = MplsRouteKey::new(1_048_576); // exceeds max
        let result = key.validate_label();
        assert!(result.is_err());

        let key = MplsRouteKey::new(2_000_000);
        let result = key.validate_label();
        assert!(result.is_err());
    }

    #[test]
    fn test_mpls_error_variants() {
        let err1 = MplsRouteOrchError::RouteNotFound(MplsRouteKey::new(100));
        let err2 = MplsRouteOrchError::InvalidLabel(999999);
        let err3 = MplsRouteOrchError::SaiError("test error".to_string());

        match err1 {
            MplsRouteOrchError::RouteNotFound(key) => {
                assert_eq!(key.label, 100);
            }
            _ => panic!("Wrong error variant"),
        }

        match err2 {
            MplsRouteOrchError::InvalidLabel(label) => {
                assert_eq!(label, 999999);
            }
            _ => panic!("Wrong error variant"),
        }

        match err3 {
            MplsRouteOrchError::SaiError(msg) => {
                assert_eq!(msg, "test error");
            }
            _ => panic!("Wrong error variant"),
        }
    }
}
