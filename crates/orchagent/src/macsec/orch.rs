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
    sas: HashMap<(Sci, u8), MacsecSa>, // (SCI, AN) composite key
}

impl MacsecOrch {
    pub fn new(config: MacsecOrchConfig) -> Self {
        Self {
            config,
            stats: MacsecOrchStats::default(),
            ports: HashMap::new(),
            scs: HashMap::new(),
            sas: HashMap::new(),
        }
    }

    pub fn get_port(&self, name: &str) -> Option<&MacsecPort> {
        self.ports.get(name)
    }

    pub fn add_port(&mut self, port: MacsecPort) -> Result<(), MacsecOrchError> {
        let name = port.port_name.clone();

        if self.ports.contains_key(&name) {
            return self.update_port(port);
        }

        if port.enable {
            self.stats.stats.ports_enabled = self.stats.stats.ports_enabled.saturating_add(1);
        }

        self.ports.insert(name, port);
        Ok(())
    }

    pub fn update_port(&mut self, port: MacsecPort) -> Result<(), MacsecOrchError> {
        let name = port.port_name.clone();

        let old_port = self.ports.get(&name)
            .ok_or_else(|| MacsecOrchError::PortNotFound(name.clone()))?;

        // Update enabled counter
        match (old_port.enable, port.enable) {
            (false, true) => {
                self.stats.stats.ports_enabled = self.stats.stats.ports_enabled.saturating_add(1);
            }
            (true, false) => {
                self.stats.stats.ports_enabled = self.stats.stats.ports_enabled.saturating_sub(1);
            }
            _ => {}
        }

        self.ports.insert(name, port);
        Ok(())
    }

    pub fn remove_port(&mut self, name: &str) -> Result<MacsecPort, MacsecOrchError> {
        let port = self.ports.remove(name)
            .ok_or_else(|| MacsecOrchError::PortNotFound(name.to_string()))?;

        if port.enable {
            self.stats.stats.ports_enabled = self.stats.stats.ports_enabled.saturating_sub(1);
        }

        Ok(port)
    }

    pub fn get_sc(&self, sci: Sci) -> Option<&MacsecSc> {
        self.scs.get(&sci)
    }

    pub fn add_sc(&mut self, sc: MacsecSc) -> Result<(), MacsecOrchError> {
        let sci = sc.sci;

        if self.scs.contains_key(&sci) {
            return Err(MacsecOrchError::SaiError("SC already exists".to_string()));
        }

        self.stats.stats.scs_created = self.stats.stats.scs_created.saturating_add(1);
        self.scs.insert(sci, sc);

        Ok(())
    }

    pub fn remove_sc(&mut self, sci: Sci) -> Result<MacsecSc, MacsecOrchError> {
        // Remove all SAs for this SC first
        let sas_to_remove: Vec<_> = self.sas
            .keys()
            .filter(|(sc_sci, _)| *sc_sci == sci)
            .cloned()
            .collect();

        for key in sas_to_remove {
            self.sas.remove(&key);
        }

        self.scs.remove(&sci)
            .ok_or_else(|| MacsecOrchError::ScNotFound(sci))
    }

    pub fn get_sa(&self, sci: Sci, an: u8) -> Option<&MacsecSa> {
        self.sas.get(&(sci, an))
    }

    pub fn add_sa(&mut self, sci: Sci, sa: MacsecSa) -> Result<(), MacsecOrchError> {
        // Validate AN (0-3)
        sa.validate_an()
            .map_err(|e| MacsecOrchError::InvalidAn(sa.an))?;

        // Verify SC exists
        if !self.scs.contains_key(&sci) {
            return Err(MacsecOrchError::ScNotFound(sci));
        }

        let key = (sci, sa.an);

        if self.sas.contains_key(&key) {
            return Err(MacsecOrchError::SaiError("SA already exists".to_string()));
        }

        self.stats.stats.sas_created = self.stats.stats.sas_created.saturating_add(1);
        self.sas.insert(key, sa);

        Ok(())
    }

    pub fn remove_sa(&mut self, sci: Sci, an: u8) -> Result<MacsecSa, MacsecOrchError> {
        let key = (sci, an);
        self.sas.remove(&key)
            .ok_or_else(|| MacsecOrchError::SaNotFound(an))
    }

    pub fn get_sas_for_sc(&self, sci: Sci) -> Vec<&MacsecSa> {
        self.sas
            .iter()
            .filter(|((sc_sci, _), _)| *sc_sci == sci)
            .map(|(_, sa)| sa)
            .collect()
    }

    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    pub fn sc_count(&self) -> usize {
        self.scs.len()
    }

    pub fn sa_count(&self) -> usize {
        self.sas.len()
    }

    pub fn stats(&self) -> &MacsecOrchStats {
        &self.stats
    }
}
