//! Tunnel decapsulation types and structures.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpPrefix;
use std::collections::HashMap;

/// Tunnel termination type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TunnelTermType {
    P2P,   // Point-to-Point
    P2MP,  // Point-to-Multipoint
    MP2MP, // Multipoint-to-Multipoint
}

impl TunnelTermType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "P2P" => Some(Self::P2P),
            "P2MP" => Some(Self::P2MP),
            "MP2MP" => Some(Self::MP2MP),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::P2P => "P2P",
            Self::P2MP => "P2MP",
            Self::MP2MP => "MP2MP",
        }
    }
}

/// Tunnel mode for DSCP/TTL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelMode {
    Uniform, // Copy from outer header
    Pipe,    // Preserve inner header
}

impl TunnelMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "uniform" => Some(Self::Uniform),
            "pipe" => Some(Self::Pipe),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
            Self::Pipe => "pipe",
        }
    }
}

/// ECN mode for tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcnMode {
    CopyFromOuter,
    Standard,
}

impl EcnMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "copy_from_outer" => Some(Self::CopyFromOuter),
            "standard" => Some(Self::Standard),
            _ => None,
        }
    }
}

/// Tunnel subnet type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubnetType {
    Vlan,
    Vip,
}

impl SubnetType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vlan" => Some(Self::Vlan),
            "vip" => Some(Self::Vip),
            _ => None,
        }
    }
}

/// Tunnel decap configuration (simplified for orchestration).
#[derive(Debug, Clone)]
pub struct TunnelDecapConfig {
    pub tunnel_name: String,
    pub tunnel_type: String,
}

impl TunnelDecapConfig {
    pub fn new(tunnel_name: String, tunnel_type: String) -> Self {
        Self {
            tunnel_name,
            tunnel_type,
        }
    }
}

/// Tunnel decap entry.
#[derive(Debug, Clone)]
pub struct TunnelDecapEntry {
    pub tunnel_name: String,
    pub tunnel_id: RawSaiObjectId,
    pub tunnel_type: String,
    pub term_entries: HashMap<String, RawSaiObjectId>,
}

impl TunnelDecapEntry {
    pub fn from_config(config: TunnelDecapConfig, tunnel_id: RawSaiObjectId) -> Self {
        Self {
            tunnel_name: config.tunnel_name,
            tunnel_id,
            tunnel_type: config.tunnel_type,
            term_entries: HashMap::new(),
        }
    }
}

/// Tunnel configuration.
#[derive(Debug, Clone)]
pub struct TunnelConfig {
    pub name: String,
    pub tunnel_type: String, // "IPINIP" only
    pub dscp_mode: TunnelMode,
    pub ecn_mode: EcnMode,
    pub encap_ecn_mode: EcnMode,
    pub ttl_mode: TunnelMode,
    pub encap_tc_to_dscp_map_id: Option<RawSaiObjectId>,
    pub encap_tc_to_queue_map_id: Option<RawSaiObjectId>,
}

impl TunnelConfig {
    pub fn new(name: String) -> Self {
        Self {
            name,
            tunnel_type: "IPINIP".to_string(),
            dscp_mode: TunnelMode::Uniform,
            ecn_mode: EcnMode::Standard,
            encap_ecn_mode: EcnMode::Standard,
            ttl_mode: TunnelMode::Uniform,
            encap_tc_to_dscp_map_id: None,
            encap_tc_to_queue_map_id: None,
        }
    }
}

/// Tunnel entry.
#[derive(Debug, Clone)]
pub struct TunnelEntry {
    pub tunnel_id: RawSaiObjectId,
    pub overlay_intf_id: RawSaiObjectId,
    pub ref_count: i32,
    pub tunnel_term_info: HashMap<IpPrefix, TunnelTermEntry>,
    pub tunnel_type: String,
    pub dscp_mode: TunnelMode,
    pub ecn_mode: EcnMode,
    pub encap_ecn_mode: EcnMode,
    pub ttl_mode: TunnelMode,
    pub encap_tc_to_dscp_map_id: RawSaiObjectId,
    pub encap_tc_to_queue_map_id: RawSaiObjectId,
}

impl TunnelEntry {
    pub fn new(
        config: TunnelConfig,
        tunnel_id: RawSaiObjectId,
        overlay_intf_id: RawSaiObjectId,
    ) -> Self {
        Self {
            tunnel_id,
            overlay_intf_id,
            ref_count: 1,
            tunnel_term_info: HashMap::new(),
            tunnel_type: config.tunnel_type,
            dscp_mode: config.dscp_mode,
            ecn_mode: config.ecn_mode,
            encap_ecn_mode: config.encap_ecn_mode,
            ttl_mode: config.ttl_mode,
            encap_tc_to_dscp_map_id: config.encap_tc_to_dscp_map_id.unwrap_or(0),
            encap_tc_to_queue_map_id: config.encap_tc_to_queue_map_id.unwrap_or(0),
        }
    }
}

/// Tunnel termination entry.
#[derive(Debug, Clone)]
pub struct TunnelTermEntry {
    pub tunnel_term_id: RawSaiObjectId,
    pub src_ip: String,
    pub dst_ip: String,
    pub term_type: TunnelTermType,
    pub subnet_type: Option<SubnetType>,
}

impl TunnelTermEntry {
    pub fn new(
        tunnel_term_id: RawSaiObjectId,
        src_ip: String,
        dst_ip: String,
        term_type: TunnelTermType,
    ) -> Self {
        Self {
            tunnel_term_id,
            src_ip,
            dst_ip,
            term_type,
            subnet_type: None,
        }
    }
}

/// Next hop tunnel entry.
#[derive(Debug, Clone)]
pub struct NexthopTunnel {
    pub nh_id: RawSaiObjectId,
    pub ref_count: u32,
}

impl NexthopTunnel {
    pub fn new(nh_id: RawSaiObjectId) -> Self {
        Self {
            nh_id,
            ref_count: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_term_type() {
        assert_eq!(TunnelTermType::parse("P2P"), Some(TunnelTermType::P2P));
        assert_eq!(TunnelTermType::parse("p2mp"), Some(TunnelTermType::P2MP));
        assert_eq!(TunnelTermType::parse("MP2MP"), Some(TunnelTermType::MP2MP));
    }

    #[test]
    fn test_tunnel_mode() {
        assert_eq!(TunnelMode::parse("uniform"), Some(TunnelMode::Uniform));
        assert_eq!(TunnelMode::parse("PIPE"), Some(TunnelMode::Pipe));
    }

    #[test]
    fn test_ecn_mode() {
        assert_eq!(
            EcnMode::parse("copy_from_outer"),
            Some(EcnMode::CopyFromOuter)
        );
        assert_eq!(EcnMode::parse("standard"), Some(EcnMode::Standard));
    }
}
