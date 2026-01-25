//! Port type definitions for SONiC switch ports.

use crate::ParseError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Type of switch port.
///
/// Corresponds to SAI port types used in the SONiC control plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortType {
    /// CPU port for control plane traffic.
    Cpu,
    /// Physical front-panel port.
    Phy,
    /// Management port (out-of-band).
    Mgmt,
    /// Loopback port.
    Loopback,
    /// VLAN interface (SVI).
    Vlan,
    /// Link Aggregation Group (LAG/Port-channel).
    Lag,
    /// Tunnel port (VxLAN, IPinIP, etc.).
    Tunnel,
    /// Sub-port (on a breakout port).
    Subport,
    /// System port (VOQ/distributed systems).
    System,
    /// Recycle port (internal).
    Recycle,
    /// Inband port (internal).
    Inband,
}

impl PortType {
    /// Returns true if this is a physical port type.
    pub const fn is_physical(&self) -> bool {
        matches!(self, PortType::Phy | PortType::Subport)
    }

    /// Returns true if this is a logical port type.
    pub const fn is_logical(&self) -> bool {
        matches!(
            self,
            PortType::Vlan | PortType::Lag | PortType::Tunnel | PortType::Loopback
        )
    }

    /// Returns true if this is an internal/system port type.
    pub const fn is_internal(&self) -> bool {
        matches!(
            self,
            PortType::Cpu | PortType::System | PortType::Recycle | PortType::Inband
        )
    }
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PortType::Cpu => "cpu",
            PortType::Phy => "phy",
            PortType::Mgmt => "mgmt",
            PortType::Loopback => "loopback",
            PortType::Vlan => "vlan",
            PortType::Lag => "lag",
            PortType::Tunnel => "tunnel",
            PortType::Subport => "subport",
            PortType::System => "system",
            PortType::Recycle => "recycle",
            PortType::Inband => "inband",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for PortType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cpu" => Ok(PortType::Cpu),
            "phy" => Ok(PortType::Phy),
            "mgmt" => Ok(PortType::Mgmt),
            "loopback" => Ok(PortType::Loopback),
            "vlan" => Ok(PortType::Vlan),
            "lag" => Ok(PortType::Lag),
            "tunnel" => Ok(PortType::Tunnel),
            "subport" => Ok(PortType::Subport),
            "system" => Ok(PortType::System),
            "recycle" => Ok(PortType::Recycle),
            "inband" => Ok(PortType::Inband),
            _ => Err(ParseError::InvalidPortType(s.to_string())),
        }
    }
}

/// Role of a port in the switch fabric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortRole {
    /// External facing port (default).
    #[default]
    External,
    /// Internal fabric port.
    Internal,
    /// Inband management port.
    Inband,
    /// Recycle port.
    Recycle,
    /// DPC (Data Plane Control) port.
    Dpc,
}

impl fmt::Display for PortRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PortRole::External => "Ext",
            PortRole::Internal => "Int",
            PortRole::Inband => "Inb",
            PortRole::Recycle => "Rec",
            PortRole::Dpc => "Dpc",
        };
        write!(f, "{}", s)
    }
}

/// Administrative state of a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdminState {
    /// Port is administratively down (default for new ports).
    #[default]
    Down,
    /// Port is administratively up.
    Up,
}

impl AdminState {
    /// Returns true if the port is administratively up.
    pub const fn is_up(&self) -> bool {
        matches!(self, AdminState::Up)
    }

    /// Returns true if the port is administratively down.
    pub const fn is_down(&self) -> bool {
        matches!(self, AdminState::Down)
    }
}

impl fmt::Display for AdminState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdminState::Up => write!(f, "up"),
            AdminState::Down => write!(f, "down"),
        }
    }
}

impl FromStr for AdminState {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "up" => Ok(AdminState::Up),
            "down" => Ok(AdminState::Down),
            _ => Err(ParseError::InvalidPortType(format!(
                "invalid admin state: {}",
                s
            ))),
        }
    }
}

/// Operational state of a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperState {
    /// Port is operationally down (default).
    #[default]
    Down,
    /// Port is operationally up.
    Up,
    /// Port state is unknown/not available.
    Unknown,
    /// Port is in testing mode.
    Testing,
}

impl OperState {
    /// Returns true if the port is operationally up.
    pub const fn is_up(&self) -> bool {
        matches!(self, OperState::Up)
    }

    /// Returns true if the port is operationally down.
    pub const fn is_down(&self) -> bool {
        matches!(self, OperState::Down)
    }
}

impl fmt::Display for OperState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperState::Up => write!(f, "up"),
            OperState::Down => write!(f, "down"),
            OperState::Unknown => write!(f, "unknown"),
            OperState::Testing => write!(f, "testing"),
        }
    }
}

impl FromStr for OperState {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "up" => Ok(OperState::Up),
            "down" => Ok(OperState::Down),
            "unknown" => Ok(OperState::Unknown),
            "testing" => Ok(OperState::Testing),
            _ => Err(ParseError::InvalidPortType(format!(
                "invalid oper state: {}",
                s
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_port_type_classification() {
        assert!(PortType::Phy.is_physical());
        assert!(PortType::Subport.is_physical());
        assert!(!PortType::Vlan.is_physical());

        assert!(PortType::Vlan.is_logical());
        assert!(PortType::Lag.is_logical());
        assert!(!PortType::Phy.is_logical());

        assert!(PortType::Cpu.is_internal());
        assert!(PortType::System.is_internal());
        assert!(!PortType::Phy.is_internal());
    }

    #[test]
    fn test_port_type_parse() {
        assert_eq!("phy".parse::<PortType>().unwrap(), PortType::Phy);
        assert_eq!("PHY".parse::<PortType>().unwrap(), PortType::Phy);
        assert_eq!("lag".parse::<PortType>().unwrap(), PortType::Lag);
    }

    #[test]
    fn test_admin_state() {
        assert!(AdminState::Up.is_up());
        assert!(!AdminState::Up.is_down());
        assert!(AdminState::Down.is_down());
    }

    #[test]
    fn test_oper_state() {
        assert!(OperState::Up.is_up());
        assert!(OperState::Down.is_down());
        assert!(!OperState::Unknown.is_up());
    }

    #[test]
    fn test_display() {
        assert_eq!(PortType::Phy.to_string(), "phy");
        assert_eq!(AdminState::Up.to_string(), "up");
        assert_eq!(OperState::Down.to_string(), "down");
    }
}
