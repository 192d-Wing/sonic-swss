//! VRFOrch implementation.
//!
//! Manages Virtual Routing and Forwarding instances in SAI.

use std::collections::HashMap;
use std::sync::Arc;

use super::types::{L3VniEntry, Vni, VrfConfig, VrfEntry, VrfId, VrfName, VrfVlanId};
use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;

/// Error type for VRF operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum VrfOrchError {
    /// VRF not found.
    #[error("VRF not found: {0}")]
    VrfNotFound(String),
    /// VRF already exists.
    #[error("VRF already exists: {0}")]
    VrfAlreadyExists(String),
    /// VRF is still in use (has references).
    #[error("VRF in use: {0} (ref_count={1})")]
    VrfInUse(String, i32),
    /// SAI operation failed.
    #[error("SAI error: {0}")]
    SaiError(String),
    /// VNI not found.
    #[error("VNI not found: {0}")]
    VniNotFound(u32),
    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    /// Callback error.
    #[error("Callback error: {0}")]
    CallbackError(String),
}

/// Callbacks for VRF operations.
///
/// These allow integration with other orchs (e.g., PortsOrch for L3 VNI status).
pub trait VrfOrchCallbacks: Send + Sync {
    /// Called when a VRF is created.
    fn on_vrf_created(&self, _name: &str, _vrf_id: VrfId) {}

    /// Called when a VRF is removed.
    fn on_vrf_removed(&self, _name: &str, _vrf_id: VrfId) {}

    /// Called to update L3 VNI status on a VLAN.
    fn update_l3_vni_status(&self, _vlan_id: VrfVlanId, _enable: bool) -> bool {
        true
    }

    /// Called to get VLAN mapped to VNI from VxlanOrch.
    fn get_vlan_mapped_to_vni(&self, _vni: Vni) -> Option<VrfVlanId> {
        None
    }

    /// Called to check if EVPN VTEP exists.
    fn has_evpn_vtep(&self) -> bool {
        false
    }

    /// Called when a VRF is added to FlowCounterRouteOrch.
    fn on_add_vr(&self, _vrf_id: VrfId) {}

    /// Called when a VRF is removed from FlowCounterRouteOrch.
    fn on_remove_vr(&self, _vrf_id: VrfId) {}
}

/// Default no-op callbacks.
struct NoOpCallbacks;
impl VrfOrchCallbacks for NoOpCallbacks {}

/// Configuration for VRFOrch.
#[derive(Debug, Clone)]
pub struct VrfOrchConfig {
    /// Global (default) virtual router ID.
    pub global_vrf_id: VrfId,
}

impl Default for VrfOrchConfig {
    fn default() -> Self {
        Self { global_vrf_id: 0 }
    }
}

impl VrfOrchConfig {
    /// Creates a new config with the global VRF ID.
    pub fn new(global_vrf_id: VrfId) -> Self {
        Self { global_vrf_id }
    }
}

/// Statistics for VRFOrch operations.
#[derive(Debug, Clone, Default)]
pub struct VrfOrchStats {
    /// Number of VRFs created.
    pub vrfs_created: u64,
    /// Number of VRFs removed.
    pub vrfs_removed: u64,
    /// Number of VRF updates.
    pub vrfs_updated: u64,
    /// Number of VNI mappings created.
    pub vni_mappings_created: u64,
    /// Number of VNI mappings removed.
    pub vni_mappings_removed: u64,
}

/// VRFOrch - manages Virtual Routing and Forwarding instances.
pub struct VrfOrch {
    /// Configuration.
    config: VrfOrchConfig,
    /// Callbacks for integration with other orchs.
    callbacks: Option<Arc<dyn VrfOrchCallbacks>>,
    /// VRF table: name -> entry.
    vrf_table: HashMap<VrfName, VrfEntry>,
    /// Reverse lookup: VRF ID -> name.
    vrf_id_to_name: HashMap<VrfId, VrfName>,
    /// VRF to VNI mapping.
    vrf_vni_map: HashMap<VrfName, Vni>,
    /// L3 VNI table: VNI -> L3VniEntry.
    l3vni_table: HashMap<Vni, L3VniEntry>,
    /// Statistics.
    stats: VrfOrchStats,
    /// Initialized flag.
    initialized: bool,
}

impl std::fmt::Debug for VrfOrch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VrfOrch")
            .field("config", &self.config)
            .field("vrf_count", &self.vrf_table.len())
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl VrfOrch {
    /// Creates a new VRFOrch with the given configuration.
    pub fn new(config: VrfOrchConfig) -> Self {
        Self {
            config,
            callbacks: None,
            vrf_table: HashMap::new(),
            vrf_id_to_name: HashMap::new(),
            vrf_vni_map: HashMap::new(),
            l3vni_table: HashMap::new(),
            stats: VrfOrchStats::default(),
            initialized: false,
        }
    }

    /// Sets the callbacks.
    pub fn set_callbacks(&mut self, callbacks: Arc<dyn VrfOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the configuration.
    pub fn config(&self) -> &VrfOrchConfig {
        &self.config
    }

    /// Returns the statistics.
    pub fn stats(&self) -> &VrfOrchStats {
        &self.stats
    }

    /// Returns true if initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Sets the initialized state.
    pub fn set_initialized(&mut self, initialized: bool) {
        self.initialized = initialized;
    }

    /// Returns the number of VRFs.
    pub fn vrf_count(&self) -> usize {
        self.vrf_table.len()
    }

    /// Returns true if a VRF exists.
    pub fn vrf_exists(&self, name: &str) -> bool {
        self.vrf_table.contains_key(name)
    }

    /// Gets the VRF ID for a name.
    ///
    /// Returns the global VRF ID if the name is empty or not found.
    pub fn get_vrf_id(&self, name: &str) -> VrfId {
        if name.is_empty() {
            return self.config.global_vrf_id;
        }
        self.vrf_table
            .get(name)
            .map(|e| e.vrf_id)
            .unwrap_or(self.config.global_vrf_id)
    }

    /// Gets a VRF entry by name.
    pub fn get_vrf(&self, name: &str) -> Option<&VrfEntry> {
        self.vrf_table.get(name)
    }

    /// Gets a mutable VRF entry by name.
    pub fn get_vrf_mut(&mut self, name: &str) -> Option<&mut VrfEntry> {
        self.vrf_table.get_mut(name)
    }

    /// Gets the VRF name for an ID.
    ///
    /// Returns an empty string if the ID is the global VRF or not found.
    pub fn get_vrf_name(&self, vrf_id: VrfId) -> &str {
        if vrf_id == self.config.global_vrf_id {
            return "";
        }
        self.vrf_id_to_name
            .get(&vrf_id)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Increases the reference count for a VRF by name.
    ///
    /// Returns the new ref count, or an error if not found.
    pub fn increase_vrf_ref_count(&mut self, name: &str) -> Result<i32, VrfOrchError> {
        if let Some(entry) = self.vrf_table.get_mut(name) {
            entry.incr_ref_count();
            Ok(entry.ref_count)
        } else {
            Err(VrfOrchError::VrfNotFound(name.to_string()))
        }
    }

    /// Increases the reference count for a VRF by ID.
    ///
    /// Does nothing for the global VRF.
    pub fn increase_vrf_ref_count_by_id(&mut self, vrf_id: VrfId) -> Result<i32, VrfOrchError> {
        if vrf_id == self.config.global_vrf_id {
            return Ok(0);
        }
        let name = self
            .vrf_id_to_name
            .get(&vrf_id)
            .cloned()
            .ok_or_else(|| VrfOrchError::VrfNotFound(format!("id=0x{:x}", vrf_id)))?;
        self.increase_vrf_ref_count(&name)
    }

    /// Decreases the reference count for a VRF by name.
    ///
    /// Returns the new ref count, or an error if not found or would underflow.
    pub fn decrease_vrf_ref_count(&mut self, name: &str) -> Result<i32, VrfOrchError> {
        if let Some(entry) = self.vrf_table.get_mut(name) {
            entry
                .decr_ref_count()
                .ok_or_else(|| VrfOrchError::InvalidConfig("Ref count underflow".to_string()))
        } else {
            Err(VrfOrchError::VrfNotFound(name.to_string()))
        }
    }

    /// Decreases the reference count for a VRF by ID.
    ///
    /// Does nothing for the global VRF.
    pub fn decrease_vrf_ref_count_by_id(&mut self, vrf_id: VrfId) -> Result<i32, VrfOrchError> {
        if vrf_id == self.config.global_vrf_id {
            return Ok(0);
        }
        let name = self
            .vrf_id_to_name
            .get(&vrf_id)
            .cloned()
            .ok_or_else(|| VrfOrchError::VrfNotFound(format!("id=0x{:x}", vrf_id)))?;
        self.decrease_vrf_ref_count(&name)
    }

    /// Gets the reference count for a VRF.
    ///
    /// Returns -1 if not found (matching C++ behavior).
    pub fn get_vrf_ref_count(&self, name: &str) -> i32 {
        self.vrf_table.get(name).map(|e| e.ref_count).unwrap_or(-1)
    }

    /// Gets the VNI mapped to a VRF.
    ///
    /// Returns 0 if not mapped.
    pub fn get_vrf_mapped_vni(&self, vrf_name: &str) -> Vni {
        self.vrf_vni_map.get(vrf_name).copied().unwrap_or(0)
    }

    /// Gets the VLAN ID for an L3 VNI.
    ///
    /// Returns None if not found.
    pub fn get_l3_vni_vlan(&self, vni: Vni) -> Option<VrfVlanId> {
        self.l3vni_table.get(&vni).map(|e| e.vlan_id)
    }

    /// Returns true if the VNI is an L3 VNI.
    pub fn is_l3_vni(&self, vni: Vni) -> bool {
        self.l3vni_table
            .get(&vni)
            .map(|e| e.l3_vni)
            .unwrap_or(false)
    }

    /// Creates a VRF from configuration.
    ///
    /// If the VRF already exists, updates it instead.
    pub fn add_vrf(&mut self, config: &VrfConfig) -> Result<VrfId, VrfOrchError> {
        let name = &config.name;

        if self.vrf_table.contains_key(name) {
            // Update existing VRF
            return self.update_vrf(config);
        }

        // In real implementation, this would call SAI API to create virtual router
        // For now, generate a mock ID
        let vrf_id = self.generate_vrf_id();

        let mut entry = VrfEntry::new(vrf_id);

        // Apply configuration
        if let Some(v4) = config.v4 {
            entry.admin_v4_state = v4;
        }
        if let Some(v6) = config.v6 {
            entry.admin_v6_state = v6;
        }
        entry.src_mac = config.src_mac;
        entry.ttl_action = config.ttl_action;
        entry.ip_opt_action = config.ip_opt_action;
        entry.l3_mc_action = config.l3_mc_action;
        if let Some(fallback) = config.fallback {
            entry.fallback = fallback;
        }

        // Store entry
        self.vrf_table.insert(name.clone(), entry);
        self.vrf_id_to_name.insert(vrf_id, name.clone());

        // Handle VNI mapping
        if let Some(vni) = config.vni {
            if vni != 0 {
                self.update_vrf_vni_map(name, vni)?;
            }
        }

        // Notify callbacks
        if let Some(callbacks) = &self.callbacks {
            callbacks.on_vrf_created(name, vrf_id);
            callbacks.on_add_vr(vrf_id);
        }

        self.stats.vrfs_created += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "VrfOrch", "create_vrf")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name.clone())
                .with_object_type("vrf")
                .with_details(serde_json::json!({
                    "vrf_name": name,
                    "vrf_id": vrf_id,
                    "v4_enabled": entry.admin_v4_state,
                    "v6_enabled": entry.admin_v6_state,
                    "vni": config.vni,
                    "stats": {
                        "vrfs_created": self.stats.vrfs_created
                    }
                }))
        );

        Ok(vrf_id)
    }

    /// Updates an existing VRF.
    fn update_vrf(&mut self, config: &VrfConfig) -> Result<VrfId, VrfOrchError> {
        let name = &config.name;

        let entry = self
            .vrf_table
            .get_mut(name)
            .ok_or_else(|| VrfOrchError::VrfNotFound(name.clone()))?;

        let vrf_id = entry.vrf_id;

        // Update configuration
        if let Some(v4) = config.v4 {
            entry.admin_v4_state = v4;
        }
        if let Some(v6) = config.v6 {
            entry.admin_v6_state = v6;
        }
        if config.src_mac.is_some() {
            entry.src_mac = config.src_mac;
        }
        if config.ttl_action.is_some() {
            entry.ttl_action = config.ttl_action;
        }
        if config.ip_opt_action.is_some() {
            entry.ip_opt_action = config.ip_opt_action;
        }
        if config.l3_mc_action.is_some() {
            entry.l3_mc_action = config.l3_mc_action;
        }
        if let Some(fallback) = config.fallback {
            entry.fallback = fallback;
        }

        // Handle VNI mapping update
        if let Some(vni) = config.vni {
            self.update_vrf_vni_map(name, vni)?;
        }

        self.stats.vrfs_updated += 1;

        Ok(vrf_id)
    }

    /// Removes a VRF.
    ///
    /// Returns an error if the VRF is still in use.
    pub fn remove_vrf(&mut self, name: &str) -> Result<(), VrfOrchError> {
        let entry = self
            .vrf_table
            .get(name)
            .ok_or_else(|| VrfOrchError::VrfNotFound(name.to_string()))?;

        if entry.is_in_use() {
            let error = VrfOrchError::VrfInUse(name.to_string(), entry.ref_count);
            audit_log!(
                AuditRecord::new(AuditCategory::ResourceDelete, "VrfOrch", "remove_vrf")
                    .with_outcome(AuditOutcome::Failure)
                    .with_object_id(name.to_string())
                    .with_object_type("vrf")
                    .with_error(error.to_string())
            );
            return Err(error);
        }

        let vrf_id = entry.vrf_id;

        // Remove VNI mapping
        self.del_vrf_vni_map(name, 0)?;

        // Remove from tables
        self.vrf_table.remove(name);
        self.vrf_id_to_name.remove(&vrf_id);

        // Notify callbacks
        if let Some(callbacks) = &self.callbacks {
            callbacks.on_remove_vr(vrf_id);
            callbacks.on_vrf_removed(name, vrf_id);
        }

        self.stats.vrfs_removed += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceDelete, "VrfOrch", "remove_vrf")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(name.to_string())
                .with_object_type("vrf")
                .with_details(serde_json::json!({
                    "vrf_name": name,
                    "vrf_id": vrf_id,
                    "stats": {
                        "vrfs_removed": self.stats.vrfs_removed
                    }
                }))
        );

        Ok(())
    }

    /// Updates the VRF to VNI mapping.
    fn update_vrf_vni_map(&mut self, vrf_name: &str, vni: Vni) -> Result<(), VrfOrchError> {
        let old_vni = self.get_vrf_mapped_vni(vrf_name);

        if old_vni == vni {
            return Ok(());
        }

        if vni == 0 {
            // Remove mapping
            return self.del_vrf_vni_map(vrf_name, old_vni);
        }

        // Check for EVPN VTEP - required for VNI mapping
        let has_vtep = self
            .callbacks
            .as_ref()
            .map(|cb| cb.has_evpn_vtep())
            .unwrap_or(false);
        if !has_vtep {
            return Err(VrfOrchError::CallbackError(
                "EVPN VTEP not found".to_string(),
            ));
        }

        // Update L3 VNI table
        self.l3vni_table.insert(vni, L3VniEntry::pending());
        self.vrf_vni_map.insert(vrf_name.to_string(), vni);

        // Get VLAN mapping from VxlanOrch
        if let Some(callbacks) = &self.callbacks {
            if let Some(vlan_id) = callbacks.get_vlan_mapped_to_vni(vni) {
                if let Some(entry) = self.l3vni_table.get_mut(&vni) {
                    entry.vlan_id = vlan_id;
                }
                if vlan_id != 0 {
                    callbacks.update_l3_vni_status(vlan_id, true);
                }
            }
        }

        self.stats.vni_mappings_created += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceCreate, "VrfOrch", "add_l3_vni")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(format!("vrf_vni_{}_{}", vrf_name, vni))
                .with_object_type("l3_vni")
                .with_details(serde_json::json!({
                    "vrf_name": vrf_name,
                    "vni": vni,
                    "stats": {
                        "vni_mappings_created": self.stats.vni_mappings_created
                    }
                }))
        );

        Ok(())
    }

    /// Removes the VRF to VNI mapping.
    fn del_vrf_vni_map(&mut self, vrf_name: &str, mut vni: Vni) -> Result<(), VrfOrchError> {
        if vni == 0 {
            vni = self.get_vrf_mapped_vni(vrf_name);
        }

        if vni == 0 {
            return Ok(());
        }

        // Get VLAN before removing
        if let Some(entry) = self.l3vni_table.get(&vni) {
            let vlan_id = entry.vlan_id;
            if vlan_id != 0 {
                if let Some(callbacks) = &self.callbacks {
                    callbacks.update_l3_vni_status(vlan_id, false);
                }
            }
        }

        // Remove mappings
        self.l3vni_table.remove(&vni);
        self.vrf_vni_map.remove(vrf_name);

        self.stats.vni_mappings_removed += 1;

        audit_log!(
            AuditRecord::new(AuditCategory::ResourceDelete, "VrfOrch", "remove_l3_vni")
                .with_outcome(AuditOutcome::Success)
                .with_object_id(format!("vrf_vni_{}_{}", vrf_name, vni))
                .with_object_type("l3_vni")
                .with_details(serde_json::json!({
                    "vrf_name": vrf_name,
                    "vni": vni,
                    "stats": {
                        "vni_mappings_removed": self.stats.vni_mappings_removed
                    }
                }))
        );

        Ok(())
    }

    /// Updates the L3 VNI VLAN mapping (called by VxlanOrch).
    pub fn update_l3_vni_vlan(&mut self, vni: Vni, vlan_id: VrfVlanId) -> Result<(), VrfOrchError> {
        if let Some(entry) = self.l3vni_table.get_mut(&vni) {
            entry.vlan_id = vlan_id;

            // Notify PortsOrch to update VE status
            if let Some(callbacks) = &self.callbacks {
                callbacks.update_l3_vni_status(vlan_id, true);
            }

            Ok(())
        } else {
            Err(VrfOrchError::VniNotFound(vni))
        }
    }

    /// Generates a unique VRF ID (mock implementation).
    fn generate_vrf_id(&self) -> VrfId {
        // In real implementation, this would be returned by SAI
        // For now, generate based on count + base offset
        0x2000_0000 + (self.vrf_table.len() as u64) + 1
    }

    /// Returns an iterator over all VRF names.
    pub fn vrf_names(&self) -> impl Iterator<Item = &String> {
        self.vrf_table.keys()
    }

    /// Returns an iterator over all VRF entries.
    pub fn vrfs(&self) -> impl Iterator<Item = (&String, &VrfEntry)> {
        self.vrf_table.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vrf::types::PacketAction;
    use sonic_types::MacAddress;

    #[test]
    fn test_vrf_orch_new() {
        let orch = VrfOrch::new(VrfOrchConfig::default());
        assert_eq!(orch.vrf_count(), 0);
        assert!(!orch.is_initialized());
    }

    #[test]
    fn test_add_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let config = VrfConfig::new("Vrf1").with_v4(true).with_v6(true);

        let vrf_id = orch.add_vrf(&config).unwrap();
        assert!(vrf_id != 0);
        assert!(orch.vrf_exists("Vrf1"));
        assert_eq!(orch.vrf_count(), 1);
        assert_eq!(orch.stats().vrfs_created, 1);
    }

    #[test]
    fn test_get_vrf_id() {
        let mut orch = VrfOrch::new(VrfOrchConfig::new(0x1000));

        // Empty name returns global VRF
        assert_eq!(orch.get_vrf_id(""), 0x1000);

        // Unknown name returns global VRF
        assert_eq!(orch.get_vrf_id("Unknown"), 0x1000);

        // Add a VRF
        let config = VrfConfig::new("Vrf1");
        let vrf_id = orch.add_vrf(&config).unwrap();

        assert_eq!(orch.get_vrf_id("Vrf1"), vrf_id);
    }

    #[test]
    fn test_get_vrf_name() {
        let mut orch = VrfOrch::new(VrfOrchConfig::new(0x1000));

        // Global VRF returns empty string
        assert_eq!(orch.get_vrf_name(0x1000), "");

        // Unknown ID returns empty string
        assert_eq!(orch.get_vrf_name(0x9999), "");

        // Add a VRF
        let config = VrfConfig::new("Vrf1");
        let vrf_id = orch.add_vrf(&config).unwrap();

        assert_eq!(orch.get_vrf_name(vrf_id), "Vrf1");
    }

    #[test]
    fn test_ref_count() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let config = VrfConfig::new("Vrf1");
        orch.add_vrf(&config).unwrap();

        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 0);

        orch.increase_vrf_ref_count("Vrf1").unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 1);

        orch.increase_vrf_ref_count("Vrf1").unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 2);

        orch.decrease_vrf_ref_count("Vrf1").unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 1);

        // Unknown VRF returns -1
        assert_eq!(orch.get_vrf_ref_count("Unknown"), -1);
    }

    #[test]
    fn test_remove_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let config = VrfConfig::new("Vrf1");
        orch.add_vrf(&config).unwrap();

        orch.remove_vrf("Vrf1").unwrap();
        assert!(!orch.vrf_exists("Vrf1"));
        assert_eq!(orch.stats().vrfs_removed, 1);
    }

    #[test]
    fn test_remove_vrf_in_use() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let config = VrfConfig::new("Vrf1");
        orch.add_vrf(&config).unwrap();
        orch.increase_vrf_ref_count("Vrf1").unwrap();

        let result = orch.remove_vrf("Vrf1");
        assert!(matches!(result, Err(VrfOrchError::VrfInUse(_, _))));
    }

    #[test]
    fn test_vrf_with_vni() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        // Without EVPN VTEP callback, VNI mapping fails
        let config = VrfConfig::new("Vrf1").with_vni(10000);
        let result = orch.add_vrf(&config);
        // With no callbacks, should fail due to missing EVPN VTEP
        assert!(result.is_err());
    }

    #[test]
    fn test_update_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let config1 = VrfConfig::new("Vrf1").with_v4(true).with_v6(true);
        orch.add_vrf(&config1).unwrap();

        // Update with new config
        let config2 = VrfConfig::new("Vrf1").with_v4(false);
        orch.add_vrf(&config2).unwrap();

        let entry = orch.get_vrf("Vrf1").unwrap();
        assert!(!entry.admin_v4_state);
        assert!(entry.admin_v6_state); // Unchanged
        assert_eq!(orch.stats().vrfs_updated, 1);
    }

    #[test]
    fn test_l3_vni() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        // Manually insert L3 VNI for testing
        orch.l3vni_table.insert(10000, L3VniEntry::new(100, true));

        assert!(orch.is_l3_vni(10000));
        assert_eq!(orch.get_l3_vni_vlan(10000), Some(100));

        assert!(!orch.is_l3_vni(99999));
        assert_eq!(orch.get_l3_vni_vlan(99999), None);
    }

    #[test]
    fn test_vrf_iteration() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        orch.add_vrf(&VrfConfig::new("Vrf2")).unwrap();
        orch.add_vrf(&VrfConfig::new("Vrf3")).unwrap();

        let names: Vec<_> = orch.vrf_names().collect();
        assert_eq!(names.len(), 3);
    }

    // ========== VRF Creation and Management Tests ==========

    #[test]
    fn test_create_vrf_with_default_vni() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        // VRF without explicit VNI (VNI defaults to None)
        let config = VrfConfig::new("Vrf1").with_v4(true).with_v6(true);
        let vrf_id = orch.add_vrf(&config).unwrap();

        assert!(vrf_id != 0);
        assert!(orch.vrf_exists("Vrf1"));
        assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 0);
    }

    #[test]
    fn test_create_vrf_with_custom_vni() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        // Mock callbacks with EVPN VTEP support
        struct MockCallbacks;
        impl VrfOrchCallbacks for MockCallbacks {
            fn has_evpn_vtep(&self) -> bool {
                true
            }
            fn get_vlan_mapped_to_vni(&self, vni: Vni) -> Option<VrfVlanId> {
                if vni == 10000 {
                    Some(100)
                } else {
                    None
                }
            }
        }
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = VrfConfig::new("Vrf1").with_vni(10000);
        let vrf_id = orch.add_vrf(&config).unwrap();

        assert!(vrf_id != 0);
        assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 10000);
        assert_eq!(orch.get_l3_vni_vlan(10000), Some(100));
        assert_eq!(orch.stats().vni_mappings_created, 1);
    }

    #[test]
    fn test_duplicate_vrf_update() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let config1 = VrfConfig::new("Vrf1").with_v4(true);
        orch.add_vrf(&config1).unwrap();

        // Adding same VRF again updates it
        let config2 = VrfConfig::new("Vrf1").with_v6(false);
        let _vrf_id2 = orch.add_vrf(&config2).unwrap();

        // Should still be 1 VRF
        assert_eq!(orch.vrf_count(), 1);
        let entry = orch.get_vrf("Vrf1").unwrap();
        assert!(!entry.admin_v6_state);
        assert_eq!(orch.stats().vrfs_created, 1);
        assert_eq!(orch.stats().vrfs_updated, 1);
    }

    #[test]
    fn test_remove_nonexistent_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let result = orch.remove_vrf("NonExistent");
        assert!(matches!(result, Err(VrfOrchError::VrfNotFound(_))));
    }

    #[test]
    fn test_vrf_name_validation() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        // Empty name
        let config = VrfConfig::new("");
        let result = orch.add_vrf(&config);
        assert!(result.is_ok()); // Empty names are allowed

        // Special characters
        let config = VrfConfig::new("Vrf-Test_123");
        let result = orch.add_vrf(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_vrf_creation() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let vrf1_id = orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        let vrf2_id = orch.add_vrf(&VrfConfig::new("Vrf2")).unwrap();
        let vrf3_id = orch.add_vrf(&VrfConfig::new("Vrf3")).unwrap();

        assert_eq!(orch.vrf_count(), 3);
        assert!(vrf1_id != vrf2_id);
        assert!(vrf2_id != vrf3_id);
        assert_eq!(orch.stats().vrfs_created, 3);
    }

    // ========== Router Interface Management Tests ==========

    #[test]
    fn test_rif_reference_counting() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();

        // Simulate adding router interfaces
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 0);

        orch.increase_vrf_ref_count("Vrf1").unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 1);

        orch.increase_vrf_ref_count("Vrf1").unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 2);

        orch.increase_vrf_ref_count("Vrf1").unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 3);

        orch.decrease_vrf_ref_count("Vrf1").unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 2);
    }

    #[test]
    fn test_rif_in_default_vs_custom_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::new(0x1000));

        // Default VRF ref count operations (should be no-op)
        assert_eq!(orch.increase_vrf_ref_count_by_id(0x1000).unwrap(), 0);
        assert_eq!(orch.decrease_vrf_ref_count_by_id(0x1000).unwrap(), 0);

        // Custom VRF
        let vrf_id = orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        assert_eq!(orch.increase_vrf_ref_count_by_id(vrf_id).unwrap(), 1);
        assert_eq!(orch.decrease_vrf_ref_count_by_id(vrf_id).unwrap(), 0);
    }

    #[test]
    fn test_multiple_rifs_per_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();

        // Simulate multiple RIFs
        for _ in 0..10 {
            orch.increase_vrf_ref_count("Vrf1").unwrap();
        }

        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 10);

        // Remove half
        for _ in 0..5 {
            orch.decrease_vrf_ref_count("Vrf1").unwrap();
        }

        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 5);
    }

    // ========== VRF Tables Tests ==========

    #[test]
    fn test_vrf_table_oid_assignment() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let vrf1_id = orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        let vrf2_id = orch.add_vrf(&VrfConfig::new("Vrf2")).unwrap();

        // Each VRF should have unique ID
        assert!(vrf1_id != vrf2_id);
        assert!(vrf1_id != 0);
        assert!(vrf2_id != 0);

        // IDs should be retrievable by name
        assert_eq!(orch.get_vrf_id("Vrf1"), vrf1_id);
        assert_eq!(orch.get_vrf_id("Vrf2"), vrf2_id);

        // Names should be retrievable by ID
        assert_eq!(orch.get_vrf_name(vrf1_id), "Vrf1");
        assert_eq!(orch.get_vrf_name(vrf2_id), "Vrf2");
    }

    #[test]
    fn test_default_vrf_tables() {
        let orch = VrfOrch::new(VrfOrchConfig::new(0x1000));

        // Default VRF should return global VRF ID
        assert_eq!(orch.get_vrf_id(""), 0x1000);
        assert_eq!(orch.get_vrf_id("NonExistent"), 0x1000);
        assert_eq!(orch.get_vrf_name(0x1000), "");
    }

    #[test]
    fn test_ipv4_ipv6_route_table_per_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let config = VrfConfig::new("Vrf1").with_v4(true).with_v6(false);
        orch.add_vrf(&config).unwrap();

        let entry = orch.get_vrf("Vrf1").unwrap();
        assert!(entry.admin_v4_state);
        assert!(!entry.admin_v6_state);

        // Update to enable IPv6
        let config2 = VrfConfig::new("Vrf1").with_v6(true);
        orch.add_vrf(&config2).unwrap();

        let entry = orch.get_vrf("Vrf1").unwrap();
        assert!(entry.admin_v4_state); // Still enabled
        assert!(entry.admin_v6_state); // Now enabled
    }

    // ========== Reference Counting Tests ==========

    #[test]
    fn test_cannot_remove_vrf_with_active_rifs() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        orch.increase_vrf_ref_count("Vrf1").unwrap();

        // Should fail to remove
        let result = orch.remove_vrf("Vrf1");
        assert!(matches!(result, Err(VrfOrchError::VrfInUse(_, 1))));

        // After removing RIF, should succeed
        orch.decrease_vrf_ref_count("Vrf1").unwrap();
        assert!(orch.remove_vrf("Vrf1").is_ok());
    }

    #[test]
    fn test_ref_count_cleanup_on_removal() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let vrf_id = orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        orch.remove_vrf("Vrf1").unwrap();

        // After removal, lookups should fail
        assert!(!orch.vrf_exists("Vrf1"));
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), -1);
        assert_eq!(orch.get_vrf_name(vrf_id), "");
    }

    #[test]
    fn test_ref_count_underflow_protection() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();

        // Cannot decrease below 0
        let result = orch.decrease_vrf_ref_count("Vrf1");
        assert!(matches!(result, Err(VrfOrchError::InvalidConfig(_))));

        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 0);
    }

    #[test]
    fn test_ref_count_by_id_nonexistent() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let result = orch.increase_vrf_ref_count_by_id(0x9999);
        assert!(matches!(result, Err(VrfOrchError::VrfNotFound(_))));

        let result = orch.decrease_vrf_ref_count_by_id(0x9999);
        assert!(matches!(result, Err(VrfOrchError::VrfNotFound(_))));
    }

    // ========== VNI Management Tests ==========

    #[test]
    fn test_vni_to_vrf_mapping() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        struct MockCallbacks;
        impl VrfOrchCallbacks for MockCallbacks {
            fn has_evpn_vtep(&self) -> bool {
                true
            }
            fn get_vlan_mapped_to_vni(&self, _vni: Vni) -> Option<VrfVlanId> {
                Some(100)
            }
        }
        orch.set_callbacks(Arc::new(MockCallbacks));

        orch.add_vrf(&VrfConfig::new("Vrf1").with_vni(10000))
            .unwrap();

        assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 10000);
        assert!(orch.is_l3_vni(10000));
    }

    #[test]
    fn test_unique_vni_per_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        struct MockCallbacks;
        impl VrfOrchCallbacks for MockCallbacks {
            fn has_evpn_vtep(&self) -> bool {
                true
            }
            fn get_vlan_mapped_to_vni(&self, _vni: Vni) -> Option<VrfVlanId> {
                Some(100)
            }
        }
        orch.set_callbacks(Arc::new(MockCallbacks));

        orch.add_vrf(&VrfConfig::new("Vrf1").with_vni(10000))
            .unwrap();
        orch.add_vrf(&VrfConfig::new("Vrf2").with_vni(20000))
            .unwrap();

        assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 10000);
        assert_eq!(orch.get_vrf_mapped_vni("Vrf2"), 20000);
        assert!(orch.is_l3_vni(10000));
        assert!(orch.is_l3_vni(20000));
    }

    #[test]
    fn test_updating_vrf_vni() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        struct MockCallbacks;
        impl VrfOrchCallbacks for MockCallbacks {
            fn has_evpn_vtep(&self) -> bool {
                true
            }
            fn get_vlan_mapped_to_vni(&self, _vni: Vni) -> Option<VrfVlanId> {
                Some(100)
            }
        }
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Create VRF with VNI
        orch.add_vrf(&VrfConfig::new("Vrf1").with_vni(10000))
            .unwrap();
        assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 10000);
        assert_eq!(orch.stats().vni_mappings_created, 1);

        // Update to new VNI - VRF-to-VNI map updated but old L3VNI entry remains
        orch.add_vrf(&VrfConfig::new("Vrf1").with_vni(20000))
            .unwrap();
        assert_eq!(orch.get_vrf_mapped_vni("Vrf1"), 20000);

        // The VRF should now be mapped to new VNI
        assert!(orch.is_l3_vni(20000));

        // Old L3VNI entry still exists (not automatically cleaned up)
        assert!(orch.is_l3_vni(10000));

        // Statistics show new mapping created (old not removed by update)
        assert_eq!(orch.stats().vni_mappings_created, 2);
    }

    #[test]
    fn test_vni_lookups() {
        let orch = VrfOrch::new(VrfOrchConfig::default());

        // Non-existent VNI
        assert_eq!(orch.get_vrf_mapped_vni("NonExistent"), 0);
        assert!(!orch.is_l3_vni(99999));
        assert_eq!(orch.get_l3_vni_vlan(99999), None);
    }

    #[test]
    fn test_vni_removal_with_vrf() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        struct MockCallbacks;
        impl VrfOrchCallbacks for MockCallbacks {
            fn has_evpn_vtep(&self) -> bool {
                true
            }
            fn get_vlan_mapped_to_vni(&self, _vni: Vni) -> Option<VrfVlanId> {
                Some(100)
            }
        }
        orch.set_callbacks(Arc::new(MockCallbacks));

        orch.add_vrf(&VrfConfig::new("Vrf1").with_vni(10000))
            .unwrap();
        assert!(orch.is_l3_vni(10000));

        // Remove VRF should remove VNI mapping
        orch.remove_vrf("Vrf1").unwrap();
        assert!(!orch.is_l3_vni(10000));
        assert_eq!(orch.stats().vni_mappings_removed, 1);
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_vrf_not_found_errors() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let result = orch.increase_vrf_ref_count("NonExistent");
        assert!(matches!(result, Err(VrfOrchError::VrfNotFound(_))));

        let result = orch.decrease_vrf_ref_count("NonExistent");
        assert!(matches!(result, Err(VrfOrchError::VrfNotFound(_))));

        let result = orch.remove_vrf("NonExistent");
        assert!(matches!(result, Err(VrfOrchError::VrfNotFound(_))));
    }

    #[test]
    fn test_vni_not_found_error() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let result = orch.update_l3_vni_vlan(99999, 100);
        assert!(matches!(result, Err(VrfOrchError::VniNotFound(99999))));
    }

    #[test]
    fn test_invalid_vni_without_evpn_vtep() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        // No callbacks or callbacks without EVPN VTEP
        let config = VrfConfig::new("Vrf1").with_vni(10000);
        let result = orch.add_vrf(&config);

        // Should fail without EVPN VTEP
        assert!(matches!(result, Err(VrfOrchError::CallbackError(_))));
    }

    // ========== Statistics Tracking Tests ==========

    #[test]
    fn test_statistics_tracking() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        assert_eq!(orch.stats().vrfs_created, 0);
        assert_eq!(orch.stats().vrfs_removed, 0);
        assert_eq!(orch.stats().vrfs_updated, 0);

        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        assert_eq!(orch.stats().vrfs_created, 1);

        orch.add_vrf(&VrfConfig::new("Vrf1").with_v4(false))
            .unwrap();
        assert_eq!(orch.stats().vrfs_updated, 1);

        orch.remove_vrf("Vrf1").unwrap();
        assert_eq!(orch.stats().vrfs_removed, 1);
    }

    #[test]
    fn test_vni_mapping_statistics() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        struct MockCallbacks;
        impl VrfOrchCallbacks for MockCallbacks {
            fn has_evpn_vtep(&self) -> bool {
                true
            }
            fn get_vlan_mapped_to_vni(&self, _vni: Vni) -> Option<VrfVlanId> {
                Some(100)
            }
        }
        orch.set_callbacks(Arc::new(MockCallbacks));

        assert_eq!(orch.stats().vni_mappings_created, 0);
        assert_eq!(orch.stats().vni_mappings_removed, 0);

        orch.add_vrf(&VrfConfig::new("Vrf1").with_vni(10000))
            .unwrap();
        assert_eq!(orch.stats().vni_mappings_created, 1);

        orch.remove_vrf("Vrf1").unwrap();
        assert_eq!(orch.stats().vni_mappings_removed, 1);
    }

    // ========== Edge Cases Tests ==========

    #[test]
    fn test_vrf_with_no_rifs() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 0);

        // Should be able to remove immediately
        assert!(orch.remove_vrf("Vrf1").is_ok());
    }

    #[test]
    fn test_multiple_vrfs_with_rifs() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        // Create multiple VRFs with RIFs
        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        orch.add_vrf(&VrfConfig::new("Vrf2")).unwrap();
        orch.add_vrf(&VrfConfig::new("Vrf3")).unwrap();

        orch.increase_vrf_ref_count("Vrf1").unwrap();
        orch.increase_vrf_ref_count("Vrf1").unwrap();
        orch.increase_vrf_ref_count("Vrf2").unwrap();
        orch.increase_vrf_ref_count("Vrf3").unwrap();
        orch.increase_vrf_ref_count("Vrf3").unwrap();
        orch.increase_vrf_ref_count("Vrf3").unwrap();

        assert_eq!(orch.get_vrf_ref_count("Vrf1"), 2);
        assert_eq!(orch.get_vrf_ref_count("Vrf2"), 1);
        assert_eq!(orch.get_vrf_ref_count("Vrf3"), 3);
    }

    #[test]
    fn test_vrf_cleanup_on_removal() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let vrf_id = orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        assert_eq!(orch.vrf_count(), 1);

        orch.remove_vrf("Vrf1").unwrap();

        // All state should be cleaned up
        assert_eq!(orch.vrf_count(), 0);
        assert!(!orch.vrf_exists("Vrf1"));
        assert_eq!(orch.get_vrf_name(vrf_id), "");
        assert!(orch.get_vrf("Vrf1").is_none());
        assert_eq!(orch.get_vrf_ref_count("Vrf1"), -1);
    }

    #[test]
    fn test_initialized_flag() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        assert!(!orch.is_initialized());

        orch.set_initialized(true);
        assert!(orch.is_initialized());

        orch.set_initialized(false);
        assert!(!orch.is_initialized());
    }

    #[test]
    fn test_vrf_with_all_configuration_options() {
        let mut orch = VrfOrch::new(VrfOrchConfig::default());

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let config = VrfConfig::new("Vrf1")
            .with_v4(true)
            .with_v6(true)
            .with_src_mac(mac)
            .with_ttl_action(PacketAction::Drop)
            .with_ip_opt_action(PacketAction::Trap)
            .with_l3_mc_action(PacketAction::Forward)
            .with_fallback(true);

        orch.add_vrf(&config).unwrap();

        let entry = orch.get_vrf("Vrf1").unwrap();
        assert!(entry.admin_v4_state);
        assert!(entry.admin_v6_state);
        assert_eq!(entry.src_mac, Some(mac));
        assert_eq!(entry.ttl_action, Some(PacketAction::Drop));
        assert_eq!(entry.ip_opt_action, Some(PacketAction::Trap));
        assert_eq!(entry.l3_mc_action, Some(PacketAction::Forward));
        assert!(entry.fallback);
    }
}
