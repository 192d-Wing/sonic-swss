//! Isolation group orchestration logic.

use super::types::{IsolationGroupConfig, IsolationGroupEntry, IsolationGroupType};
use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum IsolationGroupOrchError {
    GroupNotFound(String),
    GroupExists(String),
    MemberNotFound(String),
    BindPortNotFound(String),
    PortNotFound(String),
    InvalidType(String),
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
            return Err(IsolationGroupOrchError::GroupExists(config.name.clone()));
        }

        let callbacks = Arc::clone(
            self.callbacks.as_ref()
                .ok_or_else(|| IsolationGroupOrchError::SaiError("No callbacks set".to_string()))?,
        );

        let group_id = callbacks.create_isolation_group(config.group_type)
            .map_err(IsolationGroupOrchError::SaiError)?;

        let mut entry = IsolationGroupEntry::new(config.name.clone(), config.group_type, group_id);
        entry.description = config.description;

        self.isolation_groups.insert(config.name.clone(), entry);
        self.stats.groups_created += 1;

        Ok(())
    }

    pub fn remove_isolation_group(&mut self, name: &str) -> Result<(), IsolationGroupOrchError> {
        let entry = self.isolation_groups.remove(name)
            .ok_or_else(|| IsolationGroupOrchError::GroupNotFound(name.to_string()))?;

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

        let port_oid = match group.group_type {
            IsolationGroupType::Port => {
                callbacks.get_port_oid(member_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(member_alias.to_string()))?
            }
            IsolationGroupType::BridgePort => {
                callbacks.get_bridge_port_oid(member_alias)
                    .ok_or_else(|| IsolationGroupOrchError::PortNotFound(member_alias.to_string()))?
            }
        };

        let member_oid = callbacks.add_isolation_group_member(group.oid, port_oid)
            .map_err(IsolationGroupOrchError::SaiError)?;

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
}
