//! Mirror session orchestration logic.

use super::types::{MirrorEntry, MirrorSessionConfig, MirrorSessionType, RawSaiObjectId};
use std::collections::HashMap;
use std::sync::Arc;

pub type Result<T> = std::result::Result<T, MirrorOrchError>;

#[derive(Debug, Clone)]
pub enum MirrorOrchError {
    SessionExists(String),
    SessionNotFound(String),
    InvalidConfig(String),
    SaiError(String),
    RefCountError(String),
}

#[derive(Debug, Clone, Default)]
pub struct MirrorOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct MirrorOrchStats {
    pub sessions_created: u64,
    pub sessions_removed: u64,
    pub sessions_active: u64,
}

pub trait MirrorOrchCallbacks: Send + Sync {
    fn create_mirror_session(&self, config: &MirrorSessionConfig) -> Result<RawSaiObjectId>;
    fn remove_mirror_session(&self, session_id: RawSaiObjectId) -> Result<()>;
    fn update_mirror_session(&self, session_id: RawSaiObjectId, config: &MirrorSessionConfig) -> Result<()>;
    fn get_mirror_sessions_by_type(&self, session_type: MirrorSessionType) -> Result<Vec<RawSaiObjectId>>;
    fn on_session_created(&self, name: &str, session_id: RawSaiObjectId);
    fn on_session_removed(&self, name: &str);
}

pub struct MirrorOrch<C: MirrorOrchCallbacks> {
    config: MirrorOrchConfig,
    stats: MirrorOrchStats,
    sessions: HashMap<String, MirrorEntry>,
    callbacks: Option<Arc<C>>,
}

impl<C: MirrorOrchCallbacks> MirrorOrch<C> {
    pub fn new(config: MirrorOrchConfig) -> Self {
        Self {
            config,
            stats: MirrorOrchStats::default(),
            sessions: HashMap::new(),
            callbacks: None,
        }
    }

    pub fn with_callbacks(mut self, callbacks: Arc<C>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    pub fn create_session(&mut self, name: String, config: MirrorSessionConfig) -> Result<RawSaiObjectId> {
        if self.sessions.contains_key(&name) {
            return Err(MirrorOrchError::SessionExists(name));
        }

        let callbacks = self.callbacks.as_ref().ok_or(MirrorOrchError::SaiError("No callbacks".into()))?;
        let session_id = callbacks.create_mirror_session(&config)?;

        let entry = MirrorEntry {
            session_id: Some(session_id),
            config,
            ref_count: 1,
        };

        self.sessions.insert(name.clone(), entry);
        self.stats.sessions_created += 1;
        self.stats.sessions_active += 1;

        callbacks.on_session_created(&name, session_id);

        Ok(session_id)
    }

    pub fn remove_session(&mut self, name: &str) -> Result<()> {
        let entry = self.sessions.remove(name)
            .ok_or_else(|| MirrorOrchError::SessionNotFound(name.to_string()))?;

        if let Some(session_id) = entry.session_id {
            let callbacks = self.callbacks.as_ref().ok_or(MirrorOrchError::SaiError("No callbacks".into()))?;
            callbacks.remove_mirror_session(session_id)?;
            self.stats.sessions_removed += 1;
            self.stats.sessions_active = self.stats.sessions_active.saturating_sub(1);
            callbacks.on_session_removed(name);
        }

        Ok(())
    }

    pub fn update_session(&mut self, name: &str, config: MirrorSessionConfig) -> Result<()> {
        let entry = self.sessions.get_mut(name)
            .ok_or_else(|| MirrorOrchError::SessionNotFound(name.to_string()))?;

        if let Some(session_id) = entry.session_id {
            let callbacks = self.callbacks.as_ref().ok_or(MirrorOrchError::SaiError("No callbacks".into()))?;
            callbacks.update_mirror_session(session_id, &config)?;
            entry.config = config;
        }

        Ok(())
    }

    pub fn get_session(&self, name: &str) -> Option<&MirrorEntry> {
        self.sessions.get(name)
    }

    pub fn get_session_mut(&mut self, name: &str) -> Option<&mut MirrorEntry> {
        self.sessions.get_mut(name)
    }

    pub fn get_all_sessions(&self) -> Vec<(String, &MirrorEntry)> {
        self.sessions.iter().map(|(k, v)| (k.clone(), v)).collect()
    }

    pub fn session_exists(&self, name: &str) -> bool {
        self.sessions.contains_key(name)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn increment_ref_count(&mut self, name: &str) -> Result<u32> {
        let entry = self.sessions.get_mut(name)
            .ok_or_else(|| MirrorOrchError::SessionNotFound(name.to_string()))?;
        entry.ref_count = entry.ref_count.saturating_add(1);
        Ok(entry.ref_count)
    }

    pub fn decrement_ref_count(&mut self, name: &str) -> Result<u32> {
        let entry = self.sessions.get_mut(name)
            .ok_or_else(|| MirrorOrchError::SessionNotFound(name.to_string()))?;
        entry.ref_count = entry.ref_count.saturating_sub(1);
        Ok(entry.ref_count)
    }

    pub fn get_sessions_by_type(&self, session_type: MirrorSessionType) -> Vec<(String, &MirrorEntry)> {
        self.sessions.iter()
            .filter(|(_, entry)| entry.config.session_type == session_type)
            .map(|(k, v)| (k.clone(), v))
            .collect()
    }

    pub fn stats(&self) -> &MirrorOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::MirrorDirection;

    struct MockMirrorCallbacks;

    impl MirrorOrchCallbacks for MockMirrorCallbacks {
        fn create_mirror_session(&self, _config: &MirrorSessionConfig) -> Result<RawSaiObjectId> {
            Ok(0x1000)
        }

        fn remove_mirror_session(&self, _session_id: RawSaiObjectId) -> Result<()> {
            Ok(())
        }

        fn update_mirror_session(&self, _session_id: RawSaiObjectId, _config: &MirrorSessionConfig) -> Result<()> {
            Ok(())
        }

        fn get_mirror_sessions_by_type(&self, _session_type: MirrorSessionType) -> Result<Vec<RawSaiObjectId>> {
            Ok(vec![])
        }

        fn on_session_created(&self, _name: &str, _session_id: RawSaiObjectId) {}
        fn on_session_removed(&self, _name: &str) {}
    }

    #[test]
    fn test_create_session() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        let result = orch.create_session("session1".into(), config);
        assert!(result.is_ok());
        assert_eq!(orch.stats().sessions_created, 1);
        assert_eq!(orch.session_count(), 1);
    }

    #[test]
    fn test_create_duplicate_session() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.create_session("session1".into(), config.clone()).is_ok());
        assert!(orch.create_session("session1".into(), config).is_err());
    }

    #[test]
    fn test_remove_session() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.create_session("session1".into(), config).is_ok());
        assert_eq!(orch.session_count(), 1);

        assert!(orch.remove_session("session1").is_ok());
        assert_eq!(orch.session_count(), 0);
        assert_eq!(orch.stats().sessions_removed, 1);
    }

    #[test]
    fn test_update_session() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Rx,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.create_session("session1".into(), config).is_ok());

        let new_config = MirrorSessionConfig {
            session_type: MirrorSessionType::Erspan,
            direction: MirrorDirection::Tx,
            dst_port: Some("Ethernet4".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.update_session("session1", new_config.clone()).is_ok());
        let session = orch.get_session("session1").unwrap();
        assert_eq!(session.config.session_type, MirrorSessionType::Erspan);
    }

    #[test]
    fn test_get_session() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.create_session("session1".into(), config).is_ok());
        assert!(orch.get_session("session1").is_some());
        assert!(orch.get_session("nonexistent").is_none());
    }

    #[test]
    fn test_ref_count_operations() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.create_session("session1".into(), config).is_ok());
        assert_eq!(orch.increment_ref_count("session1").unwrap(), 2);
        assert_eq!(orch.increment_ref_count("session1").unwrap(), 3);
        assert_eq!(orch.decrement_ref_count("session1").unwrap(), 2);
    }

    #[test]
    fn test_get_sessions_by_type() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let span_config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        let erspan_config = MirrorSessionConfig {
            session_type: MirrorSessionType::Erspan,
            direction: MirrorDirection::Both,
            dst_port: None,
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.create_session("span1".into(), span_config.clone()).is_ok());
        assert!(orch.create_session("span2".into(), span_config).is_ok());
        assert!(orch.create_session("erspan1".into(), erspan_config).is_ok());

        let span_sessions = orch.get_sessions_by_type(MirrorSessionType::Span);
        assert_eq!(span_sessions.len(), 2);

        let erspan_sessions = orch.get_sessions_by_type(MirrorSessionType::Erspan);
        assert_eq!(erspan_sessions.len(), 1);
    }

    #[test]
    fn test_get_all_sessions() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(orch.create_session("session1".into(), config.clone()).is_ok());
        assert!(orch.create_session("session2".into(), config).is_ok());

        let all = orch.get_all_sessions();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_session_exists() {
        let mut orch: MirrorOrch<MockMirrorCallbacks> = MirrorOrch::new(MirrorOrchConfig::default())
            .with_callbacks(Arc::new(MockMirrorCallbacks));

        let config = MirrorSessionConfig {
            session_type: MirrorSessionType::Span,
            direction: MirrorDirection::Both,
            dst_port: Some("Ethernet0".to_string()),
            src_ip: None,
            dst_ip: None,
        };

        assert!(!orch.session_exists("session1"));
        assert!(orch.create_session("session1".into(), config).is_ok());
        assert!(orch.session_exists("session1"));
    }
}
