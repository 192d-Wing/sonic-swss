//! ZMQ orchestration logic.

use super::types::{ZmqEndpoint, ZmqStats};
use std::collections::HashMap;
use crate::audit::{AuditRecord, AuditCategory, AuditOutcome};
use crate::audit_log;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ZmqOrchError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Send failed: {0}")]
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

    /// Publishes an event to the ZMQ endpoint.
    pub fn publish_event(&mut self, event_type: &str, event_data: &str) -> Result<(), ZmqOrchError> {
        if self.config.endpoint.is_none() {
            let error = ZmqOrchError::ConnectionFailed("No ZMQ endpoint configured".to_string());
            audit_log!(AuditRecord::new(
                AuditCategory::AdminAction,
                "ZmqOrch",
                "publish_event"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(event_type.to_string())
            .with_object_type("zmq_event")
            .with_error(error.to_string()));
            return Err(error);
        }

        self.stats.stats.messages_sent += 1;

        audit_log!(AuditRecord::new(
            AuditCategory::AdminAction,
            "ZmqOrch",
            "publish_event"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(event_type.to_string())
        .with_object_type("zmq_event")
        .with_details(serde_json::json!({
            "event_type": event_type,
            "endpoint": self.config.endpoint,
            "stats": {
                "messages_sent": self.stats.stats.messages_sent
            }
        })));

        Ok(())
    }

    /// Subscribes to events from the ZMQ endpoint.
    pub fn subscribe_events(&mut self) -> Result<(), ZmqOrchError> {
        if self.config.endpoint.is_none() {
            let error = ZmqOrchError::ConnectionFailed("No ZMQ endpoint configured".to_string());
            audit_log!(AuditRecord::new(
                AuditCategory::AdminAction,
                "ZmqOrch",
                "subscribe_events"
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id("zmq_subscription".to_string())
            .with_object_type("zmq_subscription")
            .with_error(error.to_string()));
            return Err(error);
        }

        audit_log!(AuditRecord::new(
            AuditCategory::AdminAction,
            "ZmqOrch",
            "subscribe_events"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id("zmq_subscription".to_string())
        .with_object_type("zmq_subscription")
        .with_details(serde_json::json!({
            "endpoint": self.config.endpoint,
            "stats": {
                "messages_received": self.stats.stats.messages_received
            }
        })));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::ZmqMessage;

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

    // ===== Configuration tests =====

    #[test]
    fn test_zmq_orch_config_with_tcp_endpoint() {
        let config = ZmqOrchConfig {
            endpoint: Some("tcp://192.168.1.1:6666".to_string()),
        };

        assert!(config.endpoint.is_some());
        assert_eq!(config.endpoint.unwrap(), "tcp://192.168.1.1:6666");
    }

    #[test]
    fn test_zmq_orch_config_with_ipc_endpoint() {
        let config = ZmqOrchConfig {
            endpoint: Some("ipc:///tmp/zmq.sock".to_string()),
        };

        assert!(config.endpoint.is_some());
        assert_eq!(config.endpoint.unwrap(), "ipc:///tmp/zmq.sock");
    }

    #[test]
    fn test_zmq_orch_config_clone_with_endpoint() {
        let config1 = ZmqOrchConfig {
            endpoint: Some("tcp://127.0.0.1:8888".to_string()),
        };
        let config2 = config1.clone();

        assert_eq!(config1.endpoint, config2.endpoint);
    }

    #[test]
    fn test_zmq_orch_config_clone_without_endpoint() {
        let config1 = ZmqOrchConfig { endpoint: None };
        let config2 = config1.clone();

        assert_eq!(config1.endpoint, config2.endpoint);
        assert!(config2.endpoint.is_none());
    }

    #[test]
    fn test_zmq_orch_config_debug() {
        let config = ZmqOrchConfig {
            endpoint: Some("tcp://localhost:5555".to_string()),
        };
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("ZmqOrchConfig"));
    }

    // ===== Statistics tests =====

    #[test]
    fn test_zmq_stats_messages_sent_counter() {
        let mut stats = ZmqStats::default();

        stats.messages_sent = 100;
        assert_eq!(stats.messages_sent, 100);

        stats.messages_sent += 50;
        assert_eq!(stats.messages_sent, 150);
    }

    #[test]
    fn test_zmq_stats_messages_received_counter() {
        let mut stats = ZmqStats::default();

        stats.messages_received = 200;
        assert_eq!(stats.messages_received, 200);

        stats.messages_received += 75;
        assert_eq!(stats.messages_received, 275);
    }

    #[test]
    fn test_zmq_stats_errors_counter() {
        let mut stats = ZmqStats::default();

        stats.errors = 0;
        assert_eq!(stats.errors, 0);

        stats.errors += 1;
        assert_eq!(stats.errors, 1);

        stats.errors += 4;
        assert_eq!(stats.errors, 5);
    }

    #[test]
    fn test_zmq_stats_clone() {
        let stats1 = ZmqStats {
            messages_sent: 100,
            messages_received: 200,
            errors: 5,
        };
        let stats2 = stats1.clone();

        assert_eq!(stats1.messages_sent, stats2.messages_sent);
        assert_eq!(stats1.messages_received, stats2.messages_received);
        assert_eq!(stats1.errors, stats2.errors);
    }

    #[test]
    fn test_zmq_stats_all_counters_independent() {
        let mut stats = ZmqStats::default();

        stats.messages_sent = 10;
        stats.messages_received = 20;
        stats.errors = 3;

        assert_eq!(stats.messages_sent, 10);
        assert_eq!(stats.messages_received, 20);
        assert_eq!(stats.errors, 3);
    }

    #[test]
    fn test_zmq_orch_stats_wrapper() {
        let orch_stats = ZmqOrchStats::default();

        assert_eq!(orch_stats.stats.messages_sent, 0);
        assert_eq!(orch_stats.stats.messages_received, 0);
        assert_eq!(orch_stats.stats.errors, 0);
    }

    #[test]
    fn test_zmq_orch_stats_clone() {
        let stats1 = ZmqOrchStats {
            stats: ZmqStats {
                messages_sent: 50,
                messages_received: 100,
                errors: 2,
            },
        };
        let stats2 = stats1.clone();

        assert_eq!(stats1.stats.messages_sent, stats2.stats.messages_sent);
        assert_eq!(stats1.stats.messages_received, stats2.stats.messages_received);
        assert_eq!(stats1.stats.errors, stats2.stats.errors);
    }

    #[test]
    fn test_zmq_orch_stats_access_via_orch() {
        let orch = ZmqOrch::new(ZmqOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.stats.messages_sent, 0);
        assert_eq!(stats.stats.messages_received, 0);
        assert_eq!(stats.stats.errors, 0);
    }

    // ===== Error handling tests =====

    #[test]
    fn test_zmq_orch_error_connection_failed_clone() {
        let error1 = ZmqOrchError::ConnectionFailed("Timeout".to_string());
        let error2 = error1.clone();

        match (error1, error2) {
            (ZmqOrchError::ConnectionFailed(m1), ZmqOrchError::ConnectionFailed(m2)) => {
                assert_eq!(m1, m2);
            }
            _ => panic!("Error types don't match"),
        }
    }

    #[test]
    fn test_zmq_orch_error_send_failed_clone() {
        let error1 = ZmqOrchError::SendFailed("Buffer full".to_string());
        let error2 = error1.clone();

        match (error1, error2) {
            (ZmqOrchError::SendFailed(m1), ZmqOrchError::SendFailed(m2)) => {
                assert_eq!(m1, m2);
            }
            _ => panic!("Error types don't match"),
        }
    }

    #[test]
    fn test_zmq_orch_error_debug_format() {
        let error1 = ZmqOrchError::ConnectionFailed("Test error".to_string());
        let error2 = ZmqOrchError::SendFailed("Another error".to_string());

        let debug1 = format!("{:?}", error1);
        let debug2 = format!("{:?}", error2);

        assert!(debug1.contains("ConnectionFailed"));
        assert!(debug2.contains("SendFailed"));
    }

    #[test]
    fn test_zmq_orch_error_different_messages() {
        let error1 = ZmqOrchError::ConnectionFailed("Error A".to_string());
        let error2 = ZmqOrchError::ConnectionFailed("Error B".to_string());

        match (error1, error2) {
            (ZmqOrchError::ConnectionFailed(m1), ZmqOrchError::ConnectionFailed(m2)) => {
                assert_ne!(m1, m2);
            }
            _ => panic!("Error types don't match"),
        }
    }

    // ===== ZmqEndpoint tests =====

    #[test]
    fn test_zmq_endpoint_tcp_format() {
        let endpoint = ZmqEndpoint::new("tcp://127.0.0.1:5555".to_string());
        assert_eq!(endpoint.endpoint, "tcp://127.0.0.1:5555");
    }

    #[test]
    fn test_zmq_endpoint_ipc_format() {
        let endpoint = ZmqEndpoint::new("ipc:///tmp/socket".to_string());
        assert_eq!(endpoint.endpoint, "ipc:///tmp/socket");
    }

    #[test]
    fn test_zmq_endpoint_inproc_format() {
        let endpoint = ZmqEndpoint::new("inproc://test".to_string());
        assert_eq!(endpoint.endpoint, "inproc://test");
    }

    #[test]
    fn test_zmq_endpoint_clone() {
        let endpoint1 = ZmqEndpoint::new("tcp://localhost:7777".to_string());
        let endpoint2 = endpoint1.clone();

        assert_eq!(endpoint1.endpoint, endpoint2.endpoint);
    }

    #[test]
    fn test_zmq_endpoint_equality() {
        let endpoint1 = ZmqEndpoint::new("tcp://localhost:5555".to_string());
        let endpoint2 = ZmqEndpoint::new("tcp://localhost:5555".to_string());
        let endpoint3 = ZmqEndpoint::new("tcp://localhost:6666".to_string());

        assert_eq!(endpoint1, endpoint2);
        assert_ne!(endpoint1, endpoint3);
    }

    #[test]
    fn test_zmq_endpoint_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ZmqEndpoint::new("tcp://localhost:5555".to_string()));
        set.insert(ZmqEndpoint::new("tcp://localhost:6666".to_string()));
        set.insert(ZmqEndpoint::new("tcp://localhost:5555".to_string())); // duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_zmq_endpoint_debug() {
        let endpoint = ZmqEndpoint::new("tcp://localhost:5555".to_string());
        let debug_str = format!("{:?}", endpoint);

        assert!(debug_str.contains("ZmqEndpoint"));
    }

    // ===== ZmqMessage tests =====

    #[test]
    fn test_zmq_message_creation() {
        let message = ZmqMessage::new("test_topic".to_string(), vec![1, 2, 3, 4]);

        assert_eq!(message.topic, "test_topic");
        assert_eq!(message.payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_zmq_message_empty_payload() {
        let message = ZmqMessage::new("empty_topic".to_string(), vec![]);

        assert_eq!(message.topic, "empty_topic");
        assert_eq!(message.payload.len(), 0);
    }

    #[test]
    fn test_zmq_message_large_payload() {
        let payload = vec![0u8; 10000];
        let message = ZmqMessage::new("large_topic".to_string(), payload.clone());

        assert_eq!(message.payload.len(), 10000);
    }

    #[test]
    fn test_zmq_message_clone() {
        let message1 = ZmqMessage::new("topic".to_string(), vec![1, 2, 3]);
        let message2 = message1.clone();

        assert_eq!(message1.topic, message2.topic);
        assert_eq!(message1.payload, message2.payload);
    }

    #[test]
    fn test_zmq_message_different_topics() {
        let message1 = ZmqMessage::new("topic1".to_string(), vec![1, 2, 3]);
        let message2 = ZmqMessage::new("topic2".to_string(), vec![1, 2, 3]);

        assert_ne!(message1.topic, message2.topic);
        assert_eq!(message1.payload, message2.payload);
    }

    #[test]
    fn test_zmq_message_debug() {
        let message = ZmqMessage::new("test".to_string(), vec![1, 2, 3]);
        let debug_str = format!("{:?}", message);

        assert!(debug_str.contains("ZmqMessage"));
    }

    #[test]
    fn test_zmq_message_with_json_payload() {
        let json_data = r#"{"key":"value"}"#.as_bytes().to_vec();
        let message = ZmqMessage::new("json_topic".to_string(), json_data);

        assert_eq!(message.topic, "json_topic");
        assert!(!message.payload.is_empty());
    }

    #[test]
    fn test_zmq_message_with_binary_payload() {
        let binary_data = vec![0xff, 0xfe, 0xfd, 0xfc];
        let message = ZmqMessage::new("binary_topic".to_string(), binary_data.clone());

        assert_eq!(message.payload, binary_data);
    }

    // ===== Integration tests =====

    #[test]
    fn test_zmq_orch_full_lifecycle() {
        let config = ZmqOrchConfig {
            endpoint: Some("tcp://127.0.0.1:5555".to_string()),
        };
        let orch = ZmqOrch::new(config);

        assert_eq!(orch.config.endpoint, Some("tcp://127.0.0.1:5555".to_string()));
        assert_eq!(orch.stats.stats.messages_sent, 0);
    }

    #[test]
    fn test_zmq_orch_multiple_instances() {
        let config1 = ZmqOrchConfig {
            endpoint: Some("tcp://127.0.0.1:5555".to_string()),
        };
        let config2 = ZmqOrchConfig {
            endpoint: Some("tcp://127.0.0.1:6666".to_string()),
        };

        let orch1 = ZmqOrch::new(config1);
        let orch2 = ZmqOrch::new(config2);

        assert_ne!(orch1.config.endpoint, orch2.config.endpoint);
    }

    #[test]
    fn test_zmq_stats_debug() {
        let stats = ZmqStats::default();
        let debug_str = format!("{:?}", stats);

        assert!(debug_str.contains("ZmqStats"));
    }

    #[test]
    fn test_zmq_orch_stats_debug() {
        let stats = ZmqOrchStats::default();
        let debug_str = format!("{:?}", stats);

        assert!(debug_str.contains("ZmqOrchStats"));
    }
}
