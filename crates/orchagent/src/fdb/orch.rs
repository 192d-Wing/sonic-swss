//! FDB orchestration logic.

use super::types::{FdbEntry, FdbFlushStats, FdbKey, RawSaiObjectId};
use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::{audit_log, debug_log, info_log, warn_log, error_log};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Result type for FdbOrch operations.
pub type Result<T> = std::result::Result<T, FdbOrchError>;

/// FDB orchestration errors with NIST-compliant error messages.
#[derive(Debug, Clone, Error)]
pub enum FdbOrchError {
    /// FDB entry not found
    #[error("FDB entry not found: {0:?}")]
    EntryNotFound(FdbKey),
    /// FDB entry already exists
    #[error("FDB entry already exists: {0:?}")]
    EntryExists(FdbKey),
    /// Port not found
    #[error("Port not found: {0}")]
    PortNotFound(String),
    /// VLAN not found
    #[error("VLAN not found: {0}")]
    VlanNotFound(u16),
    /// Invalid MAC address
    #[error("Invalid MAC address: {0}")]
    InvalidMacAddress(String),
    /// SAI operation failed
    #[error("SAI operation failed: {0}")]
    SaiError(String),
    /// Callbacks not configured
    #[error("FDB orchestrator not initialized: callbacks not configured")]
    NotInitialized,
}

#[derive(Debug, Clone, Default)]
pub struct FdbOrchConfig {
    pub aging_time: u32,
    pub enable_flush_on_port_down: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FdbOrchStats {
    pub entries_added: u64,
    pub entries_removed: u64,
    pub entries_updated: u64,
    pub flush_stats: FdbFlushStats,
}

pub trait FdbOrchCallbacks: Send + Sync {
    /// Add an FDB entry to the forwarding database via SAI.
    fn add_fdb_entry(&self, entry: &FdbEntry) -> Result<()>;

    /// Remove an FDB entry from the forwarding database via SAI.
    fn remove_fdb_entry(&self, key: &FdbKey) -> Result<()>;

    /// Update an FDB entry in the forwarding database via SAI.
    fn update_fdb_entry(&self, key: &FdbKey, entry: &FdbEntry) -> Result<()>;

    /// Get an FDB entry from SAI by key.
    fn get_fdb_entry(&self, key: &FdbKey) -> Result<Option<FdbEntry>>;

    /// Flush FDB entries for a specific port (None flushes all).
    fn flush_entries_by_port(&self, port: Option<&str>) -> Result<u32>;

    /// Flush FDB entries for a specific VLAN (None flushes all).
    fn flush_entries_by_vlan(&self, vlan: Option<u16>) -> Result<u32>;

    /// Notification callback when entry is added.
    fn on_fdb_entry_added(&self, entry: &FdbEntry);

    /// Notification callback when entry is removed.
    fn on_fdb_entry_removed(&self, key: &FdbKey);

    /// Notification callback when entries are flushed.
    fn on_fdb_flush(&self, port: Option<&str>, vlan: Option<u16>, count: u32);
}

pub struct FdbOrch<C: FdbOrchCallbacks> {
    config: FdbOrchConfig,
    stats: FdbOrchStats,
    entries: HashMap<FdbKey, FdbEntry>,
    vlan_to_vlan_oid: HashMap<u16, RawSaiObjectId>,
    callbacks: Option<Arc<C>>,
}

impl<C: FdbOrchCallbacks> FdbOrch<C> {
    pub fn new(config: FdbOrchConfig) -> Self {
        Self {
            config,
            stats: FdbOrchStats::default(),
            entries: HashMap::new(),
            vlan_to_vlan_oid: HashMap::new(),
            callbacks: None,
        }
    }

    pub fn with_callbacks(mut self, callbacks: Arc<C>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    pub fn add_entry(&mut self, entry: FdbEntry) -> Result<()> {
        let key = entry.key.clone();
        debug_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, port = %entry.port_name, "Adding FDB entry");

        if self.entries.contains_key(&key) {
            warn_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, "FDB entry already exists");
            audit_log!(AuditRecord::new(
                AuditCategory::ResourceCreate,
                "FdbOrch",
                "add_entry"
            )
            .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
            .with_object_type("fdb_entry")
            .with_error(format!("Entry already exists: {:?}", key)));
            return Err(FdbOrchError::EntryExists(key));
        }

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| {
                error_log!("FdbOrch", "Callbacks not configured");
                FdbOrchError::NotInitialized
            })?;

        callbacks.add_fdb_entry(&entry).map_err(|e| {
            error_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, error = %e, "SAI add_fdb_entry failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "FdbOrch",
                "add_fdb_entry"
            )
            .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
            .with_object_type("fdb_entry")
            .with_error(e.to_string()));
            e
        })?;

        self.entries.insert(key.clone(), entry.clone());
        self.stats.entries_added += 1;
        callbacks.on_fdb_entry_added(&entry);

        info_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, port = %entry.port_name, "FDB entry created successfully");
        audit_log!(AuditRecord::new(
            AuditCategory::ResourceCreate,
            "FdbOrch",
            "add_entry"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
        .with_object_type("fdb_entry")
        .with_details(serde_json::json!({
            "mac_address": key.mac_address.to_string(),
            "vlan_id": key.vlan_id,
            "port_name": entry.port_name
        })));

        Ok(())
    }

    pub fn remove_entry(&mut self, key: &FdbKey) -> Result<()> {
        debug_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, "Removing FDB entry");

        self.entries.remove(key)
            .ok_or_else(|| {
                warn_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, "FDB entry not found for removal");
                audit_log!(AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "FdbOrch",
                    "remove_entry"
                )
                .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
                .with_object_type("fdb_entry")
                .with_error("Entry not found"));
                FdbOrchError::EntryNotFound(key.clone())
            })?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| {
                error_log!("FdbOrch", "Callbacks not configured");
                FdbOrchError::NotInitialized
            })?;

        callbacks.remove_fdb_entry(key).map_err(|e| {
            error_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, error = %e, "SAI remove_fdb_entry failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "FdbOrch",
                "remove_fdb_entry"
            )
            .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
            .with_object_type("fdb_entry")
            .with_error(e.to_string()));
            e
        })?;

        self.stats.entries_removed += 1;
        callbacks.on_fdb_entry_removed(key);

        info_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, "FDB entry removed successfully");
        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "FdbOrch",
            "remove_entry"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
        .with_object_type("fdb_entry")
        .with_details(serde_json::json!({
            "mac_address": key.mac_address.to_string(),
            "vlan_id": key.vlan_id
        })));

        Ok(())
    }

    pub fn update_entry(&mut self, key: &FdbKey, entry: FdbEntry) -> Result<()> {
        debug_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, port = %entry.port_name, "Updating FDB entry");

        let old_entry = self.entries.get(key)
            .ok_or_else(|| {
                warn_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, "FDB entry not found for update");
                FdbOrchError::EntryNotFound(key.clone())
            })?;

        let old_port = old_entry.port_name.clone();

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| {
                error_log!("FdbOrch", "Callbacks not configured");
                FdbOrchError::NotInitialized
            })?;

        callbacks.update_fdb_entry(key, &entry).map_err(|e| {
            error_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, error = %e, "SAI update_fdb_entry failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "FdbOrch",
                "update_fdb_entry"
            )
            .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
            .with_object_type("fdb_entry")
            .with_error(e.to_string()));
            e
        })?;

        self.entries.insert(key.clone(), entry.clone());
        self.stats.entries_updated += 1;

        info_log!("FdbOrch", mac = %key.mac_address, vlan = key.vlan_id, old_port = %old_port, new_port = %entry.port_name, "FDB entry updated successfully");
        audit_log!(AuditRecord::new(
            AuditCategory::ResourceModify,
            "FdbOrch",
            "update_entry"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(format!("{}:{}", key.mac_address, key.vlan_id))
        .with_object_type("fdb_entry")
        .with_details(serde_json::json!({
            "mac_address": key.mac_address.to_string(),
            "vlan_id": key.vlan_id,
            "old_port": old_port,
            "new_port": entry.port_name
        })));

        Ok(())
    }

    pub fn get_entry(&self, key: &FdbKey) -> Option<&FdbEntry> {
        self.entries.get(key)
    }

    pub fn get_entry_mut(&mut self, key: &FdbKey) -> Option<&mut FdbEntry> {
        self.entries.get_mut(key)
    }

    pub fn get_by_vlan(&self, vlan_id: u16) -> Vec<(FdbKey, &FdbEntry)> {
        self.entries.iter()
            .filter(|(k, _)| k.vlan_id == vlan_id)
            .map(|(k, v)| (k.clone(), v))
            .collect()
    }

    pub fn get_by_port(&self, port_name: &str) -> Vec<(FdbKey, &FdbEntry)> {
        self.entries.iter()
            .filter(|(_, v)| v.port_name == port_name)
            .map(|(k, v)| (k.clone(), v))
            .collect()
    }

    pub fn entry_exists(&self, key: &FdbKey) -> bool {
        self.entries.contains_key(key)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn flush_by_port(&mut self, port: Option<&str>) -> Result<u32> {
        debug_log!("FdbOrch", port = ?port, "Flushing FDB entries by port");

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| {
                error_log!("FdbOrch", "Callbacks not configured");
                FdbOrchError::NotInitialized
            })?;

        let count = callbacks.flush_entries_by_port(port).map_err(|e| {
            error_log!("FdbOrch", port = ?port, error = %e, "SAI flush_entries_by_port failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "FdbOrch",
                "flush_by_port"
            )
            .with_object_id(port.unwrap_or("all"))
            .with_object_type("fdb_flush")
            .with_error(e.to_string()));
            e
        })?;

        if let Some(port_name) = port {
            self.entries.retain(|_, v| v.port_name != port_name);
        } else {
            self.entries.clear();
        }

        self.stats.flush_stats.port_flushes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.stats.flush_stats.total_entries_flushed.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
        callbacks.on_fdb_flush(port, None, count);

        info_log!("FdbOrch", port = ?port, entries_flushed = count, "FDB entries flushed by port");
        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "FdbOrch",
            "flush_by_port"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(port.unwrap_or("all"))
        .with_object_type("fdb_flush")
        .with_details(serde_json::json!({
            "port": port,
            "entries_flushed": count
        })));

        Ok(count)
    }

    pub fn flush_by_vlan(&mut self, vlan: Option<u16>) -> Result<u32> {
        debug_log!("FdbOrch", vlan = ?vlan, "Flushing FDB entries by VLAN");

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| {
                error_log!("FdbOrch", "Callbacks not configured");
                FdbOrchError::NotInitialized
            })?;

        let count = callbacks.flush_entries_by_vlan(vlan).map_err(|e| {
            error_log!("FdbOrch", vlan = ?vlan, error = %e, "SAI flush_entries_by_vlan failed");
            audit_log!(AuditRecord::new(
                AuditCategory::SaiOperation,
                "FdbOrch",
                "flush_by_vlan"
            )
            .with_object_id(vlan.map(|v| v.to_string()).unwrap_or("all".to_string()))
            .with_object_type("fdb_flush")
            .with_error(e.to_string()));
            e
        })?;

        if let Some(vlan_id) = vlan {
            self.entries.retain(|k, _| k.vlan_id != vlan_id);
        } else {
            self.entries.clear();
        }

        self.stats.flush_stats.vlan_flushes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.stats.flush_stats.total_entries_flushed.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
        callbacks.on_fdb_flush(None, vlan, count);

        info_log!("FdbOrch", vlan = ?vlan, entries_flushed = count, "FDB entries flushed by VLAN");
        audit_log!(AuditRecord::new(
            AuditCategory::ResourceDelete,
            "FdbOrch",
            "flush_by_vlan"
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(vlan.map(|v| v.to_string()).unwrap_or("all".to_string()))
        .with_object_type("fdb_flush")
        .with_details(serde_json::json!({
            "vlan": vlan,
            "entries_flushed": count
        })));

        Ok(count)
    }

    pub fn stats(&self) -> &FdbOrchStats {
        &self.stats
    }

    pub fn config(&self) -> &FdbOrchConfig {
        &self.config
    }

    pub fn register_vlan(&mut self, vlan_id: u16, oid: RawSaiObjectId) {
        self.vlan_to_vlan_oid.insert(vlan_id, oid);
    }

    pub fn get_vlan_oid(&self, vlan_id: u16) -> Option<RawSaiObjectId> {
        self.vlan_to_vlan_oid.get(&vlan_id).copied()
    }

    pub fn unregister_vlan(&mut self, vlan_id: u16) -> Option<RawSaiObjectId> {
        self.vlan_to_vlan_oid.remove(&vlan_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::MacAddress;

    struct MockFdbCallbacks {
        add_called: std::sync::atomic::AtomicBool,
        remove_called: std::sync::atomic::AtomicBool,
    }

    impl MockFdbCallbacks {
        fn new() -> Self {
            Self {
                add_called: std::sync::atomic::AtomicBool::new(false),
                remove_called: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    impl FdbOrchCallbacks for MockFdbCallbacks {
        fn add_fdb_entry(&self, _entry: &FdbEntry) -> Result<()> {
            self.add_called.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }

        fn remove_fdb_entry(&self, _key: &FdbKey) -> Result<()> {
            self.remove_called.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }

        fn update_fdb_entry(&self, _key: &FdbKey, _entry: &FdbEntry) -> Result<()> {
            Ok(())
        }

        fn get_fdb_entry(&self, _key: &FdbKey) -> Result<Option<FdbEntry>> {
            Ok(None)
        }

        fn flush_entries_by_port(&self, _port: Option<&str>) -> Result<u32> {
            Ok(0)
        }

        fn flush_entries_by_vlan(&self, _vlan: Option<u16>) -> Result<u32> {
            Ok(0)
        }

        fn on_fdb_entry_added(&self, _entry: &FdbEntry) {}
        fn on_fdb_entry_removed(&self, _key: &FdbKey) {}
        fn on_fdb_flush(&self, _port: Option<&str>, _vlan: Option<u16>, _count: u32) {}
    }

    #[test]
    fn test_new_fdb_orch_with_default_config() {
        let config = FdbOrchConfig::default();
        let orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(config);

        assert_eq!(orch.stats().entries_added, 0);
        assert_eq!(orch.stats().entries_removed, 0);
        assert_eq!(orch.stats().entries_updated, 0);
    }

    #[test]
    fn test_new_fdb_orch_with_custom_config() {
        let config = FdbOrchConfig {
            aging_time: 300,
            enable_flush_on_port_down: true,
        };
        let orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(config.clone());

        assert_eq!(orch.config().aging_time, 300);
        assert_eq!(orch.config().enable_flush_on_port_down, true);
        assert_eq!(orch.stats().entries_added, 0);
    }

    #[test]
    fn test_add_entry() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let entry = FdbEntry::new(key.clone(), "Ethernet0".to_string());

        assert!(orch.add_entry(entry).is_ok());
        assert_eq!(orch.stats().entries_added, 1);
        assert_eq!(orch.entry_count(), 1);
        assert!(orch.entry_exists(&key));
    }

    #[test]
    fn test_add_duplicate_entry() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let entry = FdbEntry::new(key.clone(), "Ethernet0".to_string());

        assert!(orch.add_entry(entry.clone()).is_ok());
        assert!(orch.add_entry(entry).is_err());
        assert_eq!(orch.entry_count(), 1);
    }

    #[test]
    fn test_remove_entry() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let entry = FdbEntry::new(key.clone(), "Ethernet0".to_string());

        assert!(orch.add_entry(entry).is_ok());
        assert_eq!(orch.entry_count(), 1);

        assert!(orch.remove_entry(&key).is_ok());
        assert_eq!(orch.entry_count(), 0);
        assert_eq!(orch.stats().entries_removed, 1);
    }

    #[test]
    fn test_remove_nonexistent_entry() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);

        assert!(orch.remove_entry(&key).is_err());
        assert_eq!(orch.entry_count(), 0);
    }

    #[test]
    fn test_update_entry() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let mut entry = FdbEntry::new(key.clone(), "Ethernet0".to_string());

        assert!(orch.add_entry(entry.clone()).is_ok());
        assert_eq!(orch.entry_count(), 1);

        entry.port_name = "Ethernet4".to_string();
        assert!(orch.update_entry(&key, entry).is_ok());
        assert_eq!(orch.stats().entries_updated, 1);

        let updated = orch.get_entry(&key).unwrap();
        assert_eq!(updated.port_name, "Ethernet4");
    }

    #[test]
    fn test_update_nonexistent_entry() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let entry = FdbEntry::new(key.clone(), "Ethernet0".to_string());

        assert!(orch.update_entry(&key, entry).is_err());
    }

    #[test]
    fn test_get_entry() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let entry = FdbEntry::new(key.clone(), "Ethernet0".to_string());

        assert!(orch.add_entry(entry.clone()).is_ok());

        let retrieved = orch.get_entry(&key).unwrap();
        assert_eq!(retrieved.port_name, "Ethernet0");
        assert_eq!(retrieved.key, key);
    }

    #[test]
    fn test_get_by_vlan() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac1 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key1 = FdbKey::new(mac1, 100);
        let entry1 = FdbEntry::new(key1, "Ethernet0".to_string());

        let mac2 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x66]);
        let key2 = FdbKey::new(mac2, 100);
        let entry2 = FdbEntry::new(key2, "Ethernet1".to_string());

        let mac3 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x77]);
        let key3 = FdbKey::new(mac3, 200);
        let entry3 = FdbEntry::new(key3, "Ethernet2".to_string());

        assert!(orch.add_entry(entry1).is_ok());
        assert!(orch.add_entry(entry2).is_ok());
        assert!(orch.add_entry(entry3).is_ok());

        let vlan_100_entries = orch.get_by_vlan(100);
        assert_eq!(vlan_100_entries.len(), 2);

        let vlan_200_entries = orch.get_by_vlan(200);
        assert_eq!(vlan_200_entries.len(), 1);
    }

    #[test]
    fn test_get_by_port() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac1 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key1 = FdbKey::new(mac1, 100);
        let entry1 = FdbEntry::new(key1, "Ethernet0".to_string());

        let mac2 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x66]);
        let key2 = FdbKey::new(mac2, 100);
        let entry2 = FdbEntry::new(key2, "Ethernet0".to_string());

        let mac3 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x77]);
        let key3 = FdbKey::new(mac3, 100);
        let entry3 = FdbEntry::new(key3, "Ethernet1".to_string());

        assert!(orch.add_entry(entry1).is_ok());
        assert!(orch.add_entry(entry2).is_ok());
        assert!(orch.add_entry(entry3).is_ok());

        let eth0_entries = orch.get_by_port("Ethernet0");
        assert_eq!(eth0_entries.len(), 2);

        let eth1_entries = orch.get_by_port("Ethernet1");
        assert_eq!(eth1_entries.len(), 1);
    }

    #[test]
    fn test_flush_by_port() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac1 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key1 = FdbKey::new(mac1, 100);
        let entry1 = FdbEntry::new(key1, "Ethernet0".to_string());

        let mac2 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x66]);
        let key2 = FdbKey::new(mac2, 100);
        let entry2 = FdbEntry::new(key2, "Ethernet1".to_string());

        assert!(orch.add_entry(entry1).is_ok());
        assert!(orch.add_entry(entry2).is_ok());
        assert_eq!(orch.entry_count(), 2);

        assert!(orch.flush_by_port(Some("Ethernet0")).is_ok());
        assert_eq!(orch.entry_count(), 1);
    }

    #[test]
    fn test_flush_by_vlan() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let mac1 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key1 = FdbKey::new(mac1, 100);
        let entry1 = FdbEntry::new(key1, "Ethernet0".to_string());

        let mac2 = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x66]);
        let key2 = FdbKey::new(mac2, 200);
        let entry2 = FdbEntry::new(key2, "Ethernet1".to_string());

        assert!(orch.add_entry(entry1).is_ok());
        assert!(orch.add_entry(entry2).is_ok());
        assert_eq!(orch.entry_count(), 2);

        assert!(orch.flush_by_vlan(Some(100)).is_ok());
        assert_eq!(orch.entry_count(), 1);
    }

    #[test]
    fn test_add_entry_without_callbacks() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default());

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let entry = FdbEntry::new(key, "Ethernet0".to_string());

        assert!(orch.add_entry(entry).is_err());
    }

    #[test]
    fn test_entry_count() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        assert_eq!(orch.entry_count(), 0);

        let mac = MacAddress::new([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let key = FdbKey::new(mac, 100);
        let entry = FdbEntry::new(key, "Ethernet0".to_string());

        assert!(orch.add_entry(entry).is_ok());
        assert_eq!(orch.entry_count(), 1);
    }

    #[test]
    fn test_vlan_registration() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        let vlan_id = 100;
        let oid = 0x12345678;

        orch.register_vlan(vlan_id, oid);
        assert_eq!(orch.get_vlan_oid(vlan_id), Some(oid));

        let removed = orch.unregister_vlan(vlan_id);
        assert_eq!(removed, Some(oid));
        assert_eq!(orch.get_vlan_oid(vlan_id), None);
    }

    #[test]
    fn test_multiple_vlan_registrations() {
        let mut orch: FdbOrch<MockFdbCallbacks> = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(MockFdbCallbacks::new()));

        orch.register_vlan(100, 0x11111111);
        orch.register_vlan(200, 0x22222222);
        orch.register_vlan(300, 0x33333333);

        assert_eq!(orch.get_vlan_oid(100), Some(0x11111111));
        assert_eq!(orch.get_vlan_oid(200), Some(0x22222222));
        assert_eq!(orch.get_vlan_oid(300), Some(0x33333333));
    }
}
