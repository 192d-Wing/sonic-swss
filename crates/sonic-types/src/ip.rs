//! IP address and prefix types with safe parsing.

use crate::ParseError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

/// An IPv4 address wrapper with additional SONiC-specific utilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ipv4Address(Ipv4Addr);

impl Ipv4Address {
    pub const UNSPECIFIED: Self = Ipv4Address(Ipv4Addr::UNSPECIFIED);
    pub const BROADCAST: Self = Ipv4Address(Ipv4Addr::BROADCAST);
    pub const LOCALHOST: Self = Ipv4Address(Ipv4Addr::LOCALHOST);

    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Address(Ipv4Addr::new(a, b, c, d))
    }

    pub const fn inner(&self) -> Ipv4Addr {
        self.0
    }

    pub const fn octets(&self) -> [u8; 4] {
        self.0.octets()
    }
}

impl fmt::Display for Ipv4Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Ipv4Address {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Ipv4Addr>()
            .map(Ipv4Address)
            .map_err(|_| ParseError::InvalidIpAddress(s.to_string()))
    }
}

impl From<Ipv4Addr> for Ipv4Address {
    fn from(addr: Ipv4Addr) -> Self {
        Ipv4Address(addr)
    }
}

impl From<Ipv4Address> for Ipv4Addr {
    fn from(addr: Ipv4Address) -> Self {
        addr.0
    }
}

/// An IPv6 address wrapper with additional SONiC-specific utilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ipv6Address(Ipv6Addr);

impl Ipv6Address {
    pub const UNSPECIFIED: Self = Ipv6Address(Ipv6Addr::UNSPECIFIED);
    pub const LOCALHOST: Self = Ipv6Address(Ipv6Addr::LOCALHOST);

    #[allow(clippy::too_many_arguments)]
    pub const fn new(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> Self {
        Ipv6Address(Ipv6Addr::new(a, b, c, d, e, f, g, h))
    }

    pub const fn inner(&self) -> Ipv6Addr {
        self.0
    }

    pub const fn octets(&self) -> [u8; 16] {
        self.0.octets()
    }

    pub const fn segments(&self) -> [u16; 8] {
        self.0.segments()
    }

    /// Returns true if this is a link-local address (fe80::/10).
    pub fn is_link_local(&self) -> bool {
        let segments = self.segments();
        (segments[0] & 0xffc0) == 0xfe80
    }
}

impl fmt::Display for Ipv6Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Ipv6Address {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Ipv6Addr>()
            .map(Ipv6Address)
            .map_err(|_| ParseError::InvalidIpAddress(s.to_string()))
    }
}

impl From<Ipv6Addr> for Ipv6Address {
    fn from(addr: Ipv6Addr) -> Self {
        Ipv6Address(addr)
    }
}

impl From<Ipv6Address> for Ipv6Addr {
    fn from(addr: Ipv6Address) -> Self {
        addr.0
    }
}

/// An IP address that can be either IPv4 or IPv6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpAddress {
    V4(Ipv4Address),
    V6(Ipv6Address),
}

impl IpAddress {
    /// Returns true if this is an IPv4 address.
    pub const fn is_ipv4(&self) -> bool {
        matches!(self, IpAddress::V4(_))
    }

    /// Returns true if this is an IPv6 address.
    pub const fn is_ipv6(&self) -> bool {
        matches!(self, IpAddress::V6(_))
    }

    /// Returns the IPv4 address if this is V4, None otherwise.
    pub const fn as_ipv4(&self) -> Option<&Ipv4Address> {
        match self {
            IpAddress::V4(addr) => Some(addr),
            IpAddress::V6(_) => None,
        }
    }

    /// Returns the IPv6 address if this is V6, None otherwise.
    pub const fn as_ipv6(&self) -> Option<&Ipv6Address> {
        match self {
            IpAddress::V4(_) => None,
            IpAddress::V6(addr) => Some(addr),
        }
    }
}

impl fmt::Display for IpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpAddress::V4(addr) => addr.fmt(f),
            IpAddress::V6(addr) => addr.fmt(f),
        }
    }
}

impl FromStr for IpAddress {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains(':') {
            s.parse::<Ipv6Address>().map(IpAddress::V6)
        } else {
            s.parse::<Ipv4Address>().map(IpAddress::V4)
        }
    }
}

impl From<Ipv4Address> for IpAddress {
    fn from(addr: Ipv4Address) -> Self {
        IpAddress::V4(addr)
    }
}

impl From<Ipv6Address> for IpAddress {
    fn from(addr: Ipv6Address) -> Self {
        IpAddress::V6(addr)
    }
}

impl From<Ipv4Addr> for IpAddress {
    fn from(addr: Ipv4Addr) -> Self {
        IpAddress::V4(Ipv4Address(addr))
    }
}

impl From<Ipv6Addr> for IpAddress {
    fn from(addr: Ipv6Addr) -> Self {
        IpAddress::V6(Ipv6Address(addr))
    }
}

/// An IP prefix in CIDR notation (e.g., 10.0.0.0/24 or 2001:db8::/32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IpPrefix {
    address: IpAddress,
    prefix_len: u8,
}

impl IpPrefix {
    /// Creates a new IP prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if the prefix length is invalid for the address type
    /// (>32 for IPv4, >128 for IPv6).
    pub fn new(address: IpAddress, prefix_len: u8) -> Result<Self, ParseError> {
        let max_len = match address {
            IpAddress::V4(_) => 32,
            IpAddress::V6(_) => 128,
        };

        if prefix_len > max_len {
            return Err(ParseError::InvalidIpPrefix(format!(
                "prefix length {} exceeds maximum {} for address type",
                prefix_len, max_len
            )));
        }

        Ok(IpPrefix {
            address,
            prefix_len,
        })
    }

    /// Returns the network address of this prefix.
    pub const fn address(&self) -> &IpAddress {
        &self.address
    }

    /// Returns the prefix length in bits.
    pub const fn prefix_len(&self) -> u8 {
        self.prefix_len
    }

    /// Returns true if this is an IPv4 prefix.
    pub const fn is_ipv4(&self) -> bool {
        self.address.is_ipv4()
    }

    /// Returns true if this is an IPv6 prefix.
    pub const fn is_ipv6(&self) -> bool {
        self.address.is_ipv6()
    }

    /// Returns true if this is a host route (/32 for IPv4, /128 for IPv6).
    pub const fn is_host_route(&self) -> bool {
        match self.address {
            IpAddress::V4(_) => self.prefix_len == 32,
            IpAddress::V6(_) => self.prefix_len == 128,
        }
    }

    /// Returns true if this is the default route (0.0.0.0/0 or ::/0).
    pub fn is_default(&self) -> bool {
        self.prefix_len == 0
    }
}

impl fmt::Display for IpPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix_len)
    }
}

impl FromStr for IpPrefix {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (addr_str, len_str) = s
            .rsplit_once('/')
            .ok_or_else(|| ParseError::InvalidIpPrefix(s.to_string()))?;

        let address: IpAddress = addr_str.parse()?;
        let prefix_len: u8 = len_str
            .parse()
            .map_err(|_| ParseError::InvalidIpPrefix(s.to_string()))?;

        IpPrefix::new(address, prefix_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_ipv4_parse() {
        let addr: Ipv4Address = "192.168.1.1".parse().unwrap();
        assert_eq!(addr.octets(), [192, 168, 1, 1]);
    }

    #[test]
    fn test_ipv6_parse() {
        let addr: Ipv6Address = "2001:db8::1".parse().unwrap();
        assert_eq!(addr.segments()[0], 0x2001);
        assert_eq!(addr.segments()[1], 0x0db8);
    }

    #[test]
    fn test_ipv6_link_local() {
        let link_local: Ipv6Address = "fe80::1".parse().unwrap();
        assert!(link_local.is_link_local());

        let global: Ipv6Address = "2001:db8::1".parse().unwrap();
        assert!(!global.is_link_local());
    }

    #[test]
    fn test_ip_address_discrimination() {
        let v4: IpAddress = "10.0.0.1".parse().unwrap();
        assert!(v4.is_ipv4());
        assert!(!v4.is_ipv6());

        let v6: IpAddress = "::1".parse().unwrap();
        assert!(!v6.is_ipv4());
        assert!(v6.is_ipv6());
    }

    #[test]
    fn test_ip_prefix_parse() {
        let prefix: IpPrefix = "10.0.0.0/24".parse().unwrap();
        assert!(prefix.is_ipv4());
        assert_eq!(prefix.prefix_len(), 24);

        let v6_prefix: IpPrefix = "2001:db8::/32".parse().unwrap();
        assert!(v6_prefix.is_ipv6());
        assert_eq!(v6_prefix.prefix_len(), 32);
    }

    #[test]
    fn test_ip_prefix_host_route() {
        let host_v4: IpPrefix = "10.0.0.1/32".parse().unwrap();
        assert!(host_v4.is_host_route());

        let network: IpPrefix = "10.0.0.0/24".parse().unwrap();
        assert!(!network.is_host_route());

        let host_v6: IpPrefix = "2001:db8::1/128".parse().unwrap();
        assert!(host_v6.is_host_route());
    }

    #[test]
    fn test_ip_prefix_default() {
        let default_v4: IpPrefix = "0.0.0.0/0".parse().unwrap();
        assert!(default_v4.is_default());

        let default_v6: IpPrefix = "::/0".parse().unwrap();
        assert!(default_v6.is_default());
    }

    #[test]
    fn test_invalid_prefix_length() {
        assert!("10.0.0.0/33".parse::<IpPrefix>().is_err());
        assert!("2001:db8::/129".parse::<IpPrefix>().is_err());
    }

    #[test]
    fn test_display() {
        let prefix: IpPrefix = "192.168.0.0/16".parse().unwrap();
        assert_eq!(prefix.to_string(), "192.168.0.0/16");
    }
}
