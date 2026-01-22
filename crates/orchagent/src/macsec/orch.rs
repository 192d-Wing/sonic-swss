//! MACsec orchestration logic.

use super::types::{MacsecPort, MacsecSc, MacsecSa, MacsecStats, Sci};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum MacsecOrchError {
    PortNotFound(String),
    ScNotFound(Sci),
    SaNotFound(u8),
    InvalidAn(u8),
    InvalidCipherSuite(String),
    InvalidKey(String),
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct MacsecOrchConfig {
    pub enable_xpn: bool,
    pub default_cipher_suite: String,
}

#[derive(Debug, Clone, Default)]
pub struct MacsecOrchStats {
    pub stats: MacsecStats,
    pub errors: u64,
}

pub trait MacsecOrchCallbacks: Send + Sync {
    fn on_port_enabled(&self, port: &MacsecPort);
    fn on_port_disabled(&self, port_name: &str);
    fn on_sc_created(&self, sc: &MacsecSc);
    fn on_sc_removed(&self, sci: Sci);
    fn on_sa_created(&self, sa: &MacsecSa);
    fn on_sa_removed(&self, an: u8);
}

pub struct MacsecOrch {
    config: MacsecOrchConfig,
    stats: MacsecOrchStats,
    ports: HashMap<String, MacsecPort>,
    scs: HashMap<Sci, MacsecSc>,
}

impl MacsecOrch {
    pub fn new(config: MacsecOrchConfig) -> Self {
        Self {
            config,
            stats: MacsecOrchStats::default(),
            ports: HashMap::new(),
            scs: HashMap::new(),
        }
    }

    pub fn get_port(&self, name: &str) -> Option<&MacsecPort> {
        self.ports.get(name)
    }

    pub fn get_sc(&self, sci: Sci) -> Option<&MacsecSc> {
        self.scs.get(&sci)
    }

    pub fn stats(&self) -> &MacsecOrchStats {
        &self.stats
    }
}
