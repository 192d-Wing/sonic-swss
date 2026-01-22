//! MACsec (Media Access Control Security) types.

pub type RawSaiObjectId = u64;
pub type Sci = u64; // Secure Channel Identifier

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacsecDirection {
    Ingress,
    Egress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacsecCipherSuite {
    Gcm128,
    Gcm256,
    GcmXpn128,
    GcmXpn256,
}

#[derive(Debug, Clone)]
pub struct MacsecPort {
    pub port_name: String,
    pub enable: bool,
    pub cipher_suite: MacsecCipherSuite,
    pub enable_encrypt: bool,
    pub enable_protect: bool,
    pub enable_replay_protect: bool,
    pub replay_window: u32,
    pub send_sci: bool,
}

impl MacsecPort {
    pub fn new(port_name: String) -> Self {
        Self {
            port_name,
            enable: false,
            cipher_suite: MacsecCipherSuite::Gcm128,
            enable_encrypt: true,
            enable_protect: true,
            enable_replay_protect: false,
            replay_window: 0,
            send_sci: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MacsecSc {
    pub sci: Sci,
    pub direction: MacsecDirection,
    pub sc_oid: RawSaiObjectId,
}

impl MacsecSc {
    pub fn new(sci: Sci, direction: MacsecDirection) -> Self {
        Self {
            sci,
            direction,
            sc_oid: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MacsecSa {
    pub an: u8, // Association Number (0-3)
    pub pn: u64, // Packet Number
    pub auth_key: Vec<u8>,
    pub sak: Vec<u8>, // Secure Association Key
    pub salt: Vec<u8>,
    pub sa_oid: RawSaiObjectId,
}

impl MacsecSa {
    pub fn new(an: u8, pn: u64) -> Self {
        Self {
            an,
            pn,
            auth_key: Vec::new(),
            sak: Vec::new(),
            salt: Vec::new(),
            sa_oid: 0,
        }
    }

    pub fn validate_an(&self) -> Result<(), String> {
        if self.an > 3 {
            Err(format!("Invalid AN {}, must be 0-3", self.an))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct MacsecFlowEntry {
    pub port_name: String,
    pub direction: MacsecDirection,
    pub flow_oid: RawSaiObjectId,
    pub acl_entry_oid: RawSaiObjectId,
}

impl MacsecFlowEntry {
    pub fn new(port_name: String, direction: MacsecDirection) -> Self {
        Self {
            port_name,
            direction,
            flow_oid: 0,
            acl_entry_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MacsecStats {
    pub ports_enabled: u64,
    pub scs_created: u64,
    pub sas_created: u64,
    pub flows_created: u64,
}
