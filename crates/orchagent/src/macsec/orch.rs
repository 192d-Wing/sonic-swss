//! MACsec orchestration logic.

use super::types::{MacsecDirection, MacsecPort, MacsecSa, MacsecSc, MacsecStats, Sci};
use std::collections::HashMap;

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum MacsecOrchError {
    #[error("Port not found: {0}")]
    PortNotFound(String),
    #[error("SC not found: 0x{:x}", .0)]
    ScNotFound(Sci),
    #[error("SA not found: {0}")]
    SaNotFound(u8),
    #[error("Invalid AN: {0}")]
    InvalidAn(u8),
    #[error("Invalid cipher suite: {0}")]
    InvalidCipherSuite(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("SAI error: {0}")]
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
    #[allow(dead_code)]
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

        let old_port = self
            .ports
            .get(&name)
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
        let port = self
            .ports
            .remove(name)
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
            let audit_record =
                AuditRecord::new(AuditCategory::ResourceCreate, "MacsecOrch", "create_flow")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&format!("0x{:016x}", sci))
                    .with_object_type("macsec_sc")
                    .with_error("SC already exists");
            audit_log!(audit_record);
            return Err(MacsecOrchError::SaiError("SC already exists".to_string()));
        }

        let direction = match sc.direction {
            MacsecDirection::Ingress => "ingress",
            MacsecDirection::Egress => "egress",
        };

        let audit_record =
            AuditRecord::new(AuditCategory::ResourceCreate, "MacsecOrch", "create_flow")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(&format!("0x{:016x}", sci))
                .with_object_type("macsec_sc")
                .with_details(serde_json::json!({
                    "sci": format!("0x{:016x}", sci),
                    "direction": direction,
                }));
        audit_log!(audit_record);

        self.stats.stats.scs_created = self.stats.stats.scs_created.saturating_add(1);
        self.scs.insert(sci, sc);

        Ok(())
    }

    pub fn remove_sc(&mut self, sci: Sci) -> Result<MacsecSc, MacsecOrchError> {
        // Remove all SAs for this SC first
        let sas_to_remove: Vec<_> = self
            .sas
            .keys()
            .filter(|(sc_sci, _)| *sc_sci == sci)
            .cloned()
            .collect();

        let sa_count = sas_to_remove.len();

        for key in sas_to_remove {
            self.sas.remove(&key);
        }

        match self.scs.remove(&sci) {
            Some(sc) => {
                let direction = match sc.direction {
                    MacsecDirection::Ingress => "ingress",
                    MacsecDirection::Egress => "egress",
                };

                let audit_record =
                    AuditRecord::new(AuditCategory::ResourceDelete, "MacsecOrch", "remove_flow")
                        .with_outcome(AuditOutcome::Success)
                        .with_object_id(&format!("0x{:016x}", sci))
                        .with_object_type("macsec_sc")
                        .with_details(serde_json::json!({
                            "sci": format!("0x{:016x}", sci),
                            "direction": direction,
                            "associated_sas_removed": sa_count,
                        }));
                audit_log!(audit_record);

                Ok(sc)
            }
            None => {
                let audit_record =
                    AuditRecord::new(AuditCategory::ResourceDelete, "MacsecOrch", "remove_flow")
                        .with_outcome(AuditOutcome::Failure)
                        .with_object_id(&format!("0x{:016x}", sci))
                        .with_object_type("macsec_sc")
                        .with_error("SC not found");
                audit_log!(audit_record);

                Err(MacsecOrchError::ScNotFound(sci))
            }
        }
    }

    pub fn get_sa(&self, sci: Sci, an: u8) -> Option<&MacsecSa> {
        self.sas.get(&(sci, an))
    }

    pub fn add_sa(&mut self, sci: Sci, sa: MacsecSa) -> Result<(), MacsecOrchError> {
        // Validate AN (0-3)
        sa.validate_an()
            .map_err(|_e| MacsecOrchError::InvalidAn(sa.an))?;

        // Verify SC exists
        if !self.scs.contains_key(&sci) {
            let audit_record =
                AuditRecord::new(AuditCategory::ResourceCreate, "MacsecOrch", "create_sa")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&format!("0x{:016x}:{}", sci, sa.an))
                    .with_object_type("macsec_sa")
                    .with_error("SC not found");
            audit_log!(audit_record);
            return Err(MacsecOrchError::ScNotFound(sci));
        }

        let key = (sci, sa.an);

        if self.sas.contains_key(&key) {
            let audit_record =
                AuditRecord::new(AuditCategory::ResourceCreate, "MacsecOrch", "create_sa")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(&format!("0x{:016x}:{}", sci, sa.an))
                    .with_object_type("macsec_sa")
                    .with_error("SA already exists");
            audit_log!(audit_record);
            return Err(MacsecOrchError::SaiError("SA already exists".to_string()));
        }

        let audit_record =
            AuditRecord::new(AuditCategory::ResourceCreate, "MacsecOrch", "create_sa")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(&format!("0x{:016x}:{}", sci, sa.an))
                .with_object_type("macsec_sa")
                .with_details(serde_json::json!({
                    "sci": format!("0x{:016x}", sci),
                    "an": sa.an,
                    "packet_number": sa.pn,
                }));
        audit_log!(audit_record);

        self.stats.stats.sas_created = self.stats.stats.sas_created.saturating_add(1);
        self.sas.insert(key, sa);

        Ok(())
    }

    pub fn remove_sa(&mut self, sci: Sci, an: u8) -> Result<MacsecSa, MacsecOrchError> {
        let key = (sci, an);
        match self.sas.remove(&key) {
            Some(sa) => {
                let audit_record =
                    AuditRecord::new(AuditCategory::ResourceDelete, "MacsecOrch", "remove_sa")
                        .with_outcome(AuditOutcome::Success)
                        .with_object_id(&format!("0x{:016x}:{}", sci, an))
                        .with_object_type("macsec_sa")
                        .with_details(serde_json::json!({
                            "sci": format!("0x{:016x}", sci),
                            "an": an,
                            "packet_number": sa.pn,
                        }));
                audit_log!(audit_record);

                Ok(sa)
            }
            None => {
                let audit_record =
                    AuditRecord::new(AuditCategory::ResourceDelete, "MacsecOrch", "remove_sa")
                        .with_outcome(AuditOutcome::Failure)
                        .with_object_id(&format!("0x{:016x}:{}", sci, an))
                        .with_object_type("macsec_sa")
                        .with_error("SA not found");
                audit_log!(audit_record);

                Err(MacsecOrchError::SaNotFound(an))
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macsec::types::{MacsecCipherSuite, MacsecDirection};

    fn create_test_port(port_name: &str, enable: bool) -> MacsecPort {
        MacsecPort {
            port_name: port_name.to_string(),
            enable,
            cipher_suite: MacsecCipherSuite::Gcm128,
            enable_encrypt: true,
            enable_protect: true,
            enable_replay_protect: false,
            replay_window: 0,
            send_sci: true,
        }
    }

    fn create_test_sc(sci: Sci, direction: MacsecDirection) -> MacsecSc {
        MacsecSc::new(sci, direction)
    }

    fn create_test_sa(an: u8, pn: u64) -> MacsecSa {
        MacsecSa::new(an, pn)
    }

    #[test]
    fn test_add_port() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let port = create_test_port("Ethernet0", true);

        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.stats().stats.ports_enabled, 0);

        orch.add_port(port).unwrap();

        assert_eq!(orch.port_count(), 1);
        assert_eq!(orch.stats().stats.ports_enabled, 1);
        assert!(orch.get_port("Ethernet0").is_some());
    }

    #[test]
    fn test_update_port() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let port1 = create_test_port("Ethernet0", false);
        orch.add_port(port1).unwrap();

        assert_eq!(orch.stats().stats.ports_enabled, 0);

        // Enable the port via update
        let port2 = create_test_port("Ethernet0", true);
        orch.update_port(port2).unwrap();

        assert_eq!(orch.stats().stats.ports_enabled, 1);
        assert!(orch.get_port("Ethernet0").unwrap().enable);

        // Disable the port via update
        let port3 = create_test_port("Ethernet0", false);
        orch.update_port(port3).unwrap();

        assert_eq!(orch.stats().stats.ports_enabled, 0);
        assert!(!orch.get_port("Ethernet0").unwrap().enable);
    }

    #[test]
    fn test_remove_port() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let port = create_test_port("Ethernet0", true);

        orch.add_port(port).unwrap();
        assert_eq!(orch.port_count(), 1);
        assert_eq!(orch.stats().stats.ports_enabled, 1);

        let removed = orch.remove_port("Ethernet0").unwrap();
        assert_eq!(removed.port_name, "Ethernet0");
        assert_eq!(orch.port_count(), 0);
        assert_eq!(orch.stats().stats.ports_enabled, 0);
    }

    #[test]
    fn test_add_sc() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;
        let sc = create_test_sc(sci, MacsecDirection::Ingress);

        assert_eq!(orch.sc_count(), 0);
        assert_eq!(orch.stats().stats.scs_created, 0);

        orch.add_sc(sc).unwrap();

        assert_eq!(orch.sc_count(), 1);
        assert_eq!(orch.stats().stats.scs_created, 1);
        assert!(orch.get_sc(sci).is_some());
    }

    #[test]
    fn test_remove_sc_cascades_to_sas() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;
        let sc = create_test_sc(sci, MacsecDirection::Ingress);

        // Add SC
        orch.add_sc(sc).unwrap();

        // Add multiple SAs to the SC
        orch.add_sa(sci, create_test_sa(0, 1)).unwrap();
        orch.add_sa(sci, create_test_sa(1, 1)).unwrap();
        orch.add_sa(sci, create_test_sa(2, 1)).unwrap();

        assert_eq!(orch.sa_count(), 3);
        assert_eq!(orch.get_sas_for_sc(sci).len(), 3);

        // Remove SC should cascade to all SAs
        orch.remove_sc(sci).unwrap();

        assert_eq!(orch.sc_count(), 0);
        assert_eq!(orch.sa_count(), 0);
        assert_eq!(orch.get_sas_for_sc(sci).len(), 0);
    }

    #[test]
    fn test_add_sa() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;
        let sc = create_test_sc(sci, MacsecDirection::Ingress);

        // Add SC first
        orch.add_sc(sc).unwrap();

        // Add SA with valid AN (0-3)
        let sa = create_test_sa(0, 1);
        orch.add_sa(sci, sa).unwrap();

        assert_eq!(orch.sa_count(), 1);
        assert_eq!(orch.stats().stats.sas_created, 1);
        assert!(orch.get_sa(sci, 0).is_some());
    }

    #[test]
    fn test_add_sa_invalid_an() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;
        let sc = create_test_sc(sci, MacsecDirection::Ingress);

        // Add SC first
        orch.add_sc(sc).unwrap();

        // Try to add SA with invalid AN (> 3)
        let sa = create_test_sa(4, 1);
        let result = orch.add_sa(sci, sa);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MacsecOrchError::InvalidAn(4)));
        assert_eq!(orch.sa_count(), 0);
    }

    #[test]
    fn test_add_sa_without_sc() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;

        // Try to add SA without SC existing
        let sa = create_test_sa(0, 1);
        let result = orch.add_sa(sci, sa);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MacsecOrchError::ScNotFound(_)
        ));
        assert_eq!(orch.sa_count(), 0);
    }

    #[test]
    fn test_remove_sa() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;
        let sc = create_test_sc(sci, MacsecDirection::Ingress);

        // Add SC and SA
        orch.add_sc(sc).unwrap();
        orch.add_sa(sci, create_test_sa(0, 1)).unwrap();

        assert_eq!(orch.sa_count(), 1);

        // Remove SA
        let removed = orch.remove_sa(sci, 0).unwrap();
        assert_eq!(removed.an, 0);
        assert_eq!(orch.sa_count(), 0);
        assert!(orch.get_sa(sci, 0).is_none());
    }

    #[test]
    fn test_get_sas_for_sc() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci1: Sci = 0x0011223344556677;
        let sci2: Sci = 0x8899AABBCCDDEEFF;

        // Add two SCs
        orch.add_sc(create_test_sc(sci1, MacsecDirection::Ingress))
            .unwrap();
        orch.add_sc(create_test_sc(sci2, MacsecDirection::Egress))
            .unwrap();

        // Add SAs to first SC
        orch.add_sa(sci1, create_test_sa(0, 1)).unwrap();
        orch.add_sa(sci1, create_test_sa(1, 2)).unwrap();
        orch.add_sa(sci1, create_test_sa(2, 3)).unwrap();

        // Add SAs to second SC
        orch.add_sa(sci2, create_test_sa(0, 10)).unwrap();
        orch.add_sa(sci2, create_test_sa(1, 20)).unwrap();

        // Verify composite key lookup
        let sas_sci1 = orch.get_sas_for_sc(sci1);
        assert_eq!(sas_sci1.len(), 3);

        let sas_sci2 = orch.get_sas_for_sc(sci2);
        assert_eq!(sas_sci2.len(), 2);

        // Verify ANs are correct
        let ans_sci1: Vec<u8> = sas_sci1.iter().map(|sa| sa.an).collect();
        assert!(ans_sci1.contains(&0));
        assert!(ans_sci1.contains(&1));
        assert!(ans_sci1.contains(&2));
    }

    #[test]
    fn test_add_port_disabled() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let port = create_test_port("Ethernet0", false);

        orch.add_port(port).unwrap();

        assert_eq!(orch.port_count(), 1);
        assert_eq!(orch.stats().stats.ports_enabled, 0);
    }

    #[test]
    fn test_update_port_not_found() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let port = create_test_port("Ethernet0", true);

        let result = orch.update_port(port);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MacsecOrchError::PortNotFound(_)
        ));
    }

    #[test]
    fn test_remove_port_not_found() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());

        let result = orch.remove_port("Ethernet0");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MacsecOrchError::PortNotFound(_)
        ));
    }

    #[test]
    fn test_add_duplicate_sc_fails() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;

        orch.add_sc(create_test_sc(sci, MacsecDirection::Ingress))
            .unwrap();

        let result = orch.add_sc(create_test_sc(sci, MacsecDirection::Egress));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MacsecOrchError::SaiError(_)));
    }

    #[test]
    fn test_remove_sc_not_found() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;

        let result = orch.remove_sc(sci);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MacsecOrchError::ScNotFound(_)
        ));
    }

    #[test]
    fn test_add_duplicate_sa_fails() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;

        orch.add_sc(create_test_sc(sci, MacsecDirection::Ingress))
            .unwrap();
        orch.add_sa(sci, create_test_sa(0, 1)).unwrap();

        let result = orch.add_sa(sci, create_test_sa(0, 2));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MacsecOrchError::SaiError(_)));
    }

    #[test]
    fn test_remove_sa_not_found() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;

        let result = orch.remove_sa(sci, 0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MacsecOrchError::SaNotFound(_)
        ));
    }

    #[test]
    fn test_all_valid_ans() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let sci: Sci = 0x0011223344556677;

        orch.add_sc(create_test_sc(sci, MacsecDirection::Ingress))
            .unwrap();

        // Test all valid ANs (0-3)
        for an in 0..=3 {
            orch.add_sa(sci, create_test_sa(an, an as u64)).unwrap();
        }

        assert_eq!(orch.sa_count(), 4);
        assert_eq!(orch.stats().stats.sas_created, 4);

        // Verify all ANs are present
        for an in 0..=3 {
            assert!(orch.get_sa(sci, an).is_some());
        }
    }

    #[test]
    fn test_multiple_ports() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());

        orch.add_port(create_test_port("Ethernet0", true)).unwrap();
        orch.add_port(create_test_port("Ethernet4", true)).unwrap();
        orch.add_port(create_test_port("Ethernet8", false)).unwrap();

        assert_eq!(orch.port_count(), 3);
        assert_eq!(orch.stats().stats.ports_enabled, 2);
    }

    #[test]
    fn test_add_port_updates_existing() {
        let mut orch = MacsecOrch::new(MacsecOrchConfig::default());
        let port1 = create_test_port("Ethernet0", false);

        orch.add_port(port1).unwrap();
        assert_eq!(orch.port_count(), 1);
        assert_eq!(orch.stats().stats.ports_enabled, 0);

        // Adding with same name should update
        let port2 = create_test_port("Ethernet0", true);
        orch.add_port(port2).unwrap();

        assert_eq!(orch.port_count(), 1);
        assert_eq!(orch.stats().stats.ports_enabled, 1);
        assert!(orch.get_port("Ethernet0").unwrap().enable);
    }
}
