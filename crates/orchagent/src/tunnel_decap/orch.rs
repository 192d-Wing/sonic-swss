//! Tunnel decapsulation orchestration logic.

use super::types::{TunnelDecapConfig, TunnelDecapEntry, TunnelTermType};
use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpAddress;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum TunnelDecapOrchError {
    TunnelExists(String),
    TunnelNotFound(String),
    TermEntryExists(String),
    TermEntryNotFound(String),
    InvalidConfig(String),
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
            return Err(TunnelDecapOrchError::TunnelExists(config.tunnel_name.clone()));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| TunnelDecapOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let tunnel_id = callbacks.create_tunnel(&config)
            .map_err(TunnelDecapOrchError::SaiError)?;

        let entry = TunnelDecapEntry::from_config(config.clone(), tunnel_id);
        self.tunnels.insert(config.tunnel_name, entry);
        self.stats.tunnels_created += 1;

        Ok(())
    }

    pub fn remove_tunnel(&mut self, tunnel_name: &str) -> Result<(), TunnelDecapOrchError> {
        let entry = self.tunnels.get(tunnel_name)
            .ok_or_else(|| TunnelDecapOrchError::TunnelNotFound(tunnel_name.to_string()))?;

        if !entry.term_entries.is_empty() {
            return Err(TunnelDecapOrchError::InvalidConfig(
                format!("Tunnel {} has {} term entries, remove them first", tunnel_name, entry.term_entries.len())
            ));
        }

        let entry = self.tunnels.remove(tunnel_name).unwrap();

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| TunnelDecapOrchError::InvalidConfig("No callbacks set".to_string()))?;

        callbacks.remove_tunnel(entry.tunnel_id)
            .map_err(TunnelDecapOrchError::SaiError)?;

        self.stats.tunnels_removed += 1;

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
            return Err(TunnelDecapOrchError::TermEntryExists(term_key));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| TunnelDecapOrchError::InvalidConfig("No callbacks set".to_string()))?,
        );

        let term_entry_id = callbacks.create_tunnel_term_entry(entry.tunnel_id, term_type, src_ip, dst_ip)
            .map_err(TunnelDecapOrchError::SaiError)?;

        entry.term_entries.insert(term_key, term_entry_id);
        self.stats.term_entries_created += 1;

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
}
