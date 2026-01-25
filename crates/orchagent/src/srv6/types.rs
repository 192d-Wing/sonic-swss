//! SRv6 (Segment Routing over IPv6) types.

use std::collections::HashMap;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Srv6Sid {
    pub sid: String,
}

impl Srv6Sid {
    pub fn new(sid: String) -> Self {
        Self { sid }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        // Validate IPv6 format
        if s.contains(':') {
            Ok(Self { sid: s.to_string() })
        } else {
            Err(format!("Invalid SRv6 SID format: {}", s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.sid
    }
}

impl std::fmt::Display for Srv6Sid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.sid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Srv6EndpointBehavior {
    End,
    EndX,
    EndT,
    EndDx6,
    EndDx4,
    EndDt6,
    EndDt4,
    EndDt46,
    EndB6,
    EndB6Encaps,
    Usp,
    Usd,
}

#[derive(Debug, Clone)]
pub struct Srv6LocalSidConfig {
    pub sid: Srv6Sid,
    pub endpoint_behavior: Srv6EndpointBehavior,
    pub next_hop: Option<String>,
    pub vrf: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Srv6LocalSidEntry {
    pub config: Srv6LocalSidConfig,
    pub sid_oid: RawSaiObjectId,
}

impl Srv6LocalSidEntry {
    pub fn new(config: Srv6LocalSidConfig) -> Self {
        Self { config, sid_oid: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct Srv6SidListConfig {
    pub name: String,
    pub sids: Vec<Srv6Sid>,
}

#[derive(Debug, Clone)]
pub struct Srv6SidListEntry {
    pub config: Srv6SidListConfig,
    pub sidlist_oid: RawSaiObjectId,
}

impl Srv6SidListEntry {
    pub fn new(config: Srv6SidListConfig) -> Self {
        Self {
            config,
            sidlist_oid: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Srv6EncapMode {
    Inline,
    Insert,
    Encaps,
}

#[derive(Debug, Clone)]
pub struct Srv6NextHopConfig {
    pub next_hop: String,
    pub sidlist: String,
    pub encap_mode: Srv6EncapMode,
}

#[derive(Debug, Clone)]
pub struct Srv6NextHopEntry {
    pub config: Srv6NextHopConfig,
    pub nh_oid: RawSaiObjectId,
}

impl Srv6NextHopEntry {
    pub fn new(config: Srv6NextHopConfig) -> Self {
        Self { config, nh_oid: 0 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Srv6Stats {
    pub local_sids_created: u64,
    pub sidlists_created: u64,
    pub nexthops_created: u64,
}
