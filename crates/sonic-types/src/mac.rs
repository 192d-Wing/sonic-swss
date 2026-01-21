//! MAC address type with safe parsing and formatting.

use crate::ParseError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A 48-bit Ethernet MAC address.
///
/// # Examples
///
/// ```
/// use sonic_types::MacAddress;
///
/// let mac: MacAddress = "00:11:22:33:44:55".parse().unwrap();
/// assert_eq!(mac.to_string(), "00:11:22:33:44:55");
///
/// // Also supports hyphen-separated format
/// let mac2: MacAddress = "00-11-22-33-44-55".parse().unwrap();
/// assert_eq!(mac, mac2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    /// The broadcast MAC address (FF:FF:FF:FF:FF:FF).
    pub const BROADCAST: MacAddress = MacAddress([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);

    /// The zero/null MAC address (00:00:00:00:00:00).
    pub const ZERO: MacAddress = MacAddress([0, 0, 0, 0, 0, 0]);

    /// Creates a new MAC address from raw bytes.
    pub const fn new(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }

    /// Returns the raw bytes of the MAC address.
    pub const fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    /// Returns true if this is a multicast address.
    ///
    /// A multicast address has the least significant bit of the first octet set.
    pub const fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    /// Returns true if this is a unicast address.
    pub const fn is_unicast(&self) -> bool {
        !self.is_multicast()
    }

    /// Returns true if this is a locally administered address.
    ///
    /// Locally administered addresses have the second least significant bit
    /// of the first octet set.
    pub const fn is_local(&self) -> bool {
        self.0[0] & 0x02 != 0
    }

    /// Returns true if this is a universally administered address.
    pub const fn is_universal(&self) -> bool {
        !self.is_local()
    }

    /// Returns true if this is the broadcast address.
    pub const fn is_broadcast(&self) -> bool {
        self.0[0] == 0xff && self.0[1] == 0xff && self.0[2] == 0xff
            && self.0[3] == 0xff && self.0[4] == 0xff && self.0[5] == 0xff
    }

    /// Returns true if this is the zero address.
    pub const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0
            && self.0[3] == 0 && self.0[4] == 0 && self.0[5] == 0
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl FromStr for MacAddress {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Support both colon and hyphen separators
        let separator = if s.contains(':') { ':' } else { '-' };

        let parts: Vec<&str> = s.split(separator).collect();
        if parts.len() != 6 {
            return Err(ParseError::InvalidMacAddress(s.to_string()));
        }

        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16)
                .map_err(|_| ParseError::InvalidMacAddress(s.to_string()))?;
        }

        Ok(MacAddress(bytes))
    }
}

impl TryFrom<String> for MacAddress {
    type Error = ParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<MacAddress> for String {
    fn from(mac: MacAddress) -> String {
        mac.to_string()
    }
}

impl From<[u8; 6]> for MacAddress {
    fn from(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }
}

impl From<MacAddress> for [u8; 6] {
    fn from(mac: MacAddress) -> [u8; 6] {
        mac.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_colon_format() {
        let mac: MacAddress = "00:11:22:33:44:55".parse().unwrap();
        assert_eq!(mac.as_bytes(), &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    }

    #[test]
    fn test_parse_hyphen_format() {
        let mac: MacAddress = "00-11-22-33-44-55".parse().unwrap();
        assert_eq!(mac.as_bytes(), &[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    }

    #[test]
    fn test_display() {
        let mac = MacAddress::new([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert_eq!(mac.to_string(), "aa:bb:cc:dd:ee:ff");
    }

    #[test]
    fn test_broadcast() {
        assert!(MacAddress::BROADCAST.is_broadcast());
        assert!(MacAddress::BROADCAST.is_multicast());
        assert!(!MacAddress::ZERO.is_broadcast());
    }

    #[test]
    fn test_multicast() {
        let multicast: MacAddress = "01:00:5e:00:00:01".parse().unwrap();
        assert!(multicast.is_multicast());

        let unicast: MacAddress = "00:11:22:33:44:55".parse().unwrap();
        assert!(unicast.is_unicast());
    }

    #[test]
    fn test_local_vs_universal() {
        let local: MacAddress = "02:00:00:00:00:01".parse().unwrap();
        assert!(local.is_local());

        let universal: MacAddress = "00:11:22:33:44:55".parse().unwrap();
        assert!(universal.is_universal());
    }

    #[test]
    fn test_invalid_format() {
        assert!("invalid".parse::<MacAddress>().is_err());
        assert!("00:11:22:33:44".parse::<MacAddress>().is_err());
        assert!("00:11:22:33:44:55:66".parse::<MacAddress>().is_err());
        assert!("gg:11:22:33:44:55".parse::<MacAddress>().is_err());
    }
}
