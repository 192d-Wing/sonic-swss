//! TWAMP (Two-Way Active Measurement Protocol) types and structures.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpAddress;
use std::collections::HashMap;
use std::str::FromStr;

/// TWAMP session mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TwampMode {
    /// Full mode - complete TWAMP protocol.
    Full,
    /// Light mode - simplified protocol.
    Light,
}

impl TwampMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "full" => Some(Self::Full),
            "light" => Some(Self::Light),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Light => "light",
        }
    }
}

/// TWAMP session role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TwampRole {
    /// Sender - initiates test packets.
    Sender,
    /// Reflector - responds to test packets.
    Reflector,
}

impl TwampRole {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sender" => Some(Self::Sender),
            "reflector" => Some(Self::Reflector),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sender => "sender",
            Self::Reflector => "reflector",
        }
    }
}

/// TWAMP timestamp format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampFormat {
    /// NTP format.
    Ntp,
    /// PTP format.
    Ptp,
}

impl TimestampFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "NTP" => Some(Self::Ntp),
            "PTP" => Some(Self::Ptp),
            _ => None,
        }
    }
}

/// TWAMP transmission mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxMode {
    /// Send specific packet count.
    PacketNum(u32),
    /// Continuous transmission for duration.
    Continuous(u32), // monitor_time in seconds
}

/// DSCP value (0-63).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dscp(u8);

impl Dscp {
    pub fn new(value: u8) -> Result<Self, String> {
        if value <= 63 {
            Ok(Self(value))
        } else {
            Err(format!("DSCP {} exceeds maximum 63", value))
        }
    }

    pub fn value(&self) -> u8 {
        self.0
    }
}

/// Session timeout (1-10 seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTimeout(u8);

impl SessionTimeout {
    pub fn new(value: u8) -> Result<Self, String> {
        if value >= 1 && value <= 10 {
            Ok(Self(value))
        } else {
            Err(format!("Timeout {} must be 1-10 seconds", value))
        }
    }

    pub fn value(&self) -> u8 {
        self.0
    }
}

/// UDP port for TWAMP (862, 863, or >= 1025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwampUdpPort(u16);

impl TwampUdpPort {
    pub fn new(value: u16) -> Result<Self, String> {
        if value == 862 || value == 863 || value >= 1025 {
            Ok(Self(value))
        } else {
            Err(format!("UDP port {} invalid (must be 862, 863, or >= 1025)", value))
        }
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}

/// TWAMP session configuration.
#[derive(Debug, Clone)]
pub struct TwampSessionConfig {
    pub name: String,
    pub mode: TwampMode,
    pub role: TwampRole,
    pub admin_state: bool,
    pub hw_lookup: bool,
    pub vrf_id: Option<RawSaiObjectId>,
    pub src_ip: IpAddress,
    pub dst_ip: IpAddress,
    pub src_udp_port: TwampUdpPort,
    pub dst_udp_port: TwampUdpPort,
    pub padding_size: u16,
    pub dscp: Dscp,
    pub ttl: u8,
    pub timestamp_format: TimestampFormat,
    pub tx_mode: Option<TxMode>,
    pub tx_interval: Option<u32>,
    pub statistics_interval: Option<u32>,
    pub timeout: Option<SessionTimeout>,
}

impl TwampSessionConfig {
    pub fn new(name: String, mode: TwampMode, role: TwampRole) -> Self {
        Self {
            name,
            mode,
            role,
            admin_state: false,
            hw_lookup: true,
            vrf_id: None,
            src_ip: IpAddress::from_str("0.0.0.0").unwrap(),
            dst_ip: IpAddress::from_str("0.0.0.0").unwrap(),
            src_udp_port: TwampUdpPort::new(862).unwrap(),
            dst_udp_port: TwampUdpPort::new(862).unwrap(),
            padding_size: 0,
            dscp: Dscp::new(0).unwrap(),
            ttl: 255,
            timestamp_format: TimestampFormat::Ntp,
            tx_mode: None,
            tx_interval: None,
            statistics_interval: None,
            timeout: None,
        }
    }
}

/// TWAMP session entry.
#[derive(Debug, Clone)]
pub struct TwampSessionEntry {
    pub name: String,
    pub mode: TwampMode,
    pub role: TwampRole,
    pub admin_state: bool,
    pub hw_lookup: bool,
    pub vrf_id: Option<RawSaiObjectId>,
    pub src_ip: IpAddress,
    pub dst_ip: IpAddress,
    pub src_udp_port: TwampUdpPort,
    pub dst_udp_port: TwampUdpPort,
    pub padding_size: u16,
    pub dscp: Dscp,
    pub ttl: u8,
    pub timestamp_format: TimestampFormat,
    pub tx_mode: Option<TxMode>,
    pub tx_interval: Option<u32>,
    pub statistics_interval: Option<u32>,
    pub timeout: Option<SessionTimeout>,
    pub session_id: RawSaiObjectId,
}

impl TwampSessionEntry {
    pub fn from_config(config: TwampSessionConfig, session_id: RawSaiObjectId) -> Self {
        Self {
            name: config.name,
            mode: config.mode,
            role: config.role,
            admin_state: config.admin_state,
            hw_lookup: config.hw_lookup,
            vrf_id: config.vrf_id,
            src_ip: config.src_ip,
            dst_ip: config.dst_ip,
            src_udp_port: config.src_udp_port,
            dst_udp_port: config.dst_udp_port,
            padding_size: config.padding_size,
            dscp: config.dscp,
            ttl: config.ttl,
            timestamp_format: config.timestamp_format,
            tx_mode: config.tx_mode,
            tx_interval: config.tx_interval,
            statistics_interval: config.statistics_interval,
            timeout: config.timeout,
            session_id,
        }
    }
}

/// TWAMP session statistics.
#[derive(Debug, Clone, Default)]
pub struct TwampStats {
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub tx_packets: u64,
    pub tx_bytes: u64,
    pub drop_packets: u64,
    pub max_latency: u64,
    pub min_latency: u64,
    pub avg_latency: u64,
    pub max_jitter: u64,
    pub min_jitter: u64,
    pub avg_jitter: u64,
    pub avg_latency_total: u64,
    pub avg_jitter_total: u64,
}

/// TWAMP session status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwampSessionStatus {
    Inactive,
    Active,
}

impl TwampSessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Active => "active",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twamp_mode_parse() {
        assert_eq!(TwampMode::parse("full"), Some(TwampMode::Full));
        assert_eq!(TwampMode::parse("LIGHT"), Some(TwampMode::Light));
        assert_eq!(TwampMode::parse("invalid"), None);
    }

    #[test]
    fn test_twamp_role_parse() {
        assert_eq!(TwampRole::parse("sender"), Some(TwampRole::Sender));
        assert_eq!(TwampRole::parse("REFLECTOR"), Some(TwampRole::Reflector));
        assert_eq!(TwampRole::parse("invalid"), None);
    }

    #[test]
    fn test_dscp_validation() {
        assert!(Dscp::new(0).is_ok());
        assert!(Dscp::new(63).is_ok());
        assert!(Dscp::new(64).is_err());
    }

    #[test]
    fn test_timeout_validation() {
        assert!(SessionTimeout::new(0).is_err());
        assert!(SessionTimeout::new(1).is_ok());
        assert!(SessionTimeout::new(10).is_ok());
        assert!(SessionTimeout::new(11).is_err());
    }

    #[test]
    fn test_udp_port_validation() {
        assert!(TwampUdpPort::new(862).is_ok());
        assert!(TwampUdpPort::new(863).is_ok());
        assert!(TwampUdpPort::new(1024).is_err());
        assert!(TwampUdpPort::new(1025).is_ok());
        assert!(TwampUdpPort::new(5000).is_ok());
    }

    #[test]
    fn test_tx_mode() {
        let packet_mode = TxMode::PacketNum(100);
        let continuous_mode = TxMode::Continuous(60);

        assert!(matches!(packet_mode, TxMode::PacketNum(100)));
        assert!(matches!(continuous_mode, TxMode::Continuous(60)));
    }
}
