//! Tunnel decapsulation orchestration logic.

use super::types::{TunnelDecapConfig, TunnelDecapEntry, TunnelTermType};
use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpAddress;
use std::collections::HashMap;
use std::sync::Arc;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};

#[derive(Debug, Clone, thiserror::Error)]
pub enum TunnelDecapOrchError {
    #[error("Tunnel exists: {0}")]
    TunnelExists(String),
    #[error("Tunnel not found: {0}")]
    TunnelNotFound(String),
    #[error("Term entry exists: {0}")]
    TermEntryExists(String),
    #[error("Term entry not found: {0}")]
    TermEntryNotFound(String),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("SAI error: {0}")]
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct TunnelDecapOrchConfig {
    pub max_tunnels: u32,
}

#[derive(Debug, Clone, Default)]
pub struct TunnelDecapOrchStats {
    pub tunnels_created: u64,
    pub tunnels_removed: u64,
    pub term_entries_created: u64,
    pub term_entries_removed: u64,
}

pub trait TunnelDecapOrchCallbacks: Send + Sync {
    fn create_tunnel(&self, config: &TunnelDecapConfig) -> Result<RawSaiObjectId, String>;
    fn remove_tunnel(&self, tunnel_id: RawSaiObjectId) -> Result<(), String>;
    fn create_tunnel_term_entry(
        &self,
        tunnel_id: RawSaiObjectId,
        term_type: TunnelTermType,
        src_ip: IpAddress,
        dst_ip: IpAddress,
    ) -> Result<RawSaiObjectId, String>;
    fn remove_tunnel_term_entry(&self, term_entry_id: RawSaiObjectId) -> Result<(), String>;
}

pub struct TunnelDecapOrch {
    config: TunnelDecapOrchConfig,
    stats: TunnelDecapOrchStats,
    callbacks: Option<Arc<dyn TunnelDecapOrchCallbacks>>,
    tunnels: HashMap<String, TunnelDecapEntry>,
}

impl TunnelDecapOrch {
    pub fn new(config: TunnelDecapOrchConfig) -> Self {
        Self {
            config,
            stats: TunnelDecapOrchStats::default(),
            callbacks: None,
            tunnels: HashMap::new(),
        }
    }

    pub fn set_callbacks(&mut self, callbacks: Arc<dyn TunnelDecapOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    pub fn tunnel_exists(&self, name: &str) -> bool {
        self.tunnels.contains_key(name)
    }

    pub fn tunnel_count(&self) -> usize {
        self.tunnels.len()
    }

    pub fn stats(&self) -> &TunnelDecapOrchStats {
        &self.stats
    }

    pub fn create_tunnel(&mut self, config: TunnelDecapConfig) -> Result<(), TunnelDecapOrchError> {
        if self.tunnels.contains_key(&config.tunnel_name) {
            let error = TunnelDecapOrchError::TunnelExists(config.tunnel_name.clone());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "TunnelDecapOrch",
                "create_tunnel_term"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(config.tunnel_name.clone())
            .with_object_type("tunnel_term")
            .with_error(error.to_string()));
            return Err(error);
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| TunnelDecapOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let tunnel_id = callbacks.create_tunnel(&config)
            .map_err(TunnelDecapOrchError::SaiError)?;

        let entry = TunnelDecapEntry::from_config(config.clone(), tunnel_id);
        self.tunnels.insert(config.tunnel_name.clone(), entry);
        self.stats.tunnels_created += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "TunnelDecapOrch",
            "create_tunnel_term"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(config.tunnel_name.clone())
        .with_object_type("tunnel_term")
        .with_details(serde_json::json!({
            "tunnel_name": config.tunnel_name,
            "tunnel_type": config.tunnel_type,
            "tunnel_id": tunnel_id,
            "stats": {
                "tunnels_created": self.stats.tunnels_created
            }
        })));

        Ok(())
    }

    pub fn remove_tunnel(&mut self, tunnel_name: &str) -> Result<(), TunnelDecapOrchError> {
        let entry = self.tunnels.get(tunnel_name)
            .ok_or_else(|| TunnelDecapOrchError::TunnelNotFound(tunnel_name.to_string()))?;

        if !entry.term_entries.is_empty() {
            let error = TunnelDecapOrchError::InvalidConfig(
                format!("Tunnel {} has {} term entries, remove them first", tunnel_name, entry.term_entries.len())
            );
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceDelete,
                "TunnelDecapOrch",
                "remove_tunnel_term"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(tunnel_name.to_string())
            .with_object_type("tunnel_term")
            .with_error(error.to_string()));
            return Err(error);
        }

        let entry = self.tunnels.remove(tunnel_name).unwrap();

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| TunnelDecapOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks.remove_tunnel(entry.tunnel_id)
            .map_err(TunnelDecapOrchError::SaiError)?;

        self.stats.tunnels_removed += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "TunnelDecapOrch",
            "remove_tunnel_term"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(tunnel_name.to_string())
        .with_object_type("tunnel_term")
        .with_details(serde_json::json!({
            "tunnel_name": tunnel_name,
            "tunnel_id": entry.tunnel_id,
            "stats": {
                "tunnels_removed": self.stats.tunnels_removed
            }
        })));

        Ok(())
    }

    pub fn add_term_entry(
        &mut self,
        tunnel_name: &str,
        term_key: String,
        term_type: TunnelTermType,
        src_ip: IpAddress,
        dst_ip: IpAddress,
    ) -> Result<(), TunnelDecapOrchError> {
        let entry = self.tunnels.get_mut(tunnel_name)
            .ok_or_else(|| TunnelDecapOrchError::TunnelNotFound(tunnel_name.to_string()))?;

        if entry.term_entries.contains_key(&term_key) {
            let error = TunnelDecapOrchError::TermEntryExists(term_key.clone());
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "TunnelDecapOrch",
                "add_decap_tunnel"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(term_key.clone())
            .with_object_type("tunnel_term_entry")
            .with_error(error.to_string()));
            return Err(error);
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| TunnelDecapOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let term_entry_id = callbacks.create_tunnel_term_entry(entry.tunnel_id, term_type, src_ip, dst_ip)
            .map_err(TunnelDecapOrchError::SaiError)?;

        entry.term_entries.insert(term_key.clone(), term_entry_id);
        self.stats.term_entries_created += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "TunnelDecapOrch",
            "add_decap_tunnel"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(term_key.clone())
        .with_object_type("tunnel_term_entry")
        .with_details(serde_json::json!({
            "tunnel_name": tunnel_name,
            "term_key": term_key,
            "term_type": format!("{:?}", term_type),
            "src_ip": src_ip.to_string(),
            "dst_ip": dst_ip.to_string(),
            "term_entry_id": term_entry_id,
            "stats": {
                "term_entries_created": self.stats.term_entries_created
            }
        })));

        Ok(())
    }

    pub fn remove_term_entry(&mut self, tunnel_name: &str, term_key: &str) -> Result<(), TunnelDecapOrchError> {
        let entry = self.tunnels.get_mut(tunnel_name)
            .ok_or_else(|| TunnelDecapOrchError::TunnelNotFound(tunnel_name.to_string()))?;

        let term_entry_id = entry.term_entries.remove(term_key)
            .ok_or_else(|| TunnelDecapOrchError::TermEntryNotFound(term_key.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| TunnelDecapOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks.remove_tunnel_term_entry(term_entry_id)
            .map_err(TunnelDecapOrchError::SaiError)?;

        self.stats.term_entries_removed += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "TunnelDecapOrch",
            "remove_decap_tunnel"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(term_key.to_string())
        .with_object_type("tunnel_term_entry")
        .with_details(serde_json::json!({
            "tunnel_name": tunnel_name,
            "term_key": term_key,
            "term_entry_id": term_entry_id,
            "stats": {
                "term_entries_removed": self.stats.term_entries_removed
            }
        })));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    struct MockCallbacks;
    impl TunnelDecapOrchCallbacks for MockCallbacks {
        fn create_tunnel(&self, _config: &TunnelDecapConfig) -> Result<RawSaiObjectId, String> {
            Ok(0x5000)
        }
        fn remove_tunnel(&self, _tunnel_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
        fn create_tunnel_term_entry(
            &self,
            _tunnel_id: RawSaiObjectId,
            _term_type: TunnelTermType,
            _src_ip: IpAddress,
            _dst_ip: IpAddress,
        ) -> Result<RawSaiObjectId, String> {
            Ok(0x6000)
        }
        fn remove_tunnel_term_entry(&self, _term_entry_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_create_tunnel() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new(
            "ipinip_tunnel".to_string(),
            "IPINIP".to_string(),
        );

        assert!(orch.create_tunnel(config).is_ok());
        assert_eq!(orch.tunnel_count(), 1);
    }

    #[test]
    fn test_term_entry() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new(
            "ipinip_tunnel".to_string(),
            "IPINIP".to_string(),
        );

        orch.create_tunnel(config).unwrap();

        assert!(orch.add_term_entry(
            "ipinip_tunnel",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        ).is_ok());

        assert_eq!(orch.stats().term_entries_created, 1);

        assert!(orch.remove_term_entry("ipinip_tunnel", "term1").is_ok());
        assert_eq!(orch.stats().term_entries_removed, 1);
    }

    #[test]
    fn test_remove_tunnel_with_term_entries() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new(
            "ipinip_tunnel".to_string(),
            "IPINIP".to_string(),
        );

        orch.create_tunnel(config).unwrap();

        orch.add_term_entry(
            "ipinip_tunnel",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        ).unwrap();

        // Should fail because term entries exist
        assert!(orch.remove_tunnel("ipinip_tunnel").is_err());

        // Remove term entry first
        orch.remove_term_entry("ipinip_tunnel", "term1").unwrap();

        // Now should succeed
        assert!(orch.remove_tunnel("ipinip_tunnel").is_ok());
    }

    // ========================================================================
    // Tunnel Entry Management Tests
    // ========================================================================

    #[test]
    fn test_create_vxlan_tunnel() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new(
            "vxlan_tunnel".to_string(),
            "VXLAN".to_string(),
        );

        assert!(orch.create_tunnel(config).is_ok());
        assert_eq!(orch.tunnel_count(), 1);
        assert!(orch.tunnel_exists("vxlan_tunnel"));
        assert_eq!(orch.stats().tunnels_created, 1);
    }

    #[test]
    fn test_create_nvgre_tunnel() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new(
            "nvgre_tunnel".to_string(),
            "NVGRE".to_string(),
        );

        assert!(orch.create_tunnel(config).is_ok());
        assert_eq!(orch.tunnel_count(), 1);
        assert!(orch.tunnel_exists("nvgre_tunnel"));
    }

    #[test]
    fn test_duplicate_tunnel_creation() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config1 = TunnelDecapConfig::new(
            "tunnel1".to_string(),
            "IPINIP".to_string(),
        );

        let config2 = TunnelDecapConfig::new(
            "tunnel1".to_string(),
            "IPINIP".to_string(),
        );

        assert!(orch.create_tunnel(config1).is_ok());
        let result = orch.create_tunnel(config2);
        assert!(result.is_err());

        match result {
            Err(TunnelDecapOrchError::TunnelExists(name)) => {
                assert_eq!(name, "tunnel1");
            },
            _ => panic!("Expected TunnelExists error"),
        }
    }

    #[test]
    fn test_remove_nonexistent_tunnel() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.remove_tunnel("nonexistent");
        assert!(result.is_err());

        match result {
            Err(TunnelDecapOrchError::TunnelNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            },
            _ => panic!("Expected TunnelNotFound error"),
        }
    }

    #[test]
    fn test_remove_tunnel_success() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new(
            "tunnel1".to_string(),
            "IPINIP".to_string(),
        );

        orch.create_tunnel(config).unwrap();
        assert_eq!(orch.tunnel_count(), 1);

        assert!(orch.remove_tunnel("tunnel1").is_ok());
        assert_eq!(orch.tunnel_count(), 0);
        assert_eq!(orch.stats().tunnels_removed, 1);
        assert!(!orch.tunnel_exists("tunnel1"));
    }

    #[test]
    fn test_multiple_tunnels_different_types() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let ipinip = TunnelDecapConfig::new("ipinip".to_string(), "IPINIP".to_string());
        let vxlan = TunnelDecapConfig::new("vxlan".to_string(), "VXLAN".to_string());
        let nvgre = TunnelDecapConfig::new("nvgre".to_string(), "NVGRE".to_string());

        assert!(orch.create_tunnel(ipinip).is_ok());
        assert!(orch.create_tunnel(vxlan).is_ok());
        assert!(orch.create_tunnel(nvgre).is_ok());

        assert_eq!(orch.tunnel_count(), 3);
        assert!(orch.tunnel_exists("ipinip"));
        assert!(orch.tunnel_exists("vxlan"));
        assert!(orch.tunnel_exists("nvgre"));
    }

    // ========================================================================
    // Tunnel Termination Tests
    // ========================================================================

    #[test]
    fn test_p2p_tunnel_termination() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.add_term_entry(
            "tunnel1",
            "p2p_term".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("192.168.1.1").unwrap(),
            IpAddress::from_str("192.168.1.2").unwrap(),
        );

        assert!(result.is_ok());
        assert_eq!(orch.stats().term_entries_created, 1);
    }

    #[test]
    fn test_p2mp_tunnel_termination() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.add_term_entry(
            "tunnel1",
            "p2mp_term".to_string(),
            TunnelTermType::P2MP,
            IpAddress::from_str("10.1.0.1").unwrap(),
            IpAddress::from_str("10.1.0.0").unwrap(),
        );

        assert!(result.is_ok());
        assert_eq!(orch.stats().term_entries_created, 1);
    }

    #[test]
    fn test_mp2mp_tunnel_termination() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "VXLAN".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.add_term_entry(
            "tunnel1",
            "mp2mp_term".to_string(),
            TunnelTermType::MP2MP,
            IpAddress::from_str("0.0.0.0").unwrap(),
            IpAddress::from_str("0.0.0.0").unwrap(),
        );

        assert!(result.is_ok());
        assert_eq!(orch.stats().term_entries_created, 1);
    }

    #[test]
    fn test_multiple_terminations_per_tunnel() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        // Add first termination
        assert!(orch.add_term_entry(
            "tunnel1",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        ).is_ok());

        // Add second termination
        assert!(orch.add_term_entry(
            "tunnel1",
            "term2".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.3").unwrap(),
            IpAddress::from_str("10.0.0.4").unwrap(),
        ).is_ok());

        // Add third termination
        assert!(orch.add_term_entry(
            "tunnel1",
            "term3".to_string(),
            TunnelTermType::P2MP,
            IpAddress::from_str("10.0.0.5").unwrap(),
            IpAddress::from_str("10.0.0.0").unwrap(),
        ).is_ok());

        assert_eq!(orch.stats().term_entries_created, 3);
    }

    // ========================================================================
    // Source/Destination Configuration Tests
    // ========================================================================

    #[test]
    fn test_ipv4_source_destination() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.add_term_entry(
            "tunnel1",
            "ipv4_term".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("192.168.1.100").unwrap(),
            IpAddress::from_str("192.168.1.200").unwrap(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_ipv6_source_destination() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.add_term_entry(
            "tunnel1",
            "ipv6_term".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("2001:db8::1").unwrap(),
            IpAddress::from_str("2001:db8::2").unwrap(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_wildcard_source_ip() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "VXLAN".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.add_term_entry(
            "tunnel1",
            "wildcard_src".to_string(),
            TunnelTermType::MP2MP,
            IpAddress::from_str("0.0.0.0").unwrap(),
            IpAddress::from_str("10.0.0.1").unwrap(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_wildcard_destination_ip() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "VXLAN".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.add_term_entry(
            "tunnel1",
            "wildcard_dst".to_string(),
            TunnelTermType::P2MP,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("0.0.0.0").unwrap(),
        );

        assert!(result.is_ok());
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[test]
    fn test_add_term_entry_to_nonexistent_tunnel() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.add_term_entry(
            "nonexistent",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        );

        assert!(result.is_err());
        match result {
            Err(TunnelDecapOrchError::TunnelNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            },
            _ => panic!("Expected TunnelNotFound error"),
        }
    }

    #[test]
    fn test_duplicate_term_entry() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        // Add first term entry
        assert!(orch.add_term_entry(
            "tunnel1",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        ).is_ok());

        // Try to add duplicate
        let result = orch.add_term_entry(
            "tunnel1",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.3").unwrap(),
            IpAddress::from_str("10.0.0.4").unwrap(),
        );

        assert!(result.is_err());
        match result {
            Err(TunnelDecapOrchError::TermEntryExists(key)) => {
                assert_eq!(key, "term1");
            },
            _ => panic!("Expected TermEntryExists error"),
        }
    }

    #[test]
    fn test_remove_nonexistent_term_entry() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        let result = orch.remove_term_entry("tunnel1", "nonexistent");
        assert!(result.is_err());

        match result {
            Err(TunnelDecapOrchError::TermEntryNotFound(key)) => {
                assert_eq!(key, "nonexistent");
            },
            _ => panic!("Expected TermEntryNotFound error"),
        }
    }

    #[test]
    fn test_create_tunnel_without_callbacks() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        // Don't set callbacks

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        let result = orch.create_tunnel(config);

        assert!(result.is_err());
        match result {
            Err(TunnelDecapOrchError::InvalidConfig(msg)) => {
                assert!(msg.contains("No callbacks set"));
            },
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    struct FailingCallbacks;
    impl TunnelDecapOrchCallbacks for FailingCallbacks {
        fn create_tunnel(&self, _config: &TunnelDecapConfig) -> Result<RawSaiObjectId, String> {
            Err("SAI tunnel creation failed".to_string())
        }
        fn remove_tunnel(&self, _tunnel_id: RawSaiObjectId) -> Result<(), String> {
            Err("SAI tunnel removal failed".to_string())
        }
        fn create_tunnel_term_entry(
            &self,
            _tunnel_id: RawSaiObjectId,
            _term_type: TunnelTermType,
            _src_ip: IpAddress,
            _dst_ip: IpAddress,
        ) -> Result<RawSaiObjectId, String> {
            Err("SAI term entry creation failed".to_string())
        }
        fn remove_tunnel_term_entry(&self, _term_entry_id: RawSaiObjectId) -> Result<(), String> {
            Err("SAI term entry removal failed".to_string())
        }
    }

    #[test]
    fn test_sai_tunnel_creation_failure() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(FailingCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        let result = orch.create_tunnel(config);

        assert!(result.is_err());
        match result {
            Err(TunnelDecapOrchError::SaiError(msg)) => {
                assert!(msg.contains("SAI tunnel creation failed"));
            },
            _ => panic!("Expected SaiError"),
        }
        // Tunnel should not be created
        assert_eq!(orch.tunnel_count(), 0);
    }

    #[test]
    fn test_sai_term_entry_creation_failure() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(FailingCallbacks));

        // First create with MockCallbacks
        let mut orch2 = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch2.set_callbacks(Arc::new(MockCallbacks));
        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch2.create_tunnel(config).unwrap();

        // Now switch to FailingCallbacks
        orch2.set_callbacks(Arc::new(FailingCallbacks));

        let result = orch2.add_term_entry(
            "tunnel1",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        );

        assert!(result.is_err());
        match result {
            Err(TunnelDecapOrchError::SaiError(msg)) => {
                assert!(msg.contains("SAI term entry creation failed"));
            },
            _ => panic!("Expected SaiError"),
        }
    }

    // ========================================================================
    // Statistics Tests
    // ========================================================================

    #[test]
    fn test_tunnel_statistics() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        assert_eq!(orch.stats().tunnels_created, 0);
        assert_eq!(orch.stats().tunnels_removed, 0);

        // Create three tunnels
        for i in 1..=3 {
            let config = TunnelDecapConfig::new(
                format!("tunnel{}", i),
                "IPINIP".to_string(),
            );
            orch.create_tunnel(config).unwrap();
        }

        assert_eq!(orch.stats().tunnels_created, 3);
        assert_eq!(orch.tunnel_count(), 3);

        // Remove two tunnels
        orch.remove_tunnel("tunnel1").unwrap();
        orch.remove_tunnel("tunnel2").unwrap();

        assert_eq!(orch.stats().tunnels_removed, 2);
        assert_eq!(orch.tunnel_count(), 1);
    }

    #[test]
    fn test_term_entry_statistics() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        assert_eq!(orch.stats().term_entries_created, 0);
        assert_eq!(orch.stats().term_entries_removed, 0);

        // Create five term entries
        for i in 1..=5 {
            orch.add_term_entry(
                "tunnel1",
                format!("term{}", i),
                TunnelTermType::P2P,
                IpAddress::from_str(&format!("10.0.0.{}", i)).unwrap(),
                IpAddress::from_str(&format!("10.0.1.{}", i)).unwrap(),
            ).unwrap();
        }

        assert_eq!(orch.stats().term_entries_created, 5);

        // Remove three term entries
        orch.remove_term_entry("tunnel1", "term1").unwrap();
        orch.remove_term_entry("tunnel1", "term2").unwrap();
        orch.remove_term_entry("tunnel1", "term3").unwrap();

        assert_eq!(orch.stats().term_entries_removed, 3);
    }

    #[test]
    fn test_combined_statistics() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Create two tunnels
        let config1 = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        let config2 = TunnelDecapConfig::new("tunnel2".to_string(), "VXLAN".to_string());
        orch.create_tunnel(config1).unwrap();
        orch.create_tunnel(config2).unwrap();

        // Add term entries to both tunnels
        orch.add_term_entry(
            "tunnel1",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        ).unwrap();

        orch.add_term_entry(
            "tunnel2",
            "term1".to_string(),
            TunnelTermType::MP2MP,
            IpAddress::from_str("0.0.0.0").unwrap(),
            IpAddress::from_str("0.0.0.0").unwrap(),
        ).unwrap();

        assert_eq!(orch.stats().tunnels_created, 2);
        assert_eq!(orch.stats().term_entries_created, 2);

        // Clean up
        orch.remove_term_entry("tunnel1", "term1").unwrap();
        orch.remove_term_entry("tunnel2", "term1").unwrap();
        orch.remove_tunnel("tunnel1").unwrap();
        orch.remove_tunnel("tunnel2").unwrap();

        assert_eq!(orch.stats().tunnels_removed, 2);
        assert_eq!(orch.stats().term_entries_removed, 2);
        assert_eq!(orch.tunnel_count(), 0);
    }

    // ========================================================================
    // Edge Cases Tests
    // ========================================================================

    #[test]
    fn test_empty_tunnel_lifecycle() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Create tunnel without any terminations
        let config = TunnelDecapConfig::new("empty_tunnel".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        assert_eq!(orch.tunnel_count(), 1);
        assert!(orch.tunnel_exists("empty_tunnel"));

        // Should be able to remove empty tunnel
        assert!(orch.remove_tunnel("empty_tunnel").is_ok());
        assert_eq!(orch.tunnel_count(), 0);
    }

    #[test]
    fn test_tunnel_exists_check() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        assert!(!orch.tunnel_exists("tunnel1"));

        let config = TunnelDecapConfig::new("tunnel1".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        assert!(orch.tunnel_exists("tunnel1"));
        assert!(!orch.tunnel_exists("tunnel2"));

        orch.remove_tunnel("tunnel1").unwrap();
        assert!(!orch.tunnel_exists("tunnel1"));
    }

    #[test]
    fn test_remove_term_entry_from_nonexistent_tunnel() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.remove_term_entry("nonexistent", "term1");
        assert!(result.is_err());

        match result {
            Err(TunnelDecapOrchError::TunnelNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            },
            _ => panic!("Expected TunnelNotFound error"),
        }
    }

    #[test]
    fn test_complex_tunnel_workflow() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Create tunnel
        let config = TunnelDecapConfig::new("workflow_tunnel".to_string(), "IPINIP".to_string());
        orch.create_tunnel(config).unwrap();

        // Add multiple term entries
        orch.add_term_entry(
            "workflow_tunnel",
            "term1".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.1").unwrap(),
            IpAddress::from_str("10.0.0.2").unwrap(),
        ).unwrap();

        orch.add_term_entry(
            "workflow_tunnel",
            "term2".to_string(),
            TunnelTermType::P2P,
            IpAddress::from_str("10.0.0.3").unwrap(),
            IpAddress::from_str("10.0.0.4").unwrap(),
        ).unwrap();

        orch.add_term_entry(
            "workflow_tunnel",
            "term3".to_string(),
            TunnelTermType::P2MP,
            IpAddress::from_str("10.0.0.5").unwrap(),
            IpAddress::from_str("0.0.0.0").unwrap(),
        ).unwrap();

        assert_eq!(orch.stats().term_entries_created, 3);

        // Try to remove tunnel (should fail)
        assert!(orch.remove_tunnel("workflow_tunnel").is_err());

        // Remove some term entries
        orch.remove_term_entry("workflow_tunnel", "term1").unwrap();
        orch.remove_term_entry("workflow_tunnel", "term2").unwrap();

        // Still should fail
        assert!(orch.remove_tunnel("workflow_tunnel").is_err());

        // Remove last term entry
        orch.remove_term_entry("workflow_tunnel", "term3").unwrap();

        // Now should succeed
        assert!(orch.remove_tunnel("workflow_tunnel").is_ok());
        assert_eq!(orch.tunnel_count(), 0);
    }

    #[test]
    fn test_tunnel_count_accuracy() {
        let mut orch = TunnelDecapOrch::new(TunnelDecapOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        assert_eq!(orch.tunnel_count(), 0);

        // Add tunnels incrementally and verify count
        for i in 1..=10 {
            let config = TunnelDecapConfig::new(
                format!("tunnel{}", i),
                "IPINIP".to_string(),
            );
            orch.create_tunnel(config).unwrap();
            assert_eq!(orch.tunnel_count(), i);
        }

        // Remove tunnels and verify count
        for i in 1..=10 {
            orch.remove_tunnel(&format!("tunnel{}", i)).unwrap();
            assert_eq!(orch.tunnel_count(), 10 - i);
        }

        assert_eq!(orch.tunnel_count(), 0);
    }
}
