//! Sub-interface parsing utilities

use crate::tables::{LAG_PREFIX, SUBINTF_LAG_PREFIX};

/// Parse sub-interface name into (parent, vlan_id)
///
/// Examples:
/// - "Ethernet0.100" → ("Ethernet0", "100")
/// - "PortChannel1.200" → ("PortChannel1", "200")
/// - "Po1.200" → ("PortChannel1", "200")
///
/// Returns None if the name is not a valid sub-interface
pub fn parse_subintf_name(name: &str) -> Option<(String, String)> {
    // Find the last dot (VLAN separator)
    let dot_pos = name.rfind('.')?;

    let parent = &name[..dot_pos];
    let vlan_id = &name[dot_pos + 1..];

    // Validate VLAN ID is numeric
    if vlan_id.parse::<u16>().is_err() {
        return None;
    }

    // Convert short LAG names (Po1 → PortChannel1)
    // Only convert if parent starts with "Po" but NOT "PortChannel"
    let parent = if parent.starts_with(SUBINTF_LAG_PREFIX) && !parent.starts_with(LAG_PREFIX) {
        // Extract the number after "Po"
        let lag_num = &parent[SUBINTF_LAG_PREFIX.len()..];
        format!("{}{}", LAG_PREFIX, lag_num)
    } else {
        parent.to_string()
    };

    Some((parent, vlan_id.to_string()))
}

/// Check if a name is a sub-interface
pub fn is_subintf_name(name: &str) -> bool {
    parse_subintf_name(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subintf_name_ethernet() {
        let (parent, vlan_id) = parse_subintf_name("Ethernet0.100").unwrap();
        assert_eq!(parent, "Ethernet0");
        assert_eq!(vlan_id, "100");
    }

    #[test]
    fn test_parse_subintf_name_ethernet_multi_digit() {
        let (parent, vlan_id) = parse_subintf_name("Ethernet32.4094").unwrap();
        assert_eq!(parent, "Ethernet32");
        assert_eq!(vlan_id, "4094");
    }

    #[test]
    fn test_parse_subintf_name_lag() {
        let (parent, vlan_id) = parse_subintf_name("PortChannel1.200").unwrap();
        assert_eq!(parent, "PortChannel1");
        assert_eq!(vlan_id, "200");
    }

    #[test]
    fn test_parse_subintf_name_short_lag() {
        let (parent, vlan_id) = parse_subintf_name("Po1.200").unwrap();
        assert_eq!(parent, "PortChannel1");
        assert_eq!(vlan_id, "200");
    }

    #[test]
    fn test_parse_subintf_name_short_lag_multi_digit() {
        let (parent, vlan_id) = parse_subintf_name("Po128.100").unwrap();
        assert_eq!(parent, "PortChannel128");
        assert_eq!(vlan_id, "100");
    }

    #[test]
    fn test_parse_subintf_name_no_dot() {
        assert!(parse_subintf_name("Ethernet0").is_none());
        assert!(parse_subintf_name("PortChannel1").is_none());
    }

    #[test]
    fn test_parse_subintf_name_invalid_vlan() {
        assert!(parse_subintf_name("Ethernet0.abc").is_none());
        assert!(parse_subintf_name("Ethernet0.").is_none());
    }

    #[test]
    fn test_is_subintf_name() {
        assert!(is_subintf_name("Ethernet0.100"));
        assert!(is_subintf_name("PortChannel1.200"));
        assert!(is_subintf_name("Po1.300"));

        assert!(!is_subintf_name("Ethernet0"));
        assert!(!is_subintf_name("Vlan100"));
        assert!(!is_subintf_name("Loopback0"));
    }
}
