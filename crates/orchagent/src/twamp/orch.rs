//! TWAMP session orchestration logic (stub implementation).

use super::types::{TwampMode, TwampRole, TwampSessionConfig, TwampSessionEntry, TwampStats};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum TwampOrchError {
    SessionExists(String),
    SessionNotFound(String),
    ResourceExhausted,
    VrfNotFound(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct TwampOrchConfig {
    pub max_sessions: u32,
}

#[derive(Debug, Clone, Default)]
pub struct TwampOrchStats {
    pub sessions_created: u64,
    pub sessions_removed: u64,
}

pub trait TwampOrchCallbacks: Send + Sync {
    fn create_twamp_session(&self, config: &TwampSessionConfig) -> Result<RawSaiObjectId, String>;
    fn remove_twamp_session(&self, session_id: RawSaiObjectId) -> Result<(), String>;
    fn set_session_transmit(&self, session_id: RawSaiObjectId, enabled: bool) -> Result<(), String>;
}

pub struct TwampOrch {
    config: TwampOrchConfig,
    stats: TwampOrchStats,
    callbacks: Option<Arc<dyn TwampOrchCallbacks>>,
    sessions: HashMap<String, TwampSessionEntry>,
}

impl TwampOrch {
    pub fn new(config: TwampOrchConfig) -> Self {
        Self {
            config,
            stats: TwampOrchStats::default(),
            callbacks: None,
            sessions: HashMap::new(),
        }
    }

    pub fn set_callbacks(&mut self, callbacks: Arc<dyn TwampOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    pub fn session_exists(&self, name: &str) -> bool {
        self.sessions.contains_key(name)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn stats(&self) -> &TwampOrchStats {
        &self.stats
    }

    pub fn create_session(&mut self, config: TwampSessionConfig) -> Result<(), TwampOrchError> {
        if self.sessions.contains_key(&config.name) {
            return Err(TwampOrchError::SessionExists(config.name.clone()));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| TwampOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let session_id = callbacks.create_twamp_session(&config)
            .map_err(TwampOrchError::SaiError)?;

        let entry = TwampSessionEntry::from_config(config.clone(), session_id);
        self.sessions.insert(config.name, entry);
        self.stats.sessions_created += 1;

        Ok(())
    }

    pub fn remove_session(&mut self, name: &str) -> Result<(), TwampOrchError> {
        let entry = self.sessions.remove(name)
            .ok_or_else(|| TwampOrchError::SessionNotFound(name.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| TwampOrchError::SaiError("No callbacks set".to_string()))?;

        callbacks.remove_twamp_session(entry.session_id)
            .map_err(TwampOrchError::SaiError)?;

        self.stats.sessions_removed += 1;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twamp::types::{Dscp, TwampUdpPort};
    use sonic_types::IpAddress;
    use std::str::FromStr;

    struct MockCallbacks;
    impl TwampOrchCallbacks for MockCallbacks {
        fn create_twamp_session(&self, _config: &TwampSessionConfig) -> Result<RawSaiObjectId, String> {
            Ok(0x1000)
        }
        fn remove_twamp_session(&self, _session_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }
        fn set_session_transmit(&self, _session_id: RawSaiObjectId, _enabled: bool) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_create_session() {
        let mut orch = TwampOrch::new(TwampOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let mut config = TwampSessionConfig::new("session1".to_string(), TwampMode::Full, TwampRole::Sender);
        config.src_ip = IpAddress::from_str("10.0.0.1").unwrap();
        config.dst_ip = IpAddress::from_str("10.0.0.2").unwrap();

        assert!(orch.create_session(config).is_ok());
        assert_eq!(orch.session_count(), 1);
    }
}
