//! MPLS route orchestration logic.

use super::types::{MplsRouteConfig, MplsRouteEntry, MplsRouteKey, MplsRouteStats, RawSaiObjectId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, MplsRouteOrchError>;

#[derive(Debug, Clone, Error)]
pub enum MplsRouteOrchError {
    #[error("Route not found: {:?}", .0)]
    RouteNotFound(MplsRouteKey),
    #[error("Invalid label: {0}")]
    InvalidLabel(u32),
    #[error("Route exists: {:?}", .0)]
    RouteExists(MplsRouteKey),
    #[error("SAI error: {0}")]
    SaiError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

#[derive(Debug, Clone, Default)]
pub struct MplsRouteOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct MplsRouteOrchStats {
    pub stats: MplsRouteStats,
    pub errors: u64,
}

pub trait MplsRouteOrchCallbacks: Send + Sync {
    fn create_mpls_route(&self, label: u32, config: &MplsRouteConfig) -> Result<RawSaiObjectId>;
    fn remove_mpls_route(&self, label: u32, route_oid: RawSaiObjectId) -> Result<()>;
    fn update_mpls_route(
        &self,
        label: u32,
        route_oid: RawSaiObjectId,
        config: &MplsRouteConfig,
    ) -> Result<()>;
    fn create_next_hop(&self, ip_address: &str) -> Result<RawSaiObjectId>;
    fn remove_next_hop(&self, nh_oid: RawSaiObjectId) -> Result<()>;
    fn on_route_created(&self, label: u32, route_oid: RawSaiObjectId);
    fn on_route_removed(&self, label: u32);
}

pub struct MplsRouteOrch<C: MplsRouteOrchCallbacks> {
    _config: MplsRouteOrchConfig,
    stats: MplsRouteOrchStats,
    routes: HashMap<MplsRouteKey, MplsRouteEntry>,
    callbacks: Option<Arc<C>>,
}

impl<C: MplsRouteOrchCallbacks> MplsRouteOrch<C> {
    pub fn new(config: MplsRouteOrchConfig) -> Self {
        Self {
            _config: config,
            stats: MplsRouteOrchStats::default(),
            routes: HashMap::new(),
            callbacks: None,
        }
    }

    pub fn with_callbacks(mut self, callbacks: Arc<C>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    pub fn add_route(
        &mut self,
        key: MplsRouteKey,
        config: MplsRouteConfig,
    ) -> Result<RawSaiObjectId> {
        // Validate label
        key.validate_label()
            .map_err(|_| MplsRouteOrchError::InvalidLabel(key.label))?;

        if self.routes.contains_key(&key) {
            let audit_record = AuditRecord::new(
                AuditCategory::ResourceCreate,
                "MplsRouteOrch",
                "create_mpls_route",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&key.label.to_string())
            .with_object_type("mpls_route")
            .with_error("Route already exists");
            audit_log!(audit_record);
            return Err(MplsRouteOrchError::RouteExists(key));
        }

        let callbacks = self.callbacks.as_ref().ok_or(MplsRouteOrchError::SaiError(
            "No callbacks registered".into(),
        ))?;

        // Create the MPLS route
        let route_oid = callbacks.create_mpls_route(key.label, &config)?;

        // Create next hop if specified
        let mut nh_oid = 0;
        if let Some(next_hop) = &config.next_hop {
            nh_oid = callbacks.create_next_hop(next_hop)?;
        }

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "MplsRouteOrch",
            "create_mpls_route",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&key.label.to_string())
        .with_object_type("mpls_route")
        .with_details(serde_json::json!({
            "label": key.label,
            "route_oid": format!("0x{:x}", route_oid),
            "next_hop": config.next_hop.as_deref(),
            "nh_oid": if nh_oid != 0 { Some(format!("0x{:x}", nh_oid)) } else { None },
        }));
        audit_log!(audit_record);

        let mut entry = MplsRouteEntry::new(key.clone(), config);
        entry.route_oid = route_oid;
        entry.nh_oid = nh_oid;

        self.routes.insert(key.clone(), entry);
        self.stats.stats.routes_created += 1;

        callbacks.on_route_created(key.label, route_oid);

        Ok(route_oid)
    }

    pub fn remove_route(&mut self, key: &MplsRouteKey) -> Result<()> {
        let entry = self.routes.remove(key).ok_or_else(|| {
            let audit_record = AuditRecord::new(
                AuditCategory::ResourceDelete,
                "MplsRouteOrch",
                "remove_mpls_route",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&key.label.to_string())
            .with_object_type("mpls_route")
            .with_error("Route not found");
            audit_log!(audit_record);
            MplsRouteOrchError::RouteNotFound(key.clone())
        })?;

        let callbacks = self.callbacks.as_ref().ok_or(MplsRouteOrchError::SaiError(
            "No callbacks registered".into(),
        ))?;

        // Remove the next hop if it exists
        if entry.nh_oid != 0 {
            callbacks.remove_next_hop(entry.nh_oid)?;
        }

        // Remove the route
        callbacks.remove_mpls_route(key.label, entry.route_oid)?;

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "MplsRouteOrch",
            "remove_mpls_route",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&key.label.to_string())
        .with_object_type("mpls_route")
        .with_details(serde_json::json!({
            "label": key.label,
            "route_oid": format!("0x{:x}", entry.route_oid),
            "nh_oid_removed": entry.nh_oid,
        }));
        audit_log!(audit_record);

        self.stats.stats.routes_removed += 1;
        callbacks.on_route_removed(key.label);

        Ok(())
    }

    pub fn update_route(&mut self, key: &MplsRouteKey, config: MplsRouteConfig) -> Result<()> {
        // Validate label
        key.validate_label()
            .map_err(|_| MplsRouteOrchError::InvalidLabel(key.label))?;

        let entry = self
            .routes
            .get_mut(key)
            .ok_or_else(|| MplsRouteOrchError::RouteNotFound(key.clone()))?;

        let callbacks = self.callbacks.as_ref().ok_or(MplsRouteOrchError::SaiError(
            "No callbacks registered".into(),
        ))?;

        let old_next_hop = entry.config.next_hop.clone();

        // Update the route through SAI
        callbacks.update_mpls_route(key.label, entry.route_oid, &config)?;

        // Handle next hop changes
        if let Some(new_next_hop) = &config.next_hop {
            if entry.config.next_hop.as_ref() != Some(new_next_hop) {
                // Remove old next hop if it exists
                if entry.nh_oid != 0 {
                    callbacks.remove_next_hop(entry.nh_oid)?;
                }
                // Create new next hop
                let new_nh_oid = callbacks.create_next_hop(new_next_hop)?;
                entry.nh_oid = new_nh_oid;
            }
        } else if entry.nh_oid != 0 {
            // Remove next hop if it's being cleared
            callbacks.remove_next_hop(entry.nh_oid)?;
            entry.nh_oid = 0;
        }

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceModify,
            "MplsRouteOrch",
            "update_mpls_route",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&key.label.to_string())
        .with_object_type("mpls_route")
        .with_details(serde_json::json!({
            "label": key.label,
            "route_oid": format!("0x{:x}", entry.route_oid),
            "old_next_hop": old_next_hop,
            "new_next_hop": config.next_hop,
        }));
        audit_log!(audit_record);

        entry.config = config;

        Ok(())
    }

    pub fn get_route(&self, key: &MplsRouteKey) -> Option<&MplsRouteEntry> {
        self.routes.get(key)
    }

    pub fn get_route_mut(&mut self, key: &MplsRouteKey) -> Option<&mut MplsRouteEntry> {
        self.routes.get_mut(key)
    }

    pub fn get_all_routes(&self) -> Vec<(MplsRouteKey, &MplsRouteEntry)> {
        self.routes
            .iter()
            .map(|(k, v): (&MplsRouteKey, &MplsRouteEntry)| (k.clone(), v))
            .collect()
    }

    pub fn route_exists(&self, key: &MplsRouteKey) -> bool {
        self.routes.contains_key(key)
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn stats(&self) -> &MplsRouteOrchStats {
        &self.stats
    }

    pub fn stats_mut(&mut self) -> &mut MplsRouteOrchStats {
        &mut self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::MplsAction;
    use super::*;

    struct MockMplsCallbacks;

    impl MplsRouteOrchCallbacks for MockMplsCallbacks {
        fn create_mpls_route(
            &self,
            _label: u32,
            _config: &MplsRouteConfig,
        ) -> Result<RawSaiObjectId> {
            Ok(0x1000)
        }

        fn remove_mpls_route(&self, _label: u32, _route_oid: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn update_mpls_route(
            &self,
            _label: u32,
            _route_oid: RawSaiObjectId,
            _config: &MplsRouteConfig,
        ) -> Result<()> {
            Ok(())
        }

        fn create_next_hop(&self, _ip_address: &str) -> Result<RawSaiObjectId> {
            Ok(0x2000)
        }

        fn remove_next_hop(&self, _nh_oid: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn on_route_created(&self, _label: u32, _route_oid: RawSaiObjectId) {}
        fn on_route_removed(&self, _label: u32) {}
    }

    #[test]
    fn test_mpls_route_orch_new() {
        let orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default());
        assert_eq!(orch.route_count(), 0);
        assert_eq!(orch.stats().stats.routes_created, 0);
        assert_eq!(orch.stats().stats.routes_removed, 0);
        assert_eq!(orch.stats().errors, 0);
    }

    #[test]
    fn test_add_route_with_pop_action() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        let result = orch.add_route(key.clone(), config);
        assert!(result.is_ok());
        assert_eq!(orch.route_count(), 1);
        assert_eq!(orch.stats().stats.routes_created, 1);
        assert!(orch.get_route(&key).is_some());
    }

    #[test]
    fn test_add_route_with_swap_action() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(200);
        let config = MplsRouteConfig {
            action: MplsAction::Swap,
            next_hop: Some("10.0.0.2".to_string()),
            swap_label: Some(300),
            push_labels: vec![],
        };

        let result = orch.add_route(key.clone(), config);
        assert!(result.is_ok());
        let route = orch.get_route(&key).unwrap();
        assert_eq!(route.config.action, MplsAction::Swap);
        assert_eq!(route.config.swap_label, Some(300));
    }

    #[test]
    fn test_add_route_with_push_action() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(300);
        let config = MplsRouteConfig {
            action: MplsAction::Push,
            next_hop: Some("10.0.0.3".to_string()),
            swap_label: None,
            push_labels: vec![400, 500, 600],
        };

        let result = orch.add_route(key.clone(), config);
        assert!(result.is_ok());
        let route = orch.get_route(&key).unwrap();
        assert_eq!(route.config.action, MplsAction::Push);
        assert_eq!(route.config.push_labels.len(), 3);
    }

    #[test]
    fn test_add_route_invalid_label() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(2_000_000); // Invalid label
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        let result = orch.add_route(key, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_route_duplicate() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        assert!(orch.add_route(key.clone(), config.clone()).is_ok());
        let result = orch.add_route(key, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_route_without_callbacks() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default());

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        let result = orch.add_route(key, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_route() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        assert!(orch.add_route(key.clone(), config).is_ok());
        assert_eq!(orch.route_count(), 1);

        let result = orch.remove_route(&key);
        assert!(result.is_ok());
        assert_eq!(orch.route_count(), 0);
        assert_eq!(orch.stats().stats.routes_removed, 1);
    }

    #[test]
    fn test_remove_nonexistent_route() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let result = orch.remove_route(&key);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_route() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        assert!(orch.add_route(key.clone(), config).is_ok());

        let new_config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.2".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        let result = orch.update_route(&key, new_config.clone());
        assert!(result.is_ok());

        let route = orch.get_route(&key).unwrap();
        assert_eq!(route.config.next_hop, Some("10.0.0.2".to_string()));
    }

    #[test]
    fn test_update_nonexistent_route() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        let result = orch.update_route(&key, config);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_all_routes() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        for i in 100..105 {
            let key = MplsRouteKey::new(i);
            assert!(orch.add_route(key, config.clone()).is_ok());
        }

        let all_routes = orch.get_all_routes();
        assert_eq!(all_routes.len(), 5);
    }

    #[test]
    fn test_route_exists() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        assert!(!orch.route_exists(&key));
        assert!(orch.add_route(key.clone(), config).is_ok());
        assert!(orch.route_exists(&key));
    }

    #[test]
    fn test_route_count() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        assert_eq!(orch.route_count(), 0);

        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        for i in 100..105 {
            let key = MplsRouteKey::new(i);
            assert!(orch.add_route(key, config.clone()).is_ok());
        }

        assert_eq!(orch.route_count(), 5);
    }

    #[test]
    fn test_stats_structure() {
        let stats = MplsRouteOrchStats::default();
        assert_eq!(stats.stats.routes_created, 0);
        assert_eq!(stats.stats.routes_removed, 0);
        assert_eq!(stats.errors, 0);
    }

    #[test]
    fn test_multiple_route_operations_sequence() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        // Create multiple routes
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        let key1 = MplsRouteKey::new(100);
        let key2 = MplsRouteKey::new(200);
        let key3 = MplsRouteKey::new(300);

        assert!(orch.add_route(key1.clone(), config.clone()).is_ok());
        assert!(orch.add_route(key2.clone(), config.clone()).is_ok());
        assert!(orch.add_route(key3.clone(), config.clone()).is_ok());
        assert_eq!(orch.route_count(), 3);
        assert_eq!(orch.stats().stats.routes_created, 3);

        // Remove one route
        assert!(orch.remove_route(&key2).is_ok());
        assert_eq!(orch.route_count(), 2);
        assert_eq!(orch.stats().stats.routes_removed, 1);

        // Update remaining route
        let new_config = MplsRouteConfig {
            action: MplsAction::Swap,
            next_hop: Some("10.0.0.5".to_string()),
            swap_label: Some(400),
            push_labels: vec![],
        };
        assert!(orch.update_route(&key1, new_config).is_ok());

        // Verify final state
        assert!(orch.route_exists(&key1));
        assert!(!orch.route_exists(&key2));
        assert!(orch.route_exists(&key3));
    }

    #[test]
    fn test_error_variants() {
        let err1 = MplsRouteOrchError::RouteNotFound(MplsRouteKey::new(100));
        let err2 = MplsRouteOrchError::InvalidLabel(2_000_000);
        let err3 = MplsRouteOrchError::RouteExists(MplsRouteKey::new(100));
        let err4 = MplsRouteOrchError::SaiError("SAI error".to_string());
        let err5 = MplsRouteOrchError::ConfigurationError("Config error".to_string());

        assert!(matches!(err1, MplsRouteOrchError::RouteNotFound(_)));
        assert!(matches!(err2, MplsRouteOrchError::InvalidLabel(_)));
        assert!(matches!(err3, MplsRouteOrchError::RouteExists(_)));
        assert!(matches!(err4, MplsRouteOrchError::SaiError(_)));
        assert!(matches!(err5, MplsRouteOrchError::ConfigurationError(_)));
    }

    #[test]
    fn test_get_route_mut() {
        let mut orch: MplsRouteOrch<MockMplsCallbacks> =
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(Arc::new(MockMplsCallbacks));

        let key = MplsRouteKey::new(100);
        let config = MplsRouteConfig {
            action: MplsAction::Pop,
            next_hop: Some("10.0.0.1".to_string()),
            swap_label: None,
            push_labels: vec![],
        };

        assert!(orch.add_route(key.clone(), config).is_ok());

        if let Some(route) = orch.get_route_mut(&key) {
            // Just verify we can get mutable reference
            assert_eq!(route.key.label, 100);
        } else {
            panic!("Failed to get mutable route reference");
        }
    }
}
