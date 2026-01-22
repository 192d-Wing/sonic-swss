//! NVGRE tunnel types and structures.

use sonic_sai::types::RawSaiObjectId;
use sonic_types::IpAddress;
use std::collections::HashMap;

/// Maximum VSID value (24-bit).
pub const NVGRE_VSID_MAX_VALUE: u32 = 16777214;

/// Tunnel map type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapType {
    /// VLAN to VSID mapping.
    Vlan = 0,
    /// Bridge to VSID mapping.
    Bridge = 1,
}

/// SAI object IDs for a tunnel.
#[derive(Debug, Clone, Default)]
pub struct TunnelSaiIds {
    /// Encap mapper OIDs (one per map type).
    pub tunnel_encap_id: HashMap<MapType, RawSaiObjectId>,
    /// Decap mapper OIDs (one per map type).
    pub tunnel_decap_id: HashMap<MapType, RawSaiObjectId>,
    /// Main tunnel OID.
    pub tunnel_id: RawSaiObjectId,
    /// Tunnel termination entry OID.
    pub tunnel_term_id: RawSaiObjectId,
}

impl TunnelSaiIds {
    /// Creates a new empty TunnelSaiIds.
    pub fn new() -> Self {
        Self::default()
    }
}

/// NVGRE tunnel map entry.
#[derive(Debug, Clone)]
pub struct NvgreTunnelMapEntry {
    /// SAI map entry OID.
    pub map_entry_id: RawSaiObjectId,
    /// VLAN ID.
    pub vlan_id: u16,
    /// Virtual Subnet ID (VSID).
    pub vsid: u32,
}

impl NvgreTunnelMapEntry {
    /// Creates a new tunnel map entry.
    pub fn new(map_entry_id: RawSaiObjectId, vlan_id: u16, vsid: u32) -> Self {
        Self {
            map_entry_id,
            vlan_id,
            vsid,
        }
    }
}

/// NVGRE tunnel configuration.
#[derive(Debug, Clone)]
pub struct NvgreTunnelConfig {
    /// Tunnel name.
    pub name: String,
    /// Source IP address.
    pub src_ip: IpAddress,
}

impl NvgreTunnelConfig {
    /// Creates a new tunnel configuration.
    pub fn new(name: String, src_ip: IpAddress) -> Self {
        Self { name, src_ip }
    }
}

/// NVGRE tunnel map configuration.
#[derive(Debug, Clone)]
pub struct NvgreTunnelMapConfig {
    /// Tunnel name.
    pub tunnel_name: String,
    /// Map entry name.
    pub map_entry_name: String,
    /// VLAN ID.
    pub vlan_id: u16,
    /// Virtual Subnet ID.
    pub vsid: u32,
}

impl NvgreTunnelMapConfig {
    /// Creates a new tunnel map configuration.
    pub fn new(tunnel_name: String, map_entry_name: String, vlan_id: u16, vsid: u32) -> Self {
        Self {
            tunnel_name,
            map_entry_name,
            vlan_id,
            vsid,
        }
    }

    /// Validates the VSID is within range.
    pub fn validate_vsid(&self) -> Result<(), String> {
        if self.vsid == 0 {
            return Err("VSID cannot be 0 (reserved)".to_string());
        }
        if self.vsid > NVGRE_VSID_MAX_VALUE {
            return Err(format!(
                "VSID {} exceeds maximum value {}",
                self.vsid, NVGRE_VSID_MAX_VALUE
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_sai_ids() {
        let mut ids = TunnelSaiIds::new();

        ids.tunnel_encap_id.insert(MapType::Vlan, 0x1000);
        ids.tunnel_decap_id.insert(MapType::Vlan, 0x2000);
        ids.tunnel_id = 0x3000;
        ids.tunnel_term_id = 0x4000;

        assert_eq!(ids.tunnel_encap_id.get(&MapType::Vlan), Some(&0x1000));
        assert_eq!(ids.tunnel_decap_id.get(&MapType::Vlan), Some(&0x2000));
        assert_eq!(ids.tunnel_id, 0x3000);
        assert_eq!(ids.tunnel_term_id, 0x4000);
    }

    #[test]
    fn test_nvgre_tunnel_map_entry() {
        let entry = NvgreTunnelMapEntry::new(0x5000, 100, 1000);

        assert_eq!(entry.map_entry_id, 0x5000);
        assert_eq!(entry.vlan_id, 100);
        assert_eq!(entry.vsid, 1000);
    }

    #[test]
    fn test_vsid_validation() {
        let config = NvgreTunnelMapConfig::new(
            "tunnel1".to_string(),
            "map1".to_string(),
            100,
            1000,
        );
        assert!(config.validate_vsid().is_ok());

        // Test VSID = 0
        let config_zero = NvgreTunnelMapConfig::new(
            "tunnel1".to_string(),
            "map1".to_string(),
            100,
            0,
        );
        assert!(config_zero.validate_vsid().is_err());

        // Test VSID > max
        let config_max = NvgreTunnelMapConfig::new(
            "tunnel1".to_string(),
            "map1".to_string(),
            100,
            NVGRE_VSID_MAX_VALUE + 1,
        );
        assert!(config_max.validate_vsid().is_err());

        // Test VSID at max (should be valid)
        let config_at_max = NvgreTunnelMapConfig::new(
            "tunnel1".to_string(),
            "map1".to_string(),
            100,
            NVGRE_VSID_MAX_VALUE,
        );
        assert!(config_at_max.validate_vsid().is_ok());
    }
}
