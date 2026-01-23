//! Isolation group orchestration logic.

use super::types::{IsolationGroupConfig, IsolationGroupEntry, IsolationGroupType};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::audit::{AuditCategory, AuditOutcome, AuditRecord};
use crate::audit_log;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum IsolationGroupOrchError {
    #[error("Group not found: {0}")]
    GroupNotFound(String),
    #[error("Group exists: {0}")]
    GroupExists(String),
    #[error("Member not found: {0}")]
    MemberNotFound(String),
    #[error("Bind port not found: {0}")]
    BindPortNotFound(String),
    #[error("Port not found: {0}")]
    PortNotFound(String),
    #[error("Invalid type: {0}")]
    InvalidType(String),
    #[error("SAI error: {0}")]
    SaiError(String),
}

#[derive(Debug, Clone, Default)]
pub struct IsolationGroupOrchConfig {
    pub enable_isolation: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IsolationGroupOrchStats {
    pub groups_created: u64,
    pub groups_removed: u64,
    pub members_added: u64,
    pub members_removed: u64,
    pub bindings_added: u64,
    pub bindings_removed: u64,
}

pub trait IsolationGroupOrchCallbacks: Send + Sync {
    fn create_isolation_group(&self, group_type: IsolationGroupType) -> Result<RawSaiObjectId, String>;
    fn remove_isolation_group(&self, oid: RawSaiObjectId) -> Result<(), String>;
    fn add_isolation_group_member(&self, group_id: RawSaiObjectId, port_oid: RawSaiObjectId) -> Result<RawSaiObjectId, String>;
    fn remove_isolation_group_member(&self, member_oid: RawSaiObjectId) -> Result<(), String>;
    fn bind_isolation_group_to_port(&self, port_oid: RawSaiObjectId, group_id: RawSaiObjectId) -> Result<(), String>;
    fn unbind_isolation_group_from_port(&self, port_oid: RawSaiObjectId) -> Result<(), String>;
    fn get_port_oid(&self, alias: &str) -> Option<RawSaiObjectId>;
    fn get_bridge_port_oid(&self, alias: &str) -> Option<RawSaiObjectId>;
}

pub struct IsolationGroupOrch {
    config: IsolationGroupOrchConfig,
    stats: IsolationGroupOrchStats,
    callbacks: Option<Arc<dyn IsolationGroupOrchCallbacks>>,
    isolation_groups: HashMap<String, IsolationGroupEntry>,
}

impl IsolationGroupOrch {
    pub fn new(config: IsolationGroupOrchConfig) -> Self {
        Self {
            config,
            stats: IsolationGroupOrchStats::default(),
            callbacks: None,
            isolation_groups: HashMap::new(),
        }
    }

    pub fn set_callbacks(&mut self, callbacks: Arc<dyn IsolationGroupOrchCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    pub fn group_exists(&self, name: &str) -> bool {
        self.isolation_groups.contains_key(name)
    }

    pub fn get_group(&self, name: &str) -> Option<&IsolationGroupEntry> {
        self.isolation_groups.get(name)
    }

    pub fn get_group_mut(&mut self, name: &str) -> Option<&mut IsolationGroupEntry> {
        self.isolation_groups.get_mut(name)
    }

    pub fn create_isolation_group(&mut self, config: IsolationGroupConfig) -> Result<(), IsolationGroupOrchError> {
        if self.isolation_groups.contains_key(&config.name) {
            let audit_record = AuditRecord::new(
                AuditCategory::ResourceCreate,
                "IsolationGroupOrch",
                "create_isolation_group",
            )
            .with_outcome(AuditOutcome::Failure)
            .with_object_id(&config.name)
            .with_object_type("isolation_group")
            .with_error("Group already exists");
            audit_log!(audit_record);
            return Err(IsolationGroupOrchError::GroupExists(config.name.clone()));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| IsolationGroupOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let group_id = callbacks.create_isolation_group(config.group_type)
            .map_err(IsolationGroupOrchError::SaiError)?;

        let mut entry = IsolationGroupEntry::new(config.name.clone(), config.group_type, group_id);
        entry.description = config.description.clone();

        let group_type_str = match config.group_type {
            IsolationGroupType::Port => "PORT_ISOLATION",
            IsolationGroupType::BridgePort => "BRIDGE_PORT_ISOLATION",
        };

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "IsolationGroupOrch",
            "create_isolation_group",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(&config.name)
        .with_object_type("isolation_group")
        .with_details(serde_json::json!({
            "group_name": config.name,
            "group_type": group_type_str,
            "sai_oid": format!("0x{:x}", group_id),
            "description": config.description,
        }));
        audit_log!(audit_record);

        self.isolation_groups.insert(config.name.clone(), entry);
        self.stats.groups_created += 1;

        Ok(())
    }

    pub fn remove_isolation_group(&mut self, name: &str) -> Result<(), IsolationGroupOrchError> {
        let entry = self.isolation_groups.remove(name)
            .ok_or_else(|| {
                let audit_record = AuditRecord::new(
                    AuditCategory::ResourceDelete,
                    "IsolationGroupOrch",
                    "remove_isolation_group",
                )
                .with_outcome(AuditOutcome::Failure)
                .with_object_id(name)
                .with_object_type("isolation_group")
                .with_error("Group not found");
                audit_log!(audit_record);
                IsolationGroupOrchError::GroupNotFound(name.to_string())
            })?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| IsolationGroupOrchError::SaiError("No callbacks set".to_string()))?;

        // Remove all bindings first (bind_ports is Vec<String>, need to get OIDs)
        for port_alias in &entry.bind_ports {
            if let Some(port_oid) = match entry.group_type {
                IsolationGroupType::Port => callbacks.get_port_oid(port_alias),
                IsolationGroupType::BridgePort => callbacks.get_bridge_port_oid(port_alias),
            } {
                let _ = callbacks.unbind_isolation_group_from_port(port_oid);
            }
        }

        // Remove all members
        for (_, member_oid) in entry.members {
            let _ = callbacks.remove_isolation_group_member(member_oid);
        }

        // Remove group
        callbacks.remove_isolation_group(entry.oid)
            .map_err(IsolationGroupOrchError::SaiError)?;

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "IsolationGroupOrch",
            "remove_isolation_group",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(name)
        .with_object_type("isolation_group")
        .with_details(serde_json::json!({
            "group_name": name,
            "sai_oid": format!("0x{:x}", entry.oid),
            "members_removed": entry.members.len(),
            "bindings_removed": entry.bind_ports.len(),
        }));
        audit_log!(audit_record);

        self.stats.groups_removed += 1;

        Ok(())
    }

    pub fn add_isolation_group_member(&mut self, group_name: &str, member_alias: &str) -> Result<(), IsolationGroupOrchError> {
        let group = self.isolation_groups.get_mut(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?;

        if group.members.contains_key(member_alias) {
            return Ok(()); // Already a member
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| IsolationGroupOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let group_type = group.group_type;
        let group_oid = group.oid;

        let port_oid = match group_type {
            IsolationGroupType::Port => {
                callbacks.get_port_oid(member_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(member_alias.to_string()))?
            }
            IsolationGroupType::BridgePort => {
                callbacks.get_bridge_port_oid(member_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(member_alias.to_string()))?
            }
        };

        let member_oid = callbacks.add_isolation_group_member(group_oid, port_oid)
            .map_err(IsolationGroupOrchError::SaiError)?;

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceCreate,
            "IsolationGroupOrch",
            "add_isolation_group_member",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(group_name)
        .with_object_type("isolation_group_member")
        .with_details(serde_json::json!({
            "group_name": group_name,
            "member_name": member_alias,
            "group_oid": format!("0x{:x}", group_oid),
            "member_oid": format!("0x{:x}", member_oid),
            "member_port_oid": format!("0x{:x}", port_oid),
        }));
        audit_log!(audit_record);

        let group = self.isolation_groups.get_mut(group_name).unwrap();
        group.members.insert(member_alias.to_string(), member_oid);
        self.stats.members_added += 1;

        Ok(())
    }

    pub fn remove_isolation_group_member(&mut self, group_name: &str, member_alias: &str) -> Result<(), IsolationGroupOrchError> {
        let group = self.isolation_groups.get_mut(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?;

        let member_oid = group.members.remove(member_alias)
            .ok_or_else(|| IsolationGroupOrchError::MemberNotFound(member_alias.to_string()))?;

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| IsolationGroupOrchError::SaiError("No callbacks set".to_string()))?;

        callbacks.remove_isolation_group_member(member_oid)
            .map_err(IsolationGroupOrchError::SaiError)?;

        let audit_record = AuditRecord::new(
            AuditCategory::ResourceDelete,
            "IsolationGroupOrch",
            "remove_isolation_group_member",
        )
        .with_outcome(AuditOutcome::Success)
        .with_object_id(group_name)
        .with_object_type("isolation_group_member")
        .with_details(serde_json::json!({
            "group_name": group_name,
            "member_name": member_alias,
            "member_oid": format!("0x{:x}", member_oid),
            "action": "member_unbinding",
        }));
        audit_log!(audit_record);

        self.stats.members_removed += 1;

        Ok(())
    }

    pub fn bind_isolation_group(&mut self, group_name: &str, port_alias: &str) -> Result<(), IsolationGroupOrchError> {
        let group = self.isolation_groups.get_mut(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?;

        if group.bind_ports.contains(&port_alias.to_string()) {
            return Ok(()); // Already bound
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| IsolationGroupOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let group_oid = group.oid;
        let group_type = group.group_type;

        let port_oid = match group_type {
            IsolationGroupType::Port => {
                callbacks.get_port_oid(port_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(port_alias.to_string()))?
            }
            IsolationGroupType::BridgePort => {
                callbacks.get_bridge_port_oid(port_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(port_alias.to_string()))?
            }
        };

        callbacks.bind_isolation_group_to_port(port_oid, group_oid)
            .map_err(IsolationGroupOrchError::SaiError)?;

        let group = self.isolation_groups.get_mut(group_name).unwrap();
        group.bind_ports.push(port_alias.to_string());
        self.stats.bindings_added += 1;

        Ok(())
    }

    pub fn unbind_isolation_group(&mut self, group_name: &str, port_alias: &str) -> Result<(), IsolationGroupOrchError> {
        let group = self.isolation_groups.get_mut(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?;

        // Find and remove from bind_ports Vec
        let pos = group.bind_ports.iter().position(|p| p == port_alias)
            .ok_or_else(|| IsolationGroupOrchError::BindPortNotFound(port_alias.to_string()))?;
        group.bind_ports.remove(pos);

        let callbacks = self.callbacks.as_ref()
            .ok_or_else(|| IsolationGroupOrchError::SaiError("No callbacks set".to_string()))?;

        // Get port OID for unbinding
        let port_oid = match group.group_type {
            IsolationGroupType::Port => {
                callbacks.get_port_oid(port_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(port_alias.to_string()))?
            }
            IsolationGroupType::BridgePort => {
                callbacks.get_bridge_port_oid(port_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(port_alias.to_string()))?
            }
        };

        callbacks.unbind_isolation_group_from_port(port_oid)
            .map_err(IsolationGroupOrchError::SaiError)?;

        self.stats.bindings_removed += 1;

        Ok(())
    }

    pub fn add_pending_member(&mut self, group_name: &str, member_alias: &str) -> Result<(), IsolationGroupOrchError> {
        let group = self.isolation_groups.get_mut(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?;

        group.add_pending_member(member_alias.to_string());
        Ok(())
    }

    pub fn add_pending_bind_port(&mut self, group_name: &str, port_alias: &str) -> Result<(), IsolationGroupOrchError> {
        let group = self.isolation_groups.get_mut(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?;

        group.add_pending_bind_port(port_alias.to_string());
        Ok(())
    }

    pub fn process_pending_members(&mut self, group_name: &str) -> Result<(), IsolationGroupOrchError> {
        let pending_members: Vec<String> = self.isolation_groups
            .get(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?
            .pending_members
            .clone();

        for member_alias in pending_members {
            if let Ok(()) = self.add_isolation_group_member(group_name, &member_alias) {
                if let Some(group) = self.isolation_groups.get_mut(group_name) {
                    group.pending_members.retain(|m| m != &member_alias);
                }
            }
        }

        Ok(())
    }

    pub fn process_pending_bind_ports(&mut self, group_name: &str) -> Result<(), IsolationGroupOrchError> {
        let pending_bind_ports: Vec<String> = self.isolation_groups
            .get(group_name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(group_name.to_string()))?
            .pending_bind_ports
            .clone();

        for port_alias in pending_bind_ports {
            if let Ok(()) = self.bind_isolation_group(group_name, &port_alias) {
                if let Some(group) = self.isolation_groups.get_mut(group_name) {
                    group.pending_bind_ports.retain(|p| p != &port_alias);
                }
            }
        }

        Ok(())
    }

    pub fn stats(&self) -> &IsolationGroupOrchStats {
        &self.stats
    }

    pub fn group_count(&self) -> usize {
        self.isolation_groups.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCallbacks;

    impl IsolationGroupOrchCallbacks for MockCallbacks {
        fn create_isolation_group(&self, _group_type: IsolationGroupType) -> Result<RawSaiObjectId, String> {
            Ok(0x1000)
        }

        fn remove_isolation_group(&self, _oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn add_isolation_group_member(&self, _group_id: RawSaiObjectId, _port_oid: RawSaiObjectId) -> Result<RawSaiObjectId, String> {
            Ok(0x2000)
        }

        fn remove_isolation_group_member(&self, _member_oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn bind_isolation_group_to_port(&self, _port_oid: RawSaiObjectId, _group_id: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn unbind_isolation_group_from_port(&self, _port_oid: RawSaiObjectId) -> Result<(), String> {
            Ok(())
        }

        fn get_port_oid(&self, _alias: &str) -> Option<RawSaiObjectId> {
            Some(0x3000)
        }

        fn get_bridge_port_oid(&self, _alias: &str) -> Option<RawSaiObjectId> {
            Some(0x4000)
        }
    }

    #[test]
    fn test_create_isolation_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        assert!(orch.create_isolation_group(config).is_ok());
        assert_eq!(orch.group_count(), 1);
        assert_eq!(orch.stats().groups_created, 1);
    }

    #[test]
    fn test_add_member() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.add_isolation_group_member("group1", "Ethernet0").is_ok());
        assert_eq!(orch.stats().members_added, 1);
    }

    #[test]
    fn test_bind_port() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.bind_isolation_group("group1", "Ethernet0").is_ok());
        assert_eq!(orch.stats().bindings_added, 1);
    }

    #[test]
    fn test_pending_operations() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.add_pending_member("group1", "Ethernet0").is_ok());
        assert!(orch.add_pending_bind_port("group1", "Ethernet4").is_ok());

        assert!(orch.process_pending_members("group1").is_ok());
        assert!(orch.process_pending_bind_ports("group1").is_ok());
    }

    // ========== Isolation Group Management ==========

    #[test]
    fn test_create_duplicate_isolation_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config1 = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        assert!(orch.create_isolation_group(config1).is_ok());

        // Try to create duplicate
        let config2 = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        let result = orch.create_isolation_group(config2);
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupExists(_))));
        assert_eq!(orch.group_count(), 1);
    }

    #[test]
    fn test_remove_isolation_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();
        assert_eq!(orch.group_count(), 1);

        assert!(orch.remove_isolation_group("group1").is_ok());
        assert_eq!(orch.group_count(), 0);
        assert_eq!(orch.stats().groups_removed, 1);
    }

    #[test]
    fn test_remove_nonexistent_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.remove_isolation_group("nonexistent");
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupNotFound(_))));
    }

    #[test]
    fn test_create_bridge_port_isolation_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("bridge_group1".to_string(), IsolationGroupType::BridgePort);
        assert!(orch.create_isolation_group(config).is_ok());

        let group = orch.get_group("bridge_group1").unwrap();
        assert_eq!(group.group_type, IsolationGroupType::BridgePort);
    }

    #[test]
    fn test_group_exists() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        assert!(!orch.group_exists("group1"));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.group_exists("group1"));
        assert!(!orch.group_exists("group2"));
    }

    // ========== Member Operations ==========

    #[test]
    fn test_add_multiple_members_to_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.add_isolation_group_member("group1", "Ethernet0").is_ok());
        assert!(orch.add_isolation_group_member("group1", "Ethernet4").is_ok());
        assert!(orch.add_isolation_group_member("group1", "Ethernet8").is_ok());

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.members.len(), 3);
        assert_eq!(orch.stats().members_added, 3);
    }

    #[test]
    fn test_add_duplicate_member() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.add_isolation_group_member("group1", "Ethernet0").is_ok());
        // Adding same member again should succeed but not add duplicate
        assert!(orch.add_isolation_group_member("group1", "Ethernet0").is_ok());

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.members.len(), 1);
        assert_eq!(orch.stats().members_added, 1); // Only counted once
    }

    #[test]
    fn test_remove_isolation_group_member() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        orch.add_isolation_group_member("group1", "Ethernet0").unwrap();
        assert_eq!(orch.get_group("group1").unwrap().members.len(), 1);

        assert!(orch.remove_isolation_group_member("group1", "Ethernet0").is_ok());
        assert_eq!(orch.get_group("group1").unwrap().members.len(), 0);
        assert_eq!(orch.stats().members_removed, 1);
    }

    #[test]
    fn test_remove_nonexistent_member() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        let result = orch.remove_isolation_group_member("group1", "Ethernet0");
        assert!(matches!(result, Err(IsolationGroupOrchError::MemberNotFound(_))));
    }

    #[test]
    fn test_add_member_to_nonexistent_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.add_isolation_group_member("nonexistent", "Ethernet0");
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupNotFound(_))));
    }

    // ========== Port Isolation and Binding ==========

    #[test]
    fn test_bind_multiple_ports_to_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.bind_isolation_group("group1", "Ethernet0").is_ok());
        assert!(orch.bind_isolation_group("group1", "Ethernet4").is_ok());
        assert!(orch.bind_isolation_group("group1", "Ethernet8").is_ok());

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.bind_ports.len(), 3);
        assert_eq!(orch.stats().bindings_added, 3);
    }

    #[test]
    fn test_bind_duplicate_port() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        assert!(orch.bind_isolation_group("group1", "Ethernet0").is_ok());
        // Binding same port again should succeed but not add duplicate
        assert!(orch.bind_isolation_group("group1", "Ethernet0").is_ok());

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.bind_ports.len(), 1);
        assert_eq!(orch.stats().bindings_added, 1); // Only counted once
    }

    #[test]
    fn test_unbind_isolation_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        orch.bind_isolation_group("group1", "Ethernet0").unwrap();
        assert_eq!(orch.get_group("group1").unwrap().bind_ports.len(), 1);

        assert!(orch.unbind_isolation_group("group1", "Ethernet0").is_ok());
        assert_eq!(orch.get_group("group1").unwrap().bind_ports.len(), 0);
        assert_eq!(orch.stats().bindings_removed, 1);
    }

    #[test]
    fn test_unbind_nonexistent_port() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        let result = orch.unbind_isolation_group("group1", "Ethernet0");
        assert!(matches!(result, Err(IsolationGroupOrchError::BindPortNotFound(_))));
    }

    #[test]
    fn test_bind_to_nonexistent_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.bind_isolation_group("nonexistent", "Ethernet0");
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupNotFound(_))));
    }

    // ========== Group Types and Cross-Type Operations ==========

    #[test]
    fn test_bridge_port_member_operations() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("bridge_group".to_string(), IsolationGroupType::BridgePort);
        orch.create_isolation_group(config).unwrap();

        // Add bridge port members
        assert!(orch.add_isolation_group_member("bridge_group", "Ethernet0").is_ok());
        assert!(orch.bind_isolation_group("bridge_group", "Ethernet4").is_ok());

        let group = orch.get_group("bridge_group").unwrap();
        assert_eq!(group.group_type, IsolationGroupType::BridgePort);
        assert_eq!(group.members.len(), 1);
        assert_eq!(group.bind_ports.len(), 1);
    }

    #[test]
    fn test_multiple_groups_different_types() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let port_config = IsolationGroupConfig::new("port_group".to_string(), IsolationGroupType::Port);
        let bridge_config = IsolationGroupConfig::new("bridge_group".to_string(), IsolationGroupType::BridgePort);

        assert!(orch.create_isolation_group(port_config).is_ok());
        assert!(orch.create_isolation_group(bridge_config).is_ok());

        assert_eq!(orch.group_count(), 2);
        assert_eq!(orch.get_group("port_group").unwrap().group_type, IsolationGroupType::Port);
        assert_eq!(orch.get_group("bridge_group").unwrap().group_type, IsolationGroupType::BridgePort);
    }

    // ========== Reference Counting and Cleanup ==========

    #[test]
    fn test_remove_group_with_members_and_bindings() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        // Add members and bindings
        orch.add_isolation_group_member("group1", "Ethernet0").unwrap();
        orch.add_isolation_group_member("group1", "Ethernet4").unwrap();
        orch.bind_isolation_group("group1", "Ethernet8").unwrap();

        // Remove group should succeed and cleanup all members and bindings
        assert!(orch.remove_isolation_group("group1").is_ok());
        assert_eq!(orch.group_count(), 0);
        assert!(!orch.group_exists("group1"));
    }

    // ========== Statistics ==========

    #[test]
    fn test_comprehensive_statistics() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Create groups
        let config1 = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        let config2 = IsolationGroupConfig::new("group2".to_string(), IsolationGroupType::BridgePort);
        orch.create_isolation_group(config1).unwrap();
        orch.create_isolation_group(config2).unwrap();

        // Add members
        orch.add_isolation_group_member("group1", "Ethernet0").unwrap();
        orch.add_isolation_group_member("group1", "Ethernet4").unwrap();
        orch.add_isolation_group_member("group2", "Ethernet8").unwrap();

        // Bind ports
        orch.bind_isolation_group("group1", "Ethernet12").unwrap();
        orch.bind_isolation_group("group2", "Ethernet16").unwrap();

        // Remove operations
        orch.remove_isolation_group_member("group1", "Ethernet0").unwrap();
        orch.unbind_isolation_group("group1", "Ethernet12").unwrap();
        orch.remove_isolation_group("group2").unwrap();

        let stats = orch.stats();
        assert_eq!(stats.groups_created, 2);
        assert_eq!(stats.groups_removed, 1);
        assert_eq!(stats.members_added, 3);
        assert_eq!(stats.members_removed, 1);
        assert_eq!(stats.bindings_added, 2);
        assert_eq!(stats.bindings_removed, 1);
    }

    #[test]
    fn test_group_count_tracking() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        assert_eq!(orch.group_count(), 0);

        let config1 = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config1).unwrap();
        assert_eq!(orch.group_count(), 1);

        let config2 = IsolationGroupConfig::new("group2".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config2).unwrap();
        assert_eq!(orch.group_count(), 2);

        orch.remove_isolation_group("group1").unwrap();
        assert_eq!(orch.group_count(), 1);

        orch.remove_isolation_group("group2").unwrap();
        assert_eq!(orch.group_count(), 0);
    }

    // ========== Edge Cases ==========

    #[test]
    fn test_empty_isolation_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("empty_group".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        let group = orch.get_group("empty_group").unwrap();
        assert_eq!(group.members.len(), 0);
        assert_eq!(group.bind_ports.len(), 0);

        // Empty group should be removable
        assert!(orch.remove_isolation_group("empty_group").is_ok());
    }

    #[test]
    fn test_single_member_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("single_member".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        orch.add_isolation_group_member("single_member", "Ethernet0").unwrap();

        let group = orch.get_group("single_member").unwrap();
        assert_eq!(group.members.len(), 1);
        assert!(group.members.contains_key("Ethernet0"));
    }

    #[test]
    fn test_get_group_mut() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        // Test mutable access
        if let Some(group) = orch.get_group_mut("group1") {
            group.description = Some("Modified description".to_string());
        }

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.description, Some("Modified description".to_string()));

        // Test non-existent group
        assert!(orch.get_group_mut("nonexistent").is_none());
    }

    // ========== Pending Operations ==========

    #[test]
    fn test_add_pending_member_to_nonexistent_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.add_pending_member("nonexistent", "Ethernet0");
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupNotFound(_))));
    }

    #[test]
    fn test_add_pending_bind_port_to_nonexistent_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.add_pending_bind_port("nonexistent", "Ethernet0");
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupNotFound(_))));
    }

    #[test]
    fn test_process_pending_members_nonexistent_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.process_pending_members("nonexistent");
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupNotFound(_))));
    }

    #[test]
    fn test_process_pending_bind_ports_nonexistent_group() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let result = orch.process_pending_bind_ports("nonexistent");
        assert!(matches!(result, Err(IsolationGroupOrchError::GroupNotFound(_))));
    }

    #[test]
    fn test_multiple_pending_members_processing() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        // Add multiple pending members
        orch.add_pending_member("group1", "Ethernet0").unwrap();
        orch.add_pending_member("group1", "Ethernet4").unwrap();
        orch.add_pending_member("group1", "Ethernet8").unwrap();

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.pending_members.len(), 3);

        // Process all pending members
        orch.process_pending_members("group1").unwrap();

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.members.len(), 3);
        assert_eq!(group.pending_members.len(), 0);
    }

    // ========== Error Handling Without Callbacks ==========

    #[test]
    fn test_create_group_without_callbacks() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        // Don't set callbacks

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        let result = orch.create_isolation_group(config);
        assert!(matches!(result, Err(IsolationGroupOrchError::SaiError(_))));
    }

    #[test]
    fn test_add_member_without_callbacks() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port);
        orch.create_isolation_group(config).unwrap();

        // Remove callbacks
        orch.callbacks = None;

        let result = orch.add_isolation_group_member("group1", "Ethernet0");
        assert!(matches!(result, Err(IsolationGroupOrchError::SaiError(_))));
    }

    // ========== Complex Scenarios ==========

    #[test]
    fn test_group_with_description() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port)
            .with_description("VLAN isolation group".to_string());
        orch.create_isolation_group(config).unwrap();

        let group = orch.get_group("group1").unwrap();
        assert_eq!(group.description, Some("VLAN isolation group".to_string()));
    }

    #[test]
    fn test_isolation_scenario_pvlan() {
        let mut orch = IsolationGroupOrch::new(IsolationGroupOrchConfig::default());
        orch.set_callbacks(Arc::new(MockCallbacks));

        // Create a PVLAN-style isolation group
        let config = IsolationGroupConfig::new("pvlan_isolated".to_string(), IsolationGroupType::BridgePort)
            .with_description("Private VLAN isolated ports".to_string());
        orch.create_isolation_group(config).unwrap();

        // Add isolated ports as members (they can't talk to each other)
        orch.add_isolation_group_member("pvlan_isolated", "Ethernet0").unwrap();
        orch.add_isolation_group_member("pvlan_isolated", "Ethernet4").unwrap();
        orch.add_isolation_group_member("pvlan_isolated", "Ethernet8").unwrap();

        // Bind isolation to promiscuous port
        orch.bind_isolation_group("pvlan_isolated", "Ethernet12").unwrap();

        let group = orch.get_group("pvlan_isolated").unwrap();
        assert_eq!(group.members.len(), 3);
        assert_eq!(group.bind_ports.len(), 1);
        assert_eq!(group.group_type, IsolationGroupType::BridgePort);
    }
}
