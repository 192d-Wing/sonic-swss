//! ZMQ orchestration logic.

use super::types::{ZmqEndpoint, ZmqStats};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum ZmqOrchError {
    ConnectionFailed(String),
    SendFailed(String),
}

#[derive(Debug, Clone, Default)]
pub struct ZmqOrchConfig {
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ZmqOrchStats {
    pub stats: ZmqStats,
}

pub trait ZmqOrchCallbacks: Send + Sync {}

pub struct ZmqOrch {
    config: ZmqOrchConfig,
    stats: ZmqOrchStats,
}

impl ZmqOrch {
    pub fn new(config: ZmqOrchConfig) -> Self {
        Self {
            config,
            stats: ZmqOrchStats::default(),
        }
    }

    pub fn stats(&self) -> &ZmqOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zmq_orch_new() {
        let config = ZmqOrchConfig::default();
        let orch = ZmqOrch::new(config);

        assert_eq!(orch.stats.stats.messages_sent, 0);
        assert_eq!(orch.stats.stats.messages_received, 0);
        assert_eq!(orch.stats.stats.errors, 0);
    }

    #[test]
    fn test_zmq_orch_new_with_default_config() {
        let orch = ZmqOrch::new(ZmqOrchConfig::default());

        // Verify initial state
        let stats = orch.stats();
        assert_eq!(stats.stats.messages_sent, 0);
        assert_eq!(stats.stats.messages_received, 0);
        assert_eq!(stats.stats.errors, 0);
    }

    #[test]
    fn test_zmq_orch_new_with_endpoint() {
        let config = ZmqOrchConfig {
            endpoint: Some("tcp://127.0.0.1:5555".to_string()),
        };
        let orch = ZmqOrch::new(config);

        assert_eq!(orch.config.endpoint, Some("tcp://127.0.0.1:5555".to_string()));
    }

    #[test]
    fn test_zmq_orch_config_default_has_no_endpoint() {
        let config = ZmqOrchConfig::default();

        assert!(config.endpoint.is_none());
    }

    #[test]
    fn test_zmq_stats_default() {
        let stats = ZmqOrchStats::default();

        assert_eq!(stats.stats.messages_sent, 0);
        assert_eq!(stats.stats.messages_received, 0);
        assert_eq!(stats.stats.errors, 0);
    }

    #[test]
    fn test_zmq_orch_config_clone() {
        let config1 = ZmqOrchConfig {
            endpoint: Some("tcp://localhost:5555".to_string()),
        };
        let config2 = config1.clone();

        assert_eq!(config1.endpoint, config2.endpoint);
    }

    #[test]
    fn test_zmq_orch_error_connection_failed() {
        let error = ZmqOrchError::ConnectionFailed("Failed to connect".to_string());

        match error {
            ZmqOrchError::ConnectionFailed(msg) => {
                assert_eq!(msg, "Failed to connect");
            }
            _ => panic!("Expected ConnectionFailed error"),
        }
    }

    #[test]
    fn test_zmq_orch_error_send_failed() {
        let error = ZmqOrchError::SendFailed("Message too large".to_string());

        match error {
            ZmqOrchError::SendFailed(msg) => {
                assert_eq!(msg, "Message too large");
            }
            _ => panic!("Expected SendFailed error"),
        }
    }

    #[test]
    fn test_zmq_orch_error_clone() {
        let error1 = ZmqOrchError::ConnectionFailed("Connection timeout".to_string());
        let error2 = error1.clone();

        match (error1, error2) {
            (ZmqOrchError::ConnectionFailed(m1), ZmqOrchError::ConnectionFailed(m2)) => {
                assert_eq!(m1, m2);
            }
            _ => panic!("Cloned error doesn't match original"),
        }
    }

    #[test]
    fn test_zmq_endpoint_creation() {
        let endpoint = ZmqEndpoint::new("tcp://0.0.0.0:5555".to_string());

        assert_eq!(endpoint.endpoint, "tcp://0.0.0.0:5555");
    }
}
