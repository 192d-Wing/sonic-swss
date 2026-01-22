//! ZMQ messaging types for SONiC event notification.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ZmqEndpoint {
    pub endpoint: String,
}

impl ZmqEndpoint {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }
}

#[derive(Debug, Clone)]
pub struct ZmqMessage {
    pub topic: String,
    pub payload: Vec<u8>,
}

impl ZmqMessage {
    pub fn new(topic: String, payload: Vec<u8>) -> Self {
        Self { topic, payload }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ZmqStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub errors: u64,
}
