//! ICMP echo orchestration logic.

use super::types::{
    IcmpEchoEntry, IcmpEchoKey, IcmpRedirectConfig, IcmpStats, NeighborDiscoveryConfig,
};
use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::{audit_log, debug_log, error_log, info_log, warn_log};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, IcmpOrchError>;

/// ICMP orchestration errors with NIST-compliant error messages.
#[derive(Debug, Clone, Error)]
pub enum IcmpOrchError {
    /// ICMP echo entry not found
    #[error("ICMP echo entry not found: {0:?}")]
    EntryNotFound(IcmpEchoKey),
    /// Invalid ICMP configuration
    #[error("Invalid ICMP configuration: {0}")]
    InvalidConfig(String),
    /// SAI operation failed
    #[error("SAI operation failed: {0}")]
    SaiError(String),
    /// ICMP operation failed
    #[error("ICMP operation failed: {0}")]
    OperationFailed(String),
    /// Callbacks not configured
    #[error("ICMP orchestrator not initialized: callbacks not configured")]
    NotInitialized,
}

#[derive(Debug, Clone, Default)]
pub struct IcmpOrchConfig {
    pub enable_redirects: bool,
    pub enable_neighbor_discovery: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IcmpOrchStats {
    pub stats: IcmpStats,
    pub redirects_processed: u64,
    pub nd_solicitations_processed: u64,
}

pub trait IcmpOrchCallbacks: Send + Sync {
    fn configure_icmp_redirect(&self, config: &IcmpRedirectConfig) -> Result<()>;
    fn configure_neighbor_discovery(&self, config: &NeighborDiscoveryConfig) -> Result<()>;
    fn process_redirect(&self, src_ip: &str, dst_ip: &str, gateway_ip: &str) -> Result<()>;
    fn get_icmp_statistics(&self) -> Result<IcmpStats>;
    fn on_redirect_processed(&self, src_ip: &str);
    fn on_neighbor_discovery_complete(&self, neighbor_ip: &str);
}

pub struct IcmpOrch<C: IcmpOrchCallbacks> {
    config: IcmpOrchConfig,
    stats: IcmpOrchStats,
    entries: HashMap<IcmpEchoKey, IcmpEchoEntry>,
    redirect_config: Option<IcmpRedirectConfig>,
    nd_config: Option<NeighborDiscoveryConfig>,
    callbacks: Option<Arc<C>>,
}

impl<C: IcmpOrchCallbacks> IcmpOrch<C> {
    pub fn new(config: IcmpOrchConfig) -> Self {
        Self {
            config,
            stats: IcmpOrchStats::default(),
            entries: HashMap::new(),
            redirect_config: None,
            nd_config: None,
            callbacks: None,
        }
    }

    pub fn with_callbacks(mut self, callbacks: Arc<C>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    pub fn get_entry(&self, key: &IcmpEchoKey) -> Option<&IcmpEchoEntry> {
        self.entries.get(key)
    }

    pub fn add_entry(&mut self, key: IcmpEchoKey, entry: IcmpEchoEntry) -> Result<()> {
        debug_log!("IcmpOrch", vrf = %key.vrf_name, ip = %key.ip, "Adding ICMP echo entry");

        if self.entries.contains_key(&key) {
            warn_log!("IcmpOrch", vrf = %key.vrf_name, ip = %key.ip, "Entry already exists");
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceCreate, "IcmpOrch", "add_entry")
                    .with_object_id(format!("{}", key.ip))
                    .with_object_type("icmp_entry")
                    .with_error(format!("Entry already exists: {:?}", key))
            );
            return Err(IcmpOrchError::OperationFailed(format!(
                "Entry already exists for key: {:?}",
                key
            )));
        }
        self.entries.insert(key.clone(), entry);
        self.stats.stats.entries_added += 1;

        info_log!("IcmpOrch", vrf = %key.vrf_name, ip = %key.ip, "ICMP echo entry created successfully");
        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "IcmpOrch", "add_entry")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(format!("{}", key.ip))
                .with_object_type("icmp_entry")
                .with_details(serde_json::json!({
                    "vrf_name": key.vrf_name,
                    "ip_address": format!("{}", key.ip)
                }))
        );
        Ok(())
    }

    pub fn remove_entry(&mut self, key: &IcmpEchoKey) -> Result<()> {
        debug_log!("IcmpOrch", vrf = %key.vrf_name, ip = %key.ip, "Removing ICMP echo entry");

        if self.entries.remove(key).is_none() {
            warn_log!("IcmpOrch", vrf = %key.vrf_name, ip = %key.ip, "Entry not found for removal");
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceDelete, "IcmpOrch", "remove_entry")
                    .with_object_id(format!("{}", key.ip))
                    .with_object_type("icmp_entry")
                    .with_error("Entry not found")
            );
            return Err(IcmpOrchError::EntryNotFound(key.clone()));
        }
        self.stats.stats.entries_removed += 1;

        info_log!("IcmpOrch", vrf = %key.vrf_name, ip = %key.ip, "ICMP echo entry removed successfully");
        audit_log!(
            AuditRecord::new(AuditCategory::ResourceDelete, "IcmpOrch", "remove_entry")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(format!("{}", key.ip))
                .with_object_type("icmp_entry")
                .with_details(serde_json::json!({
                    "vrf_name": key.vrf_name,
                    "ip_address": format!("{}", key.ip)
                }))
        );
        Ok(())
    }

    pub fn configure_redirect(&mut self, config: IcmpRedirectConfig) -> Result<()> {
        debug_log!(
            "IcmpOrch",
            enabled = config.enabled,
            hop_limit = config.hop_limit,
            "Configuring ICMP redirect"
        );

        if !self.config.enable_redirects {
            error_log!("IcmpOrch", "ICMP redirects not enabled in configuration");
            audit_log!(AuditRecord::new(
                AuditCategory::ConfigurationChange,
                "IcmpOrch",
                "configure_redirect"
            )
            .with_object_type("icmp_redirect")
            .with_error("ICMP redirects not enabled"));
            return Err(IcmpOrchError::InvalidConfig(
                "ICMP redirects not enabled".to_string(),
            ));
        }

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("IcmpOrch", "Callbacks not configured");
            IcmpOrchError::NotInitialized
        })?;
        callbacks.configure_icmp_redirect(&config).map_err(|e| {
            error_log!("IcmpOrch", error = %e, "Failed to configure ICMP redirect");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "IcmpOrch",
                "configure_icmp_redirect"
            )
            .with_object_type("icmp_redirect")
            .with_error(e.to_string()));
            e
        })?;

        self.redirect_config = Some(config.clone());

        info_log!(
            "IcmpOrch",
            enabled = config.enabled,
            hop_limit = config.hop_limit,
            "ICMP redirect configured successfully"
        );
        audit_log!(AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "IcmpOrch",
            "configure_redirect"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_type("icmp_redirect")
        .with_details(serde_json::json!({
            "enabled": config.enabled,
            "hop_limit": config.hop_limit
        })));
        Ok(())
    }

    pub fn configure_neighbor_discovery(&mut self, config: NeighborDiscoveryConfig) -> Result<()> {
        debug_log!(
            "IcmpOrch",
            enabled = config.enabled,
            delay = config.max_solicitation_delay,
            "Configuring neighbor discovery"
        );

        if !self.config.enable_neighbor_discovery {
            error_log!(
                "IcmpOrch",
                "Neighbor discovery not enabled in configuration"
            );
            audit_log!(AuditRecord::new(
                AuditCategory::ConfigurationChange,
                "IcmpOrch",
                "configure_neighbor_discovery"
            )
            .with_object_type("neighbor_discovery")
            .with_error("Neighbor discovery not enabled"));
            return Err(IcmpOrchError::InvalidConfig(
                "Neighbor discovery not enabled".to_string(),
            ));
        }

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("IcmpOrch", "Callbacks not configured");
            IcmpOrchError::NotInitialized
        })?;
        callbacks
            .configure_neighbor_discovery(&config)
            .map_err(|e| {
                error_log!("IcmpOrch", error = %e, "Failed to configure neighbor discovery");
                audit_log!(AuditRecord::new(
                    AuditCategory::SaiOperation,
                    "IcmpOrch",
                    "configure_neighbor_discovery"
                )
                .with_object_type("neighbor_discovery")
                .with_error(e.to_string()));
                e
            })?;

        self.nd_config = Some(config.clone());

        info_log!(
            "IcmpOrch",
            enabled = config.enabled,
            delay = config.max_solicitation_delay,
            "Neighbor discovery configured successfully"
        );
        audit_log!(AuditRecord::new(
            AuditCategory::ConfigurationChange,
            "IcmpOrch",
            "configure_neighbor_discovery"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_type("neighbor_discovery")
        .with_details(serde_json::json!({
            "enabled": config.enabled,
            "max_solicitation_delay": config.max_solicitation_delay
        })));
        Ok(())
    }

    pub fn process_icmp_redirect(
        &mut self,
        src_ip: &str,
        dst_ip: &str,
        gateway_ip: &str,
    ) -> Result<()> {
        debug_log!("IcmpOrch", src = %src_ip, dst = %dst_ip, gateway = %gateway_ip, "Processing ICMP redirect");

        if self.redirect_config.is_none() {
            error_log!("IcmpOrch", "Redirect not configured");
            audit_log!(AuditRecord::new(
                AuditCategory::NetworkConfig,
                "IcmpOrch",
                "process_redirect"
            )
            .with_object_id(src_ip)
            .with_object_type("icmp_redirect")
            .with_error("Redirect not configured"));
            return Err(IcmpOrchError::InvalidConfig(
                "Redirect not configured".to_string(),
            ));
        }

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("IcmpOrch", "Callbacks not configured");
            IcmpOrchError::NotInitialized
        })?;
        callbacks.process_redirect(src_ip, dst_ip, gateway_ip).map_err(|e| {
            error_log!("IcmpOrch", src = %src_ip, dst = %dst_ip, error = %e, "Failed to process redirect");
            audit_log!(AuditRecord::new(
                AuditCategory::NetworkConfig,
                "IcmpOrch",
                "process_redirect"
            )
            .with_object_id(src_ip)
            .with_object_type("icmp_redirect")
            .with_error(e.to_string()));
            e
        })?;

        self.stats.redirects_processed += 1;
        callbacks.on_redirect_processed(src_ip);

        info_log!("IcmpOrch", src = %src_ip, dst = %dst_ip, gateway = %gateway_ip, "ICMP redirect processed successfully");
        audit_log!(
            AuditRecord::new(AuditCategory::NetworkConfig, "IcmpOrch", "process_redirect")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(src_ip)
                .with_object_type("icmp_redirect")
                .with_details(serde_json::json!({
                    "source_ip": src_ip,
                    "destination_ip": dst_ip,
                    "gateway_ip": gateway_ip
                }))
        );

        Ok(())
    }

    pub fn process_neighbor_discovery(&mut self, neighbor_ip: &str) -> Result<()> {
        debug_log!("IcmpOrch", neighbor = %neighbor_ip, "Processing neighbor discovery");

        if self.nd_config.is_none() {
            error_log!("IcmpOrch", "Neighbor discovery not configured");
            audit_log!(AuditRecord::new(
                AuditCategory::NetworkConfig,
                "IcmpOrch",
                "process_neighbor_discovery"
            )
            .with_object_id(neighbor_ip)
            .with_object_type("neighbor_discovery")
            .with_error("Neighbor discovery not configured"));
            return Err(IcmpOrchError::InvalidConfig(
                "Neighbor discovery not configured".to_string(),
            ));
        }

        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            error_log!("IcmpOrch", "Callbacks not configured");
            IcmpOrchError::NotInitialized
        })?;

        self.stats.nd_solicitations_processed += 1;
        callbacks.on_neighbor_discovery_complete(neighbor_ip);

        info_log!("IcmpOrch", neighbor = %neighbor_ip, "Neighbor discovery completed successfully");
        audit_log!(AuditRecord::new(
            AuditCategory::NetworkConfig,
            "IcmpOrch",
            "process_neighbor_discovery"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(neighbor_ip)
        .with_object_type("neighbor_discovery")
        .with_details(serde_json::json!({
            "neighbor_ip": neighbor_ip
        })));

        Ok(())
    }

    pub fn get_redirect_config(&self) -> Option<&IcmpRedirectConfig> {
        self.redirect_config.as_ref()
    }

    pub fn get_nd_config(&self) -> Option<&NeighborDiscoveryConfig> {
        self.nd_config.as_ref()
    }

    pub fn stats(&self) -> &IcmpOrchStats {
        &self.stats
    }

    pub fn stats_mut(&mut self) -> &mut IcmpOrchStats {
        &mut self.stats
    }

    pub fn get_entry_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    struct MockIcmpCallbacks;

    impl IcmpOrchCallbacks for MockIcmpCallbacks {
        fn configure_icmp_redirect(&self, _config: &IcmpRedirectConfig) -> Result<()> {
            Ok(())
        }

        fn configure_neighbor_discovery(&self, _config: &NeighborDiscoveryConfig) -> Result<()> {
            Ok(())
        }

        fn process_redirect(&self, _src_ip: &str, _dst_ip: &str, _gateway_ip: &str) -> Result<()> {
            Ok(())
        }

        fn get_icmp_statistics(&self) -> Result<IcmpStats> {
            Ok(IcmpStats::default())
        }

        fn on_redirect_processed(&self, _src_ip: &str) {}
        fn on_neighbor_discovery_complete(&self, _neighbor_ip: &str) {}
    }

    #[test]
    fn test_icmp_orch_new() {
        let config = IcmpOrchConfig::default();
        let orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(config);

        assert_eq!(orch.get_entry_count(), 0);
        assert_eq!(orch.stats().stats.entries_added, 0);
        assert_eq!(orch.stats().stats.entries_removed, 0);
    }

    #[test]
    fn test_icmp_orch_new_with_default_config() {
        let orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(IcmpOrchConfig::default());

        assert_eq!(orch.get_entry_count(), 0);
        let stats = orch.stats();
        assert_eq!(stats.stats.entries_added, 0);
        assert_eq!(stats.stats.entries_removed, 0);
    }

    #[test]
    fn test_get_entry_empty_orch() {
        let orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(IcmpOrchConfig::default());

        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );

        assert!(orch.get_entry(&key).is_none());
    }

    #[test]
    fn test_add_entry() {
        let mut orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(IcmpOrchConfig::default());

        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let entry = IcmpEchoEntry::new(key.clone(), super::super::types::IcmpMode::Enabled);

        assert!(orch.add_entry(key.clone(), entry).is_ok());
        assert_eq!(orch.get_entry_count(), 1);
        assert_eq!(orch.stats().stats.entries_added, 1);
        assert!(orch.get_entry(&key).is_some());
    }

    #[test]
    fn test_add_duplicate_entry() {
        let mut orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(IcmpOrchConfig::default());

        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let entry = IcmpEchoEntry::new(key.clone(), super::super::types::IcmpMode::Enabled);

        assert!(orch.add_entry(key.clone(), entry.clone()).is_ok());
        assert!(orch.add_entry(key, entry).is_err());
    }

    #[test]
    fn test_remove_entry() {
        let mut orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(IcmpOrchConfig::default());

        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );
        let entry = IcmpEchoEntry::new(key.clone(), super::super::types::IcmpMode::Enabled);

        assert!(orch.add_entry(key.clone(), entry).is_ok());
        assert_eq!(orch.get_entry_count(), 1);

        assert!(orch.remove_entry(&key).is_ok());
        assert_eq!(orch.get_entry_count(), 0);
        assert_eq!(orch.stats().stats.entries_removed, 1);
    }

    #[test]
    fn test_remove_nonexistent_entry() {
        let mut orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(IcmpOrchConfig::default());

        let key = IcmpEchoKey::new(
            "default".to_string(),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );

        assert!(orch.remove_entry(&key).is_err());
    }

    #[test]
    fn test_configure_redirect() {
        let config = IcmpOrchConfig {
            enable_redirects: true,
            enable_neighbor_discovery: false,
        };
        let mut orch: IcmpOrch<MockIcmpCallbacks> =
            IcmpOrch::new(config).with_callbacks(Arc::new(MockIcmpCallbacks));

        let redirect_config = IcmpRedirectConfig {
            enabled: true,
            hop_limit: 64,
        };

        assert!(orch.configure_redirect(redirect_config.clone()).is_ok());
        assert!(orch.get_redirect_config().is_some());
        assert_eq!(orch.get_redirect_config().unwrap().hop_limit, 64);
    }

    #[test]
    fn test_configure_redirect_not_enabled() {
        let config = IcmpOrchConfig {
            enable_redirects: false,
            enable_neighbor_discovery: false,
        };
        let mut orch: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(config);

        let redirect_config = IcmpRedirectConfig {
            enabled: true,
            hop_limit: 64,
        };

        assert!(orch.configure_redirect(redirect_config).is_err());
    }

    #[test]
    fn test_configure_neighbor_discovery() {
        let config = IcmpOrchConfig {
            enable_redirects: false,
            enable_neighbor_discovery: true,
        };
        let mut orch: IcmpOrch<MockIcmpCallbacks> =
            IcmpOrch::new(config).with_callbacks(Arc::new(MockIcmpCallbacks));

        let nd_config = NeighborDiscoveryConfig {
            enabled: true,
            max_solicitation_delay: 1000,
        };

        assert!(orch.configure_neighbor_discovery(nd_config.clone()).is_ok());
        assert!(orch.get_nd_config().is_some());
        assert_eq!(orch.get_nd_config().unwrap().max_solicitation_delay, 1000);
    }

    #[test]
    fn test_process_icmp_redirect() {
        let config = IcmpOrchConfig {
            enable_redirects: true,
            enable_neighbor_discovery: false,
        };
        let mut orch: IcmpOrch<MockIcmpCallbacks> =
            IcmpOrch::new(config).with_callbacks(Arc::new(MockIcmpCallbacks));

        let redirect_config = IcmpRedirectConfig {
            enabled: true,
            hop_limit: 64,
        };

        assert!(orch.configure_redirect(redirect_config).is_ok());
        assert_eq!(orch.stats().redirects_processed, 0);

        assert!(orch
            .process_icmp_redirect("192.168.1.1", "10.0.0.1", "192.168.1.254")
            .is_ok());
        assert_eq!(orch.stats().redirects_processed, 1);
    }

    #[test]
    fn test_process_neighbor_discovery() {
        let config = IcmpOrchConfig {
            enable_redirects: false,
            enable_neighbor_discovery: true,
        };
        let mut orch: IcmpOrch<MockIcmpCallbacks> =
            IcmpOrch::new(config).with_callbacks(Arc::new(MockIcmpCallbacks));

        let nd_config = NeighborDiscoveryConfig {
            enabled: true,
            max_solicitation_delay: 1000,
        };

        assert!(orch.configure_neighbor_discovery(nd_config).is_ok());
        assert_eq!(orch.stats().nd_solicitations_processed, 0);

        assert!(orch.process_neighbor_discovery("fe80::1").is_ok());
        assert_eq!(orch.stats().nd_solicitations_processed, 1);
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
        assert_eq!(
            key.ip,
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1))
        );
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
        assert_eq!(stats.redirects_processed, 0);
        assert_eq!(stats.nd_solicitations_processed, 0);
    }

    #[test]
    fn test_icmp_orch_config_clone() {
        let config1 = IcmpOrchConfig {
            enable_redirects: true,
            enable_neighbor_discovery: true,
        };
        let config2 = config1.clone();

        let _orch1: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(config1);
        let _orch2: IcmpOrch<MockIcmpCallbacks> = IcmpOrch::new(config2);
    }

    #[test]
    fn test_icmp_orch_with_callbacks() {
        let config = IcmpOrchConfig::default();
        let orch: IcmpOrch<MockIcmpCallbacks> =
            IcmpOrch::new(config).with_callbacks(Arc::new(MockIcmpCallbacks));

        // Verify orch is created successfully
        assert_eq!(orch.get_entry_count(), 0);
    }

    #[test]
    fn test_process_redirect_without_config() {
        let config = IcmpOrchConfig {
            enable_redirects: true,
            enable_neighbor_discovery: false,
        };
        let mut orch: IcmpOrch<MockIcmpCallbacks> =
            IcmpOrch::new(config).with_callbacks(Arc::new(MockIcmpCallbacks));

        assert!(orch
            .process_icmp_redirect("192.168.1.1", "10.0.0.1", "192.168.1.254")
            .is_err());
    }
}
