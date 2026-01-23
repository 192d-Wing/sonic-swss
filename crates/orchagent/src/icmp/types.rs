//! ICMP echo (ping) responder types.

use std::net::IpAddr;

pub type RawSaiObjectId = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IcmpEchoKey {
    pub vrf_name: String,
    pub ip: IpAddr,
}

impl IcmpEchoKey {
    pub fn new(vrf_name: String, ip: IpAddr) -> Self {
        Self { vrf_name, ip }
    }
}

#[derive(Debug, Clone)]
pub struct IcmpEchoEntry {
    pub key: IcmpEchoKey,
    pub mode: IcmpMode,
}

impl IcmpEchoEntry {
    pub fn new(key: IcmpEchoKey, mode: IcmpMode) -> Self {
        Self { key, mode }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcmpMode {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Default)]
pub struct IcmpStats {
    pub entries_added: u64,
    pub entries_removed: u64,
}

/// ICMP redirect configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcmpRedirectConfig {
    pub enabled: bool,
    pub hop_limit: u8,
}

impl Default for IcmpRedirectConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hop_limit: 64,
        }
    }
}

/// Neighbor discovery configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeighborDiscoveryConfig {
    pub enabled: bool,
    pub max_solicitation_delay: u32,
}

impl Default for NeighborDiscoveryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_solicitation_delay: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icmp_redirect_config_default() {
        let config = IcmpRedirectConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.hop_limit, 64);
    }

    #[test]
    fn test_neighbor_discovery_config_default() {
        let config = NeighborDiscoveryConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.max_solicitation_delay, 1000);
    }
}
