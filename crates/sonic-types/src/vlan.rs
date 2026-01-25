//! VLAN ID type with validation.

use crate::ParseError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// IEEE 802.1Q VLAN identifier (1-4094).
///
/// VLAN 0 is reserved (priority tagged frames).
/// VLAN 4095 is reserved.
/// Valid range is 1-4094.
///
/// # Examples
///
/// ```
/// use sonic_types::VlanId;
///
/// let vlan = VlanId::new(100).unwrap();
/// assert_eq!(vlan.as_u16(), 100);
///
/// // Invalid VLAN IDs return errors
/// assert!(VlanId::new(0).is_err());
/// assert!(VlanId::new(4095).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct VlanId(u16);

impl VlanId {
    /// Minimum valid VLAN ID.
    pub const MIN: u16 = 1;

    /// Maximum valid VLAN ID.
    pub const MAX: u16 = 4094;

    /// Default VLAN ID (VLAN 1).
    pub const DEFAULT: VlanId = VlanId(1);

    /// Creates a new VLAN ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the VLAN ID is not in the valid range (1-4094).
    pub const fn new(id: u16) -> Result<Self, ParseError> {
        if id >= Self::MIN && id <= Self::MAX {
            Ok(VlanId(id))
        } else {
            Err(ParseError::InvalidVlanId(id))
        }
    }

    /// Creates a new VLAN ID without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the ID is in the valid range (1-4094).
    /// Using an invalid ID may cause undefined behavior in SAI operations.
    pub const unsafe fn new_unchecked(id: u16) -> Self {
        VlanId(id)
    }

    /// Returns the VLAN ID as a u16.
    pub const fn as_u16(&self) -> u16 {
        self.0
    }

    /// Returns true if this is the default VLAN (VLAN 1).
    pub const fn is_default(&self) -> bool {
        self.0 == 1
    }
}

impl fmt::Display for VlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for VlanId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle "Vlan100" format
        let id_str = if s.to_lowercase().starts_with("vlan") {
            &s[4..]
        } else {
            s
        };

        let id: u16 = id_str.parse().map_err(|_| ParseError::InvalidVlanId(0))?;

        VlanId::new(id)
    }
}

impl TryFrom<u16> for VlanId {
    type Error = ParseError;

    fn try_from(id: u16) -> Result<Self, Self::Error> {
        VlanId::new(id)
    }
}

impl From<VlanId> for u16 {
    fn from(vlan: VlanId) -> u16 {
        vlan.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_valid_vlan_ids() {
        assert!(VlanId::new(1).is_ok());
        assert!(VlanId::new(100).is_ok());
        assert!(VlanId::new(4094).is_ok());
    }

    #[test]
    fn test_invalid_vlan_ids() {
        assert!(VlanId::new(0).is_err());
        assert!(VlanId::new(4095).is_err());
        assert!(VlanId::new(65535).is_err());
    }

    #[test]
    fn test_parse_numeric() {
        let vlan: VlanId = "100".parse().unwrap();
        assert_eq!(vlan.as_u16(), 100);
    }

    #[test]
    fn test_parse_vlan_prefix() {
        let vlan: VlanId = "Vlan100".parse().unwrap();
        assert_eq!(vlan.as_u16(), 100);

        let vlan2: VlanId = "VLAN200".parse().unwrap();
        assert_eq!(vlan2.as_u16(), 200);
    }

    #[test]
    fn test_default_vlan() {
        assert!(VlanId::DEFAULT.is_default());
        assert!(!VlanId::new(100).unwrap().is_default());
    }

    #[test]
    fn test_display() {
        let vlan = VlanId::new(100).unwrap();
        assert_eq!(vlan.to_string(), "100");
    }

    #[test]
    fn test_ordering() {
        let v1 = VlanId::new(10).unwrap();
        let v2 = VlanId::new(20).unwrap();
        assert!(v1 < v2);
    }
}
