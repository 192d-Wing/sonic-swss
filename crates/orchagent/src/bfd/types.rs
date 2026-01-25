//! BFD types and data structures.

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use sonic_sai::types::RawSaiObjectId;
use sonic_types::MacAddress;

/// Default BFD TX interval in milliseconds.
pub const BFD_SESSION_DEFAULT_TX_INTERVAL: u32 = 1000;

/// Default BFD RX interval in milliseconds.
pub const BFD_SESSION_DEFAULT_RX_INTERVAL: u32 = 1000;

/// Default BFD detect multiplier.
pub const BFD_SESSION_DEFAULT_DETECT_MULTIPLIER: u8 = 10;

/// Default BFD Type of Service value.
pub const BFD_SESSION_DEFAULT_TOS: u8 = 192;

/// BFD source port range start.
pub const BFD_SRCPORT_INIT: u16 = 49152;

/// BFD source port range end.
pub const BFD_SRCPORT_MAX: u16 = 65535;

/// Number of source port retry attempts.
pub const NUM_BFD_SRCPORT_RETRIES: u8 = 3;

/// BFD session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BfdSessionState {
    /// Administratively disabled.
    AdminDown,
    /// Not yet established.
    #[default]
    Down,
    /// Initializing.
    Init,
    /// Session operational.
    Up,
}

impl BfdSessionState {
    /// Returns the SAI value for this state.
    pub fn sai_value(&self) -> i32 {
        match self {
            Self::AdminDown => 0, // SAI_BFD_SESSION_STATE_ADMIN_DOWN
            Self::Down => 1,      // SAI_BFD_SESSION_STATE_DOWN
            Self::Init => 2,      // SAI_BFD_SESSION_STATE_INIT
            Self::Up => 3,        // SAI_BFD_SESSION_STATE_UP
        }
    }

    /// Creates a state from SAI value.
    pub fn from_sai_value(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::AdminDown),
            1 => Some(Self::Down),
            2 => Some(Self::Init),
            3 => Some(Self::Up),
            _ => None,
        }
    }

    /// Returns the state DB string representation.
    pub fn state_db_string(&self) -> &'static str {
        match self {
            Self::AdminDown => "Admin_Down",
            Self::Down => "Down",
            Self::Init => "Init",
            Self::Up => "Up",
        }
    }
}

impl FromStr for BfdSessionState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin_down" | "admindown" => Ok(Self::AdminDown),
            "down" => Ok(Self::Down),
            "init" => Ok(Self::Init),
            "up" => Ok(Self::Up),
            _ => Err(format!("Unknown BFD session state: {}", s)),
        }
    }
}

impl fmt::Display for BfdSessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.state_db_string())
    }
}

/// BFD session type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BfdSessionType {
    /// Demand mode, active role.
    DemandActive,
    /// Demand mode, passive role.
    DemandPassive,
    /// Asynchronous mode, active role.
    #[default]
    AsyncActive,
    /// Asynchronous mode, passive role.
    AsyncPassive,
}

impl BfdSessionType {
    /// Returns the SAI session type value.
    pub fn sai_value(&self) -> i32 {
        match self {
            Self::DemandActive => 0,  // SAI_BFD_SESSION_TYPE_DEMAND_ACTIVE
            Self::DemandPassive => 1, // SAI_BFD_SESSION_TYPE_DEMAND_PASSIVE
            Self::AsyncActive => 2,   // SAI_BFD_SESSION_TYPE_ASYNC_ACTIVE
            Self::AsyncPassive => 3,  // SAI_BFD_SESSION_TYPE_ASYNC_PASSIVE
        }
    }

    /// Returns the config string representation.
    pub fn config_string(&self) -> &'static str {
        match self {
            Self::DemandActive => "demand_active",
            Self::DemandPassive => "demand_passive",
            Self::AsyncActive => "async_active",
            Self::AsyncPassive => "async_passive",
        }
    }

    /// Returns true if this is an active session type.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::DemandActive | Self::AsyncActive)
    }

    /// Returns true if this is an async session type.
    pub fn is_async(&self) -> bool {
        matches!(self, Self::AsyncActive | Self::AsyncPassive)
    }
}

impl FromStr for BfdSessionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "demand_active" => Ok(Self::DemandActive),
            "demand_passive" => Ok(Self::DemandPassive),
            "async_active" => Ok(Self::AsyncActive),
            "async_passive" => Ok(Self::AsyncPassive),
            _ => Err(format!("Unknown BFD session type: {}", s)),
        }
    }
}

impl fmt::Display for BfdSessionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.config_string())
    }
}

/// BFD session update notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BfdUpdate {
    /// Peer identifier (state DB key).
    pub peer: String,
    /// BFD session state.
    pub state: BfdSessionState,
}

impl BfdUpdate {
    /// Creates a new BFD update.
    pub fn new(peer: impl Into<String>, state: BfdSessionState) -> Self {
        Self {
            peer: peer.into(),
            state,
        }
    }
}

/// BFD session key (parsed from config key).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BfdSessionKey {
    /// VRF name ("default" for default VRF).
    pub vrf: String,
    /// Interface name (optional, for single-hop BFD).
    pub interface: Option<String>,
    /// Peer IP address.
    pub peer_ip: IpAddr,
}

impl BfdSessionKey {
    /// Creates a new session key.
    pub fn new(vrf: impl Into<String>, interface: Option<String>, peer_ip: IpAddr) -> Self {
        Self {
            vrf: vrf.into(),
            interface,
            peer_ip,
        }
    }

    /// Parses a config key in format "vrf:interface:peer_ip" or "vrf::peer_ip".
    pub fn parse(key: &str) -> Option<Self> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 3 {
            return None;
        }

        let vrf = parts[0].to_string();
        let interface = if parts[1].is_empty() {
            None
        } else {
            Some(parts[1].to_string())
        };
        let peer_ip = parts[2].parse().ok()?;

        Some(Self {
            vrf,
            interface,
            peer_ip,
        })
    }

    /// Returns the config key format.
    pub fn to_config_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.vrf,
            self.interface.as_deref().unwrap_or(""),
            self.peer_ip
        )
    }

    /// Returns the state DB key format (using | delimiter).
    pub fn to_state_db_key(&self) -> String {
        match &self.interface {
            Some(intf) => format!("{}|{}|{}", self.vrf, intf, self.peer_ip),
            None => format!("{}|{}", self.vrf, self.peer_ip),
        }
    }

    /// Returns true if this is a multihop session (no interface specified).
    pub fn is_multihop(&self) -> bool {
        self.interface.is_none()
    }
}

impl fmt::Display for BfdSessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_config_key())
    }
}

/// BFD session configuration.
#[derive(Debug, Clone)]
pub struct BfdSessionConfig {
    /// Session key.
    pub key: BfdSessionKey,
    /// TX interval in milliseconds.
    pub tx_interval: u32,
    /// RX interval in milliseconds.
    pub rx_interval: u32,
    /// Detect multiplier.
    pub multiplier: u8,
    /// Local IP address.
    pub local_addr: Option<IpAddr>,
    /// Session type.
    pub session_type: BfdSessionType,
    /// Type of Service.
    pub tos: u8,
    /// Whether to shutdown during TSA.
    pub shutdown_bfd_during_tsa: bool,
    /// Local MAC address (for single-hop).
    pub local_mac: Option<MacAddress>,
    /// Peer MAC address (for single-hop).
    pub peer_mac: Option<MacAddress>,
}

impl BfdSessionConfig {
    /// Creates a new session config with default values.
    pub fn new(key: BfdSessionKey) -> Self {
        Self {
            key,
            tx_interval: BFD_SESSION_DEFAULT_TX_INTERVAL,
            rx_interval: BFD_SESSION_DEFAULT_RX_INTERVAL,
            multiplier: BFD_SESSION_DEFAULT_DETECT_MULTIPLIER,
            local_addr: None,
            session_type: BfdSessionType::default(),
            tos: BFD_SESSION_DEFAULT_TOS,
            shutdown_bfd_during_tsa: false,
            local_mac: None,
            peer_mac: None,
        }
    }

    /// Sets the TX interval.
    pub fn with_tx_interval(mut self, interval: u32) -> Self {
        self.tx_interval = interval;
        self
    }

    /// Sets the RX interval.
    pub fn with_rx_interval(mut self, interval: u32) -> Self {
        self.rx_interval = interval;
        self
    }

    /// Sets the detect multiplier.
    pub fn with_multiplier(mut self, multiplier: u8) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Sets the local address.
    pub fn with_local_addr(mut self, addr: IpAddr) -> Self {
        self.local_addr = Some(addr);
        self
    }

    /// Sets the session type.
    pub fn with_session_type(mut self, session_type: BfdSessionType) -> Self {
        self.session_type = session_type;
        self
    }

    /// Sets the TOS value.
    pub fn with_tos(mut self, tos: u8) -> Self {
        self.tos = tos;
        self
    }

    /// Sets the shutdown during TSA flag.
    pub fn with_shutdown_bfd_during_tsa(mut self, shutdown: bool) -> Self {
        self.shutdown_bfd_during_tsa = shutdown;
        self
    }

    /// Parses a field-value pair from AppDB.
    pub fn parse_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        match field {
            "tx_interval" => {
                self.tx_interval = value
                    .parse()
                    .map_err(|_| format!("Invalid tx_interval: {}", value))?;
            }
            "rx_interval" => {
                self.rx_interval = value
                    .parse()
                    .map_err(|_| format!("Invalid rx_interval: {}", value))?;
            }
            "multiplier" => {
                self.multiplier = value
                    .parse()
                    .map_err(|_| format!("Invalid multiplier: {}", value))?;
            }
            "local_addr" => {
                self.local_addr = Some(
                    value
                        .parse()
                        .map_err(|_| format!("Invalid local_addr: {}", value))?,
                );
            }
            "type" => {
                self.session_type = value.parse()?;
            }
            "tos" => {
                self.tos = value
                    .parse()
                    .map_err(|_| format!("Invalid tos: {}", value))?;
            }
            "shutdown_bfd_during_tsa" => {
                self.shutdown_bfd_during_tsa = value == "true" || value == "1";
            }
            "src_mac" => {
                self.local_mac = Some(
                    value
                        .parse()
                        .map_err(|_| format!("Invalid src_mac: {}", value))?,
                );
            }
            "dst_mac" => {
                self.peer_mac = Some(
                    value
                        .parse()
                        .map_err(|_| format!("Invalid dst_mac: {}", value))?,
                );
            }
            _ => {
                // Ignore unknown fields
            }
        }
        Ok(())
    }
}

/// BFD session internal tracking data.
#[derive(Debug, Clone)]
pub struct BfdSessionInfo {
    /// SAI session object ID.
    pub sai_oid: RawSaiObjectId,
    /// Current state.
    pub state: BfdSessionState,
    /// State DB key.
    pub state_db_key: String,
    /// Configuration.
    pub config: BfdSessionConfig,
    /// Local discriminator.
    pub local_discriminator: u32,
    /// Source port.
    pub src_port: u16,
}

impl BfdSessionInfo {
    /// Creates a new session info.
    pub fn new(
        sai_oid: RawSaiObjectId,
        state_db_key: String,
        config: BfdSessionConfig,
        local_discriminator: u32,
        src_port: u16,
    ) -> Self {
        Self {
            sai_oid,
            state: BfdSessionState::Down,
            state_db_key,
            config,
            local_discriminator,
            src_port,
        }
    }

    /// Updates the session state.
    pub fn set_state(&mut self, state: BfdSessionState) {
        self.state = state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_bfd_session_state() {
        assert_eq!(BfdSessionState::Down.sai_value(), 1);
        assert_eq!(BfdSessionState::Up.sai_value(), 3);
        assert_eq!(
            BfdSessionState::from_sai_value(3),
            Some(BfdSessionState::Up)
        );
        assert_eq!(BfdSessionState::from_sai_value(99), None);

        assert_eq!(
            "up".parse::<BfdSessionState>().unwrap(),
            BfdSessionState::Up
        );
        assert_eq!(
            "Down".parse::<BfdSessionState>().unwrap(),
            BfdSessionState::Down
        );
    }

    #[test]
    fn test_bfd_session_type() {
        assert_eq!(BfdSessionType::AsyncActive.sai_value(), 2);
        assert!(BfdSessionType::AsyncActive.is_active());
        assert!(BfdSessionType::AsyncActive.is_async());
        assert!(!BfdSessionType::DemandPassive.is_active());

        assert_eq!(
            "async_active".parse::<BfdSessionType>().unwrap(),
            BfdSessionType::AsyncActive
        );
    }

    #[test]
    fn test_bfd_session_key_parse() {
        // Multihop session (no interface)
        let key = BfdSessionKey::parse("default::10.0.0.1").unwrap();
        assert_eq!(key.vrf, "default");
        assert!(key.interface.is_none());
        assert_eq!(key.peer_ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(key.is_multihop());

        // Single-hop session with interface
        let key = BfdSessionKey::parse("Vrf1:Ethernet0:192.168.1.1").unwrap();
        assert_eq!(key.vrf, "Vrf1");
        assert_eq!(key.interface, Some("Ethernet0".to_string()));
        assert!(!key.is_multihop());

        // Invalid key
        assert!(BfdSessionKey::parse("invalid").is_none());
    }

    #[test]
    fn test_bfd_session_key_to_keys() {
        let key = BfdSessionKey::new(
            "default",
            Some("Ethernet0".to_string()),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        );

        assert_eq!(key.to_config_key(), "default:Ethernet0:10.0.0.1");
        assert_eq!(key.to_state_db_key(), "default|Ethernet0|10.0.0.1");

        // Multihop
        let key = BfdSessionKey::new("default", None, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(key.to_config_key(), "default::10.0.0.1");
        assert_eq!(key.to_state_db_key(), "default|10.0.0.1");
    }

    #[test]
    fn test_bfd_session_config() {
        let key = BfdSessionKey::parse("default::10.0.0.1").unwrap();
        let config = BfdSessionConfig::new(key)
            .with_tx_interval(500)
            .with_multiplier(5)
            .with_session_type(BfdSessionType::AsyncPassive);

        assert_eq!(config.tx_interval, 500);
        assert_eq!(config.rx_interval, BFD_SESSION_DEFAULT_RX_INTERVAL);
        assert_eq!(config.multiplier, 5);
        assert_eq!(config.session_type, BfdSessionType::AsyncPassive);
    }

    #[test]
    fn test_bfd_session_config_parse_field() {
        let key = BfdSessionKey::parse("default::10.0.0.1").unwrap();
        let mut config = BfdSessionConfig::new(key);

        config.parse_field("tx_interval", "500").unwrap();
        assert_eq!(config.tx_interval, 500);

        config.parse_field("type", "demand_active").unwrap();
        assert_eq!(config.session_type, BfdSessionType::DemandActive);

        config
            .parse_field("shutdown_bfd_during_tsa", "true")
            .unwrap();
        assert!(config.shutdown_bfd_during_tsa);

        config.parse_field("local_addr", "192.168.1.1").unwrap();
        assert_eq!(
            config.local_addr,
            Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))
        );
    }

    #[test]
    fn test_bfd_update() {
        let update = BfdUpdate::new("default|Ethernet0|10.0.0.1", BfdSessionState::Up);
        assert_eq!(update.peer, "default|Ethernet0|10.0.0.1");
        assert_eq!(update.state, BfdSessionState::Up);
    }
}
