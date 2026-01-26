//! VRF (Virtual Routing and Forwarding) isolation support
//!
//! Provides VRF-aware neighbor synchronization for multi-VRF deployments.
//! Maintains separate neighbor tables per VRF, enabling isolation in multi-tenant
//! or multi-instance network configurations.
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - AC-4: Information Flow Enforcement - Isolate neighbor tables per VRF
//! - SC-7: Boundary Protection - VRF network boundaries
//! - CM-8: System Component Inventory - Track neighbors per VRF

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

/// Virtual Routing and Forwarding identifier
/// Default VRF ID is 0 (default VRF)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VrfId(u32);

impl VrfId {
    /// Create a new VRF ID
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Default VRF (ID 0)
    pub const fn default_vrf() -> Self {
        Self(0)
    }

    /// Get the raw VRF ID value
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
}

impl Default for VrfId {
    fn default() -> Self {
        Self::default_vrf()
    }
}

impl std::fmt::Display for VrfId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for VrfId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse::<u32>()?;
        Ok(Self(id))
    }
}

/// VRF configuration for neighbor synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrfConfig {
    /// VRF identifier
    pub vrf_id: VrfId,
    /// VRF name (e.g., "Vrf1", "Vrf-mgmt")
    pub vrf_name: String,
    /// Whether this VRF is enabled for neighbor sync
    pub enabled: bool,
    /// Enable IPv4 support in this VRF (if feature is enabled globally)
    pub ipv4_enabled: bool,
    /// Enable IPv6 support in this VRF
    pub ipv6_enabled: bool,
}

impl VrfConfig {
    /// Create a new VRF configuration
    pub fn new(vrf_id: VrfId, vrf_name: String) -> Self {
        Self {
            vrf_id,
            vrf_name,
            enabled: true,
            ipv4_enabled: true,
            ipv6_enabled: true,
        }
    }

    /// Create the default VRF configuration
    pub fn default_vrf() -> Self {
        Self {
            vrf_id: VrfId::default_vrf(),
            vrf_name: "default".to_string(),
            enabled: true,
            ipv4_enabled: true,
            ipv6_enabled: true,
        }
    }

    /// Check if a given address family is enabled in this VRF
    pub fn is_family_enabled(&self, addr: &IpAddr) -> bool {
        if !self.enabled {
            return false;
        }

        match addr {
            IpAddr::V4(_) => self.ipv4_enabled,
            IpAddr::V6(_) => self.ipv6_enabled,
        }
    }

    /// Enable or disable this VRF
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set IPv4 support for this VRF
    pub fn set_ipv4_enabled(&mut self, enabled: bool) {
        self.ipv4_enabled = enabled;
    }

    /// Set IPv6 support for this VRF
    pub fn set_ipv6_enabled(&mut self, enabled: bool) {
        self.ipv6_enabled = enabled;
    }
}

/// VRF manager for tracking and managing multiple VRFs
#[derive(Debug, Clone)]
pub struct VrfManager {
    /// Active VRF configurations indexed by VRF ID
    vrfs: HashMap<u32, VrfConfig>,
}

impl VrfManager {
    /// Create a new VRF manager with default VRF
    pub fn new() -> Self {
        let mut vrfs = HashMap::new();
        let default = VrfConfig::default_vrf();
        vrfs.insert(default.vrf_id.as_u32(), default);

        Self { vrfs }
    }

    /// Register a new VRF
    pub fn register_vrf(&mut self, config: VrfConfig) {
        self.vrfs.insert(config.vrf_id.as_u32(), config);
    }

    /// Get a VRF configuration by ID
    pub fn get_vrf(&self, vrf_id: VrfId) -> Option<&VrfConfig> {
        self.vrfs.get(&vrf_id.as_u32())
    }

    /// Get a mutable reference to a VRF configuration
    pub fn get_vrf_mut(&mut self, vrf_id: VrfId) -> Option<&mut VrfConfig> {
        self.vrfs.get_mut(&vrf_id.as_u32())
    }

    /// Get all active VRFs
    pub fn get_all_vrfs(&self) -> Vec<&VrfConfig> {
        self.vrfs.values().collect()
    }

    /// Get all enabled VRF IDs
    pub fn get_enabled_vrfs(&self) -> Vec<VrfId> {
        self.vrfs
            .values()
            .filter(|v| v.enabled)
            .map(|v| v.vrf_id)
            .collect()
    }

    /// Check if a VRF is registered
    pub fn has_vrf(&self, vrf_id: VrfId) -> bool {
        self.vrfs.contains_key(&vrf_id.as_u32())
    }

    /// Enable or disable a VRF
    pub fn set_vrf_enabled(&mut self, vrf_id: VrfId, enabled: bool) {
        if let Some(vrf) = self.vrfs.get_mut(&vrf_id.as_u32()) {
            vrf.set_enabled(enabled);
        }
    }

    /// Count active VRFs
    pub fn count_enabled(&self) -> usize {
        self.vrfs.values().filter(|v| v.enabled).count()
    }

    /// Clear all VRFs except default
    pub fn clear_non_default(&mut self) {
        let default_id = VrfId::default_vrf().as_u32();
        self.vrfs.retain(|id, _| *id == default_id);
    }
}

impl Default for VrfManager {
    fn default() -> Self {
        Self::new()
    }
}

/// VRF interface binding tracking interface assignment to VRFs
#[derive(Debug, Clone)]
pub struct VrfInterfaceBinding {
    /// Mapping of interface name to VRF ID
    /// NIST: AC-4 - Interface to VRF binding for traffic isolation
    bindings: HashMap<String, VrfId>,
}

impl VrfInterfaceBinding {
    /// Create a new VRF interface binding tracker
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Bind an interface to a VRF
    pub fn bind_interface(&mut self, interface: String, vrf_id: VrfId) {
        self.bindings.insert(interface, vrf_id);
    }

    /// Get the VRF for a given interface
    pub fn get_interface_vrf(&self, interface: &str) -> Option<VrfId> {
        self.bindings.get(interface).copied()
    }

    /// Get the VRF for an interface, defaulting to default VRF if not bound
    pub fn get_interface_vrf_default(&self, interface: &str) -> VrfId {
        self.bindings.get(interface).copied().unwrap_or_default()
    }

    /// Unbind an interface from its VRF
    pub fn unbind_interface(&mut self, interface: &str) {
        self.bindings.remove(interface);
    }

    /// Get all interfaces bound to a specific VRF
    pub fn get_vrf_interfaces(&self, vrf_id: VrfId) -> Vec<&str> {
        self.bindings
            .iter()
            .filter(|(_, v)| **v == vrf_id)
            .map(|(i, _)| i.as_str())
            .collect()
    }

    /// Check if an interface is bound
    pub fn is_bound(&self, interface: &str) -> bool {
        self.bindings.contains_key(interface)
    }

    /// Get count of bound interfaces
    pub fn count_bound(&self) -> usize {
        self.bindings.len()
    }

    /// Clear all interface bindings
    pub fn clear_bindings(&mut self) {
        self.bindings.clear();
    }
}

impl Default for VrfInterfaceBinding {
    fn default() -> Self {
        Self::new()
    }
}

/// VRF-aware Redis key generator for neighbor tables
pub struct VrfRedisKeyGenerator {
    /// Enable VRF prefix in Redis keys (true = "VRF_NAME|..." format)
    use_vrf_prefix: bool,
}

impl VrfRedisKeyGenerator {
    /// Create a new VRF key generator
    pub fn new(use_vrf_prefix: bool) -> Self {
        Self { use_vrf_prefix }
    }

    /// Generate Redis key for a neighbor entry with VRF awareness
    ///
    /// # Key Formats
    /// - Default VRF: `NEIGH_TABLE:{interface}:{ip}`
    /// - Named VRF: `VRF_NAME|NEIGH_TABLE:{interface}:{ip}`
    pub fn neighbor_key(
        &self,
        vrf_id: VrfId,
        vrf_name: &str,
        interface: &str,
        ip: &IpAddr,
    ) -> String {
        let base_key = format!("{}:{}", interface, ip);

        if self.use_vrf_prefix && vrf_id.as_u32() != 0 {
            format!("{}|NEIGH_TABLE:{}", vrf_name, base_key)
        } else {
            format!("NEIGH_TABLE:{}", base_key)
        }
    }

    /// Generate Redis table prefix for all neighbors in a VRF
    pub fn table_prefix(&self, vrf_id: VrfId, vrf_name: &str) -> String {
        if self.use_vrf_prefix && vrf_id.as_u32() != 0 {
            format!("{}|NEIGH_TABLE", vrf_name)
        } else {
            "NEIGH_TABLE".to_string()
        }
    }

    /// Generate Redis key for VRF configuration
    pub fn config_key(&self, vrf_name: &str) -> String {
        if self.use_vrf_prefix {
            format!("{}|CONFIG", vrf_name)
        } else {
            "CONFIG".to_string()
        }
    }
}

impl Default for VrfRedisKeyGenerator {
    fn default() -> Self {
        Self::new(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vrf_id_creation() {
        let vrf = VrfId::new(1);
        assert_eq!(vrf.as_u32(), 1);

        let default_vrf = VrfId::default_vrf();
        assert_eq!(default_vrf.as_u32(), 0);
    }

    #[test]
    fn test_vrf_id_parse() {
        let vrf: VrfId = "5".parse().unwrap();
        assert_eq!(vrf.as_u32(), 5);
    }

    #[test]
    fn test_vrf_id_display() {
        let vrf = VrfId::new(42);
        assert_eq!(vrf.to_string(), "42");
    }

    #[test]
    fn test_vrf_config_creation() {
        let config = VrfConfig::new(VrfId::new(1), "Vrf1".to_string());
        assert_eq!(config.vrf_id, VrfId::new(1));
        assert_eq!(config.vrf_name, "Vrf1");
        assert!(config.enabled);
    }

    #[test]
    fn test_vrf_config_family_enabled() {
        let mut config = VrfConfig::new(VrfId::new(1), "Vrf1".to_string());
        let ipv4: IpAddr = "192.0.2.1".parse().unwrap();
        let ipv6: IpAddr = "2001:db8::1".parse().unwrap();

        assert!(config.is_family_enabled(&ipv4));
        assert!(config.is_family_enabled(&ipv6));

        config.set_ipv4_enabled(false);
        assert!(!config.is_family_enabled(&ipv4));
        assert!(config.is_family_enabled(&ipv6));
    }

    #[test]
    fn test_vrf_manager_creation() {
        let manager = VrfManager::new();
        assert!(manager.has_vrf(VrfId::default_vrf()));
    }

    #[test]
    fn test_vrf_manager_register() {
        let mut manager = VrfManager::new();
        let config = VrfConfig::new(VrfId::new(1), "Vrf1".to_string());
        manager.register_vrf(config);

        assert!(manager.has_vrf(VrfId::new(1)));
        assert_eq!(manager.count_enabled(), 2);
    }

    #[test]
    fn test_vrf_manager_get_enabled() {
        let mut manager = VrfManager::new();
        manager.register_vrf(VrfConfig::new(VrfId::new(1), "Vrf1".to_string()));
        manager.register_vrf(VrfConfig::new(VrfId::new(2), "Vrf2".to_string()));

        let enabled = manager.get_enabled_vrfs();
        assert_eq!(enabled.len(), 3); // default + 2 additional
    }

    #[test]
    fn test_vrf_manager_disable() {
        let mut manager = VrfManager::new();
        manager.register_vrf(VrfConfig::new(VrfId::new(1), "Vrf1".to_string()));

        assert_eq!(manager.count_enabled(), 2);
        manager.set_vrf_enabled(VrfId::new(1), false);
        assert_eq!(manager.count_enabled(), 1);
    }

    #[test]
    fn test_vrf_interface_binding() {
        let mut binding = VrfInterfaceBinding::new();
        binding.bind_interface("eth0".to_string(), VrfId::new(1));
        binding.bind_interface("eth1".to_string(), VrfId::new(1));
        binding.bind_interface("eth2".to_string(), VrfId::new(2));

        assert_eq!(binding.get_interface_vrf("eth0"), Some(VrfId::new(1)));
        assert_eq!(binding.get_interface_vrf("eth1"), Some(VrfId::new(1)));
        assert_eq!(binding.get_interface_vrf("eth2"), Some(VrfId::new(2)));
        assert_eq!(binding.get_interface_vrf("eth3"), None);
    }

    #[test]
    fn test_vrf_interface_binding_default() {
        let binding = VrfInterfaceBinding::new();
        let default = binding.get_interface_vrf_default("eth0");
        assert_eq!(default, VrfId::default_vrf());
    }

    #[test]
    fn test_vrf_interface_get_vrf_interfaces() {
        let mut binding = VrfInterfaceBinding::new();
        binding.bind_interface("eth0".to_string(), VrfId::new(1));
        binding.bind_interface("eth1".to_string(), VrfId::new(1));
        binding.bind_interface("eth2".to_string(), VrfId::new(2));

        let vrf1_ifaces = binding.get_vrf_interfaces(VrfId::new(1));
        assert_eq!(vrf1_ifaces.len(), 2);
    }

    #[test]
    fn test_vrf_redis_key_default() {
        let generator = VrfRedisKeyGenerator::new(true);
        let ipv6: IpAddr = "fe80::1".parse().unwrap();
        let key = generator.neighbor_key(VrfId::default_vrf(), "default", "eth0", &ipv6);
        assert_eq!(key, "NEIGH_TABLE:eth0:fe80::1");
    }

    #[test]
    fn test_vrf_redis_key_named() {
        let generator = VrfRedisKeyGenerator::new(true);
        let ipv6: IpAddr = "fe80::1".parse().unwrap();
        let key = generator.neighbor_key(VrfId::new(1), "Vrf1", "eth0", &ipv6);
        assert_eq!(key, "Vrf1|NEIGH_TABLE:eth0:fe80::1");
    }

    #[test]
    fn test_vrf_redis_key_no_prefix() {
        let generator = VrfRedisKeyGenerator::new(false);
        let ipv6: IpAddr = "fe80::1".parse().unwrap();
        let key = generator.neighbor_key(VrfId::new(1), "Vrf1", "eth0", &ipv6);
        assert_eq!(key, "NEIGH_TABLE:eth0:fe80::1");
    }

    #[test]
    fn test_vrf_redis_table_prefix() {
        let generator = VrfRedisKeyGenerator::new(true);
        let prefix = generator.table_prefix(VrfId::new(1), "Vrf1");
        assert_eq!(prefix, "Vrf1|NEIGH_TABLE");

        let default_prefix = generator.table_prefix(VrfId::default_vrf(), "default");
        assert_eq!(default_prefix, "NEIGH_TABLE");
    }

    #[test]
    fn test_vrf_redis_config_key() {
        let generator = VrfRedisKeyGenerator::new(true);
        let key = generator.config_key("Vrf1");
        assert_eq!(key, "Vrf1|CONFIG");
    }
}
