//! Isolation group types and structures.

use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;

/// Isolation group type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IsolationGroupType {
    /// Port-level isolation.
    Port,
    /// Bridge port-level isolation.
    BridgePort,
}

impl IsolationGroupType {
    /// Parses an isolation group type from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "port" => Some(Self::Port),
            "bridge-port" | "bridge_port" => Some(Self::BridgePort),
            _ => None,
        }
    }

    /// Converts to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Port => "port",
            Self::BridgePort => "bridge-port",
        }
    }
}

/// Isolation group status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationGroupStatus {
    /// Operation successful.
    Success,
    /// Invalid parameter.
    InvalidParam,
    /// Operation failed.
    Fail,
    /// Retry needed (port not ready).
    Retry,
}

/// Isolation group configuration.
#[derive(Debug, Clone)]
pub struct IsolationGroupConfig {
    /// Group name.
    pub name: String,
    /// Group type (Port or BridgePort).
    pub group_type: IsolationGroupType,
    /// Description.
    pub description: Option<String>,
    /// Ports to bind (where isolation is applied).
    pub bind_ports: Vec<String>,
    /// Member ports (isolated ports).
    pub members: Vec<String>,
}

impl IsolationGroupConfig {
    /// Creates a new isolation group configuration.
    pub fn new(name: String, group_type: IsolationGroupType) -> Self {
        Self {
            name,
            group_type,
            description: None,
            bind_ports: Vec::new(),
            members: Vec::new(),
        }
    }

    /// Sets the description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Sets the bind ports.
    pub fn with_bind_ports(mut self, ports: Vec<String>) -> Self {
        self.bind_ports = ports;
        self
    }

    /// Sets the member ports.
    pub fn with_members(mut self, members: Vec<String>) -> Self {
        self.members = members;
        self
    }
}

/// Isolation group member information.
#[derive(Debug, Clone)]
pub struct IsolationGroupMember {
    /// Port alias.
    pub port_alias: String,
    /// SAI member OID.
    pub member_oid: RawSaiObjectId,
}

impl IsolationGroupMember {
    /// Creates a new isolation group member.
    pub fn new(port_alias: String, member_oid: RawSaiObjectId) -> Self {
        Self {
            port_alias,
            member_oid,
        }
    }
}

/// Isolation group entry.
#[derive(Debug, Clone)]
pub struct IsolationGroupEntry {
    /// Group name.
    pub name: String,
    /// Group type.
    pub group_type: IsolationGroupType,
    /// SAI isolation group OID.
    pub oid: RawSaiObjectId,
    /// Description.
    pub description: Option<String>,
    /// Member map: port alias → member OID.
    pub members: HashMap<String, RawSaiObjectId>,
    /// Ports where group is bound.
    pub bind_ports: Vec<String>,
    /// Pending members (ports not yet ready).
    pub pending_members: Vec<String>,
    /// Pending bind ports (ports not yet ready).
    pub pending_bind_ports: Vec<String>,
}

impl IsolationGroupEntry {
    /// Creates a new isolation group entry.
    pub fn new(name: String, group_type: IsolationGroupType, oid: RawSaiObjectId) -> Self {
        Self {
            name,
            group_type,
            oid,
            description: None,
            members: HashMap::new(),
            bind_ports: Vec::new(),
            pending_members: Vec::new(),
            pending_bind_ports: Vec::new(),
        }
    }

    /// Adds a member. Returns true if added, false if already existed.
    pub fn add_member(&mut self, port_alias: String, member_oid: RawSaiObjectId) -> bool {
        if self.members.contains_key(&port_alias) {
            false
        } else {
            self.members.insert(port_alias, member_oid);
            true
        }
    }

    /// Removes a member.
    pub fn remove_member(&mut self, port_alias: &str) -> Option<RawSaiObjectId> {
        self.members.remove(port_alias)
    }

    /// Gets a member OID.
    pub fn get_member_oid(&self, port_alias: &str) -> Option<RawSaiObjectId> {
        self.members.get(port_alias).copied()
    }

    /// Checks if a port is a member.
    pub fn is_member(&self, port_alias: &str) -> bool {
        self.members.contains_key(port_alias)
    }

    /// Adds a pending member.
    pub fn add_pending_member(&mut self, port_alias: String) {
        if !self.pending_members.contains(&port_alias) {
            self.pending_members.push(port_alias);
        }
    }

    /// Removes a pending member.
    pub fn remove_pending_member(&mut self, port_alias: &str) -> bool {
        if let Some(pos) = self.pending_members.iter().position(|p| p == port_alias) {
            self.pending_members.remove(pos);
            true
        } else {
            false
        }
    }

    /// Adds a pending bind port.
    pub fn add_pending_bind_port(&mut self, port_alias: String) {
        if !self.pending_bind_ports.contains(&port_alias) {
            self.pending_bind_ports.push(port_alias);
        }
    }

    /// Removes a pending bind port.
    pub fn remove_pending_bind_port(&mut self, port_alias: &str) -> bool {
        if let Some(pos) = self.pending_bind_ports.iter().position(|p| p == port_alias) {
            self.pending_bind_ports.remove(pos);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_group_type_parse() {
        assert_eq!(IsolationGroupType::parse("port"), Some(IsolationGroupType::Port));
        assert_eq!(IsolationGroupType::parse("PORT"), Some(IsolationGroupType::Port));
        assert_eq!(IsolationGroupType::parse("bridge-port"), Some(IsolationGroupType::BridgePort));
        assert_eq!(IsolationGroupType::parse("BRIDGE_PORT"), Some(IsolationGroupType::BridgePort));
        assert_eq!(IsolationGroupType::parse("invalid"), None);
    }

    #[test]
    fn test_isolation_group_config() {
        let config = IsolationGroupConfig::new("group1".to_string(), IsolationGroupType::Port)
            .with_description("Test group".to_string())
            .with_bind_ports(vec!["Ethernet0".to_string()])
            .with_members(vec!["Ethernet4".to_string(), "Ethernet8".to_string()]);

        assert_eq!(config.name, "group1");
        assert_eq!(config.group_type, IsolationGroupType::Port);
        assert_eq!(config.description, Some("Test group".to_string()));
        assert_eq!(config.bind_ports.len(), 1);
        assert_eq!(config.members.len(), 2);
    }

    #[test]
    fn test_isolation_group_entry() {
        let mut entry = IsolationGroupEntry::new(
            "group1".to_string(),
            IsolationGroupType::Port,
            0x1234,
        );

        // Add members
        assert!(entry.add_member("Ethernet0".to_string(), 0x5000));
        assert!(!entry.add_member("Ethernet0".to_string(), 0x6000)); // Duplicate

        assert!(entry.is_member("Ethernet0"));
        assert!(!entry.is_member("Ethernet4"));

        assert_eq!(entry.get_member_oid("Ethernet0"), Some(0x5000));
        assert_eq!(entry.get_member_oid("Ethernet4"), None);

        // Remove member
        assert_eq!(entry.remove_member("Ethernet0"), Some(0x5000));
        assert_eq!(entry.remove_member("Ethernet0"), None);

        // Pending members
        entry.add_pending_member("Ethernet8".to_string());
        entry.add_pending_member("Ethernet8".to_string()); // Duplicate check
        assert_eq!(entry.pending_members.len(), 1);

        assert!(entry.remove_pending_member("Ethernet8"));
        assert!(!entry.remove_pending_member("Ethernet8"));
        assert_eq!(entry.pending_members.len(), 0);
    }
}
