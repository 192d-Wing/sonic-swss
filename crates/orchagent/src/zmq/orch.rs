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
