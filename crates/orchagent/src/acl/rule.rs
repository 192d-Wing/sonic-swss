//! ACL rule types and implementation.
//!
//! An ACL rule represents a single entry in an ACL table, consisting of:
//! - Match conditions (what packets to match)
//! - Actions (what to do with matched packets)
//! - Priority (which rule wins on multiple matches)

use sonic_sai::types::RawSaiObjectId;
use sonic_types::{IpAddress, IpPrefix, MacAddress};
use std::collections::{HashMap, HashSet};
use std::fmt;

use super::range::AclRangeConfig;
use super::types::{
    AclActionType, AclMatchField, AclPacketAction, AclPriority, AclRuleId, MetaDataValue,
};

/// ACL rule type (determines what actions are available).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AclRuleType {
    /// Standard packet forwarding/dropping rule.
    #[default]
    Packet,
    /// Mirror rule (associated with a mirror session).
    Mirror,
    /// DTEL/INT watchlist rule.
    DtelWatchlist,
    /// Inner source MAC rewrite rule (for VXLAN).
    InnerSrcMacRewrite,
    /// Underlay DSCP setting rule.
    UnderlaySetDscp,
}

impl fmt::Display for AclRuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Packet => write!(f, "Packet"),
            Self::Mirror => write!(f, "Mirror"),
            Self::DtelWatchlist => write!(f, "DtelWatchlist"),
            Self::InnerSrcMacRewrite => write!(f, "InnerSrcMacRewrite"),
            Self::UnderlaySetDscp => write!(f, "UnderlaySetDscp"),
        }
    }
}

/// Match value for an ACL rule.
#[derive(Debug, Clone, PartialEq)]
pub enum AclMatchValue {
    /// IPv4 address with optional mask.
    Ipv4 {
        addr: IpAddress,
        mask: Option<IpAddress>,
    },
    /// IPv6 address with optional mask.
    Ipv6 {
        addr: IpAddress,
        mask: Option<IpAddress>,
    },
    /// IP prefix (address/mask combined).
    IpPrefix(IpPrefix),
    /// MAC address with optional mask.
    Mac {
        addr: MacAddress,
        mask: Option<MacAddress>,
    },
    /// Single unsigned value (port, protocol, DSCP, etc.).
    U8(u8),
    /// Single unsigned 16-bit value.
    U16(u16),
    /// Single unsigned 32-bit value.
    U32(u32),
    /// Port range (min, max).
    Range { min: u32, max: u32 },
    /// List of port OIDs (for IN_PORTS, OUT_PORTS).
    PortList(Vec<RawSaiObjectId>),
    /// Single port OID.
    Port(RawSaiObjectId),
    /// TCP flags with mask.
    TcpFlags { flags: u8, mask: u8 },
}

impl fmt::Display for AclMatchValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ipv4 { addr, mask } => {
                if let Some(m) = mask {
                    write!(f, "{}/{}", addr, m)
                } else {
                    write!(f, "{}", addr)
                }
            }
            Self::Ipv6 { addr, mask } => {
                if let Some(m) = mask {
                    write!(f, "{}/{}", addr, m)
                } else {
                    write!(f, "{}", addr)
                }
            }
            Self::IpPrefix(prefix) => write!(f, "{}", prefix),
            Self::Mac { addr, mask } => {
                if let Some(m) = mask {
                    write!(f, "{}/{}", addr, m)
                } else {
                    write!(f, "{}", addr)
                }
            }
            Self::U8(v) => write!(f, "{}", v),
            Self::U16(v) => write!(f, "{}", v),
            Self::U32(v) => write!(f, "{}", v),
            Self::Range { min, max } => write!(f, "{}-{}", min, max),
            Self::PortList(ports) => write!(f, "[{} ports]", ports.len()),
            Self::Port(oid) => write!(f, "0x{:x}", oid),
            Self::TcpFlags { flags, mask } => write!(f, "0x{:02x}/0x{:02x}", flags, mask),
        }
    }
}

/// A match condition in an ACL rule.
#[derive(Debug, Clone)]
pub struct AclRuleMatch {
    /// Match field type.
    pub field: AclMatchField,
    /// Match value.
    pub value: AclMatchValue,
}

impl AclRuleMatch {
    /// Creates a new match condition.
    pub fn new(field: AclMatchField, value: AclMatchValue) -> Self {
        Self { field, value }
    }

    /// Creates an IPv4 source IP match.
    pub fn src_ip(addr: IpAddress, mask: Option<IpAddress>) -> Self {
        Self::new(AclMatchField::SrcIp, AclMatchValue::Ipv4 { addr, mask })
    }

    /// Creates an IPv4 destination IP match.
    pub fn dst_ip(addr: IpAddress, mask: Option<IpAddress>) -> Self {
        Self::new(AclMatchField::DstIp, AclMatchValue::Ipv4 { addr, mask })
    }

    /// Creates an IP prefix match for source.
    pub fn src_ip_prefix(prefix: IpPrefix) -> Self {
        Self::new(AclMatchField::SrcIp, AclMatchValue::IpPrefix(prefix))
    }

    /// Creates an IP prefix match for destination.
    pub fn dst_ip_prefix(prefix: IpPrefix) -> Self {
        Self::new(AclMatchField::DstIp, AclMatchValue::IpPrefix(prefix))
    }

    /// Creates an IP protocol match.
    pub fn ip_protocol(protocol: u8) -> Self {
        Self::new(AclMatchField::IpProtocol, AclMatchValue::U8(protocol))
    }

    /// Creates a DSCP match.
    pub fn dscp(value: u8) -> Self {
        Self::new(AclMatchField::Dscp, AclMatchValue::U8(value))
    }

    /// Creates an L4 source port match.
    pub fn l4_src_port(port: u16) -> Self {
        Self::new(AclMatchField::L4SrcPort, AclMatchValue::U16(port))
    }

    /// Creates an L4 destination port match.
    pub fn l4_dst_port(port: u16) -> Self {
        Self::new(AclMatchField::L4DstPort, AclMatchValue::U16(port))
    }

    /// Creates an L4 source port range match.
    pub fn l4_src_port_range(min: u32, max: u32) -> Self {
        Self::new(
            AclMatchField::L4SrcPortRange,
            AclMatchValue::Range { min, max },
        )
    }

    /// Creates an L4 destination port range match.
    pub fn l4_dst_port_range(min: u32, max: u32) -> Self {
        Self::new(
            AclMatchField::L4DstPortRange,
            AclMatchValue::Range { min, max },
        )
    }

    /// Creates an IN_PORTS match.
    pub fn in_ports(ports: Vec<RawSaiObjectId>) -> Self {
        Self::new(AclMatchField::InPorts, AclMatchValue::PortList(ports))
    }

    /// Creates a TCP flags match.
    pub fn tcp_flags(flags: u8, mask: u8) -> Self {
        Self::new(
            AclMatchField::TcpFlags,
            AclMatchValue::TcpFlags { flags, mask },
        )
    }

    /// Creates an ether type match.
    pub fn ether_type(etype: u16) -> Self {
        Self::new(AclMatchField::EtherType, AclMatchValue::U16(etype))
    }
}

/// Redirect target for ACL redirect action.
#[derive(Debug, Clone, PartialEq)]
pub enum AclRedirectTarget {
    /// Redirect to a next-hop.
    NextHop(String),
    /// Redirect to a next-hop group.
    NextHopGroup(String),
    /// Redirect to a port (by alias).
    Port(String),
    /// Redirect to a port (by OID).
    PortOid(RawSaiObjectId),
    /// Redirect to a VXLAN tunnel endpoint.
    TunnelNextHop {
        tunnel_name: String,
        endpoint_ip: IpAddress,
        vni: Option<u32>,
        mac: Option<MacAddress>,
    },
}

impl fmt::Display for AclRedirectTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NextHop(nh) => write!(f, "NH:{}", nh),
            Self::NextHopGroup(nhg) => write!(f, "NHG:{}", nhg),
            Self::Port(alias) => write!(f, "PORT:{}", alias),
            Self::PortOid(oid) => write!(f, "PORT:0x{:x}", oid),
            Self::TunnelNextHop {
                tunnel_name,
                endpoint_ip,
                ..
            } => {
                write!(f, "TUNNEL:{}@{}", endpoint_ip, tunnel_name)
            }
        }
    }
}

/// Action value for an ACL rule.
#[derive(Debug, Clone, PartialEq)]
pub enum AclActionValue {
    /// Packet action (forward, drop, etc.).
    PacketAction(AclPacketAction),
    /// Redirect to target.
    Redirect(AclRedirectTarget),
    /// Mirror session name.
    Mirror(String),
    /// DSCP value to set.
    SetDscp(u8),
    /// Traffic class to set.
    SetTc(u8),
    /// Metadata value to set.
    SetMetaData(MetaDataValue),
    /// MAC address to set.
    SetMac(MacAddress),
    /// Boolean flag (for enable/disable actions).
    Bool(bool),
    /// Counter enable/disable.
    Counter(bool),
    /// Integer value (generic).
    U32(u32),
    /// DTEL flow operation value.
    DtelFlowOp(u32),
    /// DTEL INT session name.
    DtelIntSession(String),
}

impl fmt::Display for AclActionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketAction(action) => write!(f, "{}", action),
            Self::Redirect(target) => write!(f, "{}", target),
            Self::Mirror(session) => write!(f, "MIRROR:{}", session),
            Self::SetDscp(dscp) => write!(f, "DSCP:{}", dscp),
            Self::SetTc(tc) => write!(f, "TC:{}", tc),
            Self::SetMetaData(meta) => write!(f, "META:{}", meta),
            Self::SetMac(mac) => write!(f, "MAC:{}", mac),
            Self::Bool(v) => write!(f, "{}", v),
            Self::Counter(enabled) => write!(f, "COUNTER:{}", enabled),
            Self::U32(v) => write!(f, "{}", v),
            Self::DtelFlowOp(op) => write!(f, "FLOW_OP:{}", op),
            Self::DtelIntSession(session) => write!(f, "INT:{}", session),
        }
    }
}

/// An action in an ACL rule.
#[derive(Debug, Clone)]
pub struct AclRuleAction {
    /// Action type.
    pub action_type: AclActionType,
    /// Action value.
    pub value: AclActionValue,
}

impl AclRuleAction {
    /// Creates a new action.
    pub fn new(action_type: AclActionType, value: AclActionValue) -> Self {
        Self { action_type, value }
    }

    /// Creates a packet action (forward/drop/etc).
    pub fn packet_action(action: AclPacketAction) -> Self {
        Self::new(
            AclActionType::PacketAction,
            AclActionValue::PacketAction(action),
        )
    }

    /// Creates a drop action.
    pub fn drop() -> Self {
        Self::packet_action(AclPacketAction::Drop)
    }

    /// Creates a forward action.
    pub fn forward() -> Self {
        Self::packet_action(AclPacketAction::Forward)
    }

    /// Creates a redirect action.
    pub fn redirect(target: AclRedirectTarget) -> Self {
        Self::new(AclActionType::Redirect, AclActionValue::Redirect(target))
    }

    /// Creates a mirror action (ingress).
    pub fn mirror_ingress(session: impl Into<String>) -> Self {
        Self::new(
            AclActionType::MirrorIngress,
            AclActionValue::Mirror(session.into()),
        )
    }

    /// Creates a mirror action (egress).
    pub fn mirror_egress(session: impl Into<String>) -> Self {
        Self::new(
            AclActionType::MirrorEgress,
            AclActionValue::Mirror(session.into()),
        )
    }

    /// Creates a set DSCP action.
    pub fn set_dscp(dscp: u8) -> Self {
        Self::new(AclActionType::SetDscp, AclActionValue::SetDscp(dscp))
    }

    /// Creates a set metadata action.
    pub fn set_metadata(meta: MetaDataValue) -> Self {
        Self::new(
            AclActionType::SetAclMetaData,
            AclActionValue::SetMetaData(meta),
        )
    }

    /// Creates a counter enable action.
    pub fn counter(enabled: bool) -> Self {
        Self::new(AclActionType::Counter, AclActionValue::Counter(enabled))
    }
}

/// State of a rule that has deferred activation (mirror/DTEL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AclRuleState {
    /// Rule is pending creation (waiting for dependency).
    #[default]
    Pending,
    /// Rule is active in hardware.
    Active,
    /// Rule is inactive (dependency not available).
    Inactive,
}

impl fmt::Display for AclRuleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::Active => write!(f, "ACTIVE"),
            Self::Inactive => write!(f, "INACTIVE"),
        }
    }
}

/// ACL rule structure.
///
/// This represents a single rule within an ACL table. Each rule has:
/// - A unique ID within the table
/// - A priority (higher = more specific)
/// - Match conditions
/// - Actions to take on match
#[derive(Debug, Clone)]
pub struct AclRule {
    /// Rule ID (unique within the table).
    pub id: AclRuleId,
    /// Rule type (determines available actions).
    pub rule_type: AclRuleType,
    /// Priority (higher value = higher priority).
    pub priority: AclPriority,
    /// Match conditions.
    pub matches: HashMap<AclMatchField, AclRuleMatch>,
    /// Actions.
    pub actions: HashMap<AclActionType, AclRuleAction>,
    /// SAI rule object ID (0 if not created).
    pub rule_oid: RawSaiObjectId,
    /// SAI counter object ID (0 if no counter).
    pub counter_oid: RawSaiObjectId,
    /// Range configurations for port range matching.
    pub range_configs: Vec<AclRangeConfig>,
    /// State (for rules with deferred activation).
    pub state: AclRuleState,
    /// Whether counter is enabled.
    pub counter_enabled: bool,
    /// Mirror session name (for mirror rules).
    pub mirror_session: Option<String>,
    /// DTEL INT session name (for DTEL rules).
    pub int_session: Option<String>,
    /// Redirect target next-hop key (for caching).
    pub redirect_nh_key: Option<String>,
    /// Redirect target next-hop group key (for caching).
    pub redirect_nhg_key: Option<String>,
}

impl AclRule {
    /// Creates a new ACL rule with the given ID and type.
    pub fn new(id: impl Into<String>, rule_type: AclRuleType) -> Self {
        Self {
            id: id.into(),
            rule_type,
            priority: 0,
            matches: HashMap::new(),
            actions: HashMap::new(),
            rule_oid: 0,
            counter_oid: 0,
            range_configs: Vec::new(),
            state: AclRuleState::default(),
            counter_enabled: false,
            mirror_session: None,
            int_session: None,
            redirect_nh_key: None,
            redirect_nhg_key: None,
        }
    }

    /// Creates a new packet rule.
    pub fn packet(id: impl Into<String>) -> Self {
        Self::new(id, AclRuleType::Packet)
    }

    /// Creates a new mirror rule.
    pub fn mirror(id: impl Into<String>) -> Self {
        Self::new(id, AclRuleType::Mirror)
    }

    /// Sets the priority.
    pub fn with_priority(mut self, priority: AclPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Adds a match condition.
    pub fn with_match(mut self, match_cond: AclRuleMatch) -> Self {
        self.matches.insert(match_cond.field, match_cond);
        self
    }

    /// Adds an action.
    pub fn with_action(mut self, action: AclRuleAction) -> Self {
        self.actions.insert(action.action_type, action);
        self
    }

    /// Enables the counter.
    pub fn with_counter(mut self, enabled: bool) -> Self {
        self.counter_enabled = enabled;
        self
    }

    /// Adds a match condition.
    pub fn add_match(&mut self, match_cond: AclRuleMatch) {
        self.matches.insert(match_cond.field, match_cond);
    }

    /// Adds an action.
    pub fn add_action(&mut self, action: AclRuleAction) {
        self.actions.insert(action.action_type, action);
    }

    /// Sets the priority.
    pub fn set_priority(&mut self, priority: AclPriority) {
        self.priority = priority;
    }

    /// Returns true if the rule has the given match field.
    pub fn has_match(&self, field: AclMatchField) -> bool {
        self.matches.contains_key(&field)
    }

    /// Returns true if the rule has the given action type.
    pub fn has_action(&self, action_type: AclActionType) -> bool {
        self.actions.contains_key(&action_type)
    }

    /// Returns the match value for a field (if present).
    pub fn get_match(&self, field: AclMatchField) -> Option<&AclRuleMatch> {
        self.matches.get(&field)
    }

    /// Returns the action for a type (if present).
    pub fn get_action(&self, action_type: AclActionType) -> Option<&AclRuleAction> {
        self.actions.get(&action_type)
    }

    /// Returns true if the rule is created in SAI.
    pub fn is_created(&self) -> bool {
        self.rule_oid != 0
    }

    /// Returns true if this rule requires deferred activation.
    pub fn requires_deferred_activation(&self) -> bool {
        matches!(
            self.rule_type,
            AclRuleType::Mirror | AclRuleType::DtelWatchlist
        )
    }

    /// Returns true if this rule is active.
    pub fn is_active(&self) -> bool {
        self.state == AclRuleState::Active
    }

    /// Returns true if this rule has a counter.
    pub fn has_counter(&self) -> bool {
        self.counter_oid != 0
    }

    /// Returns the IN_PORTS match value (if present).
    pub fn get_in_ports(&self) -> Option<&Vec<RawSaiObjectId>> {
        self.get_match(AclMatchField::InPorts).and_then(|m| {
            if let AclMatchValue::PortList(ports) = &m.value {
                Some(ports)
            } else {
                None
            }
        })
    }

    /// Updates the IN_PORTS match value.
    pub fn update_in_ports(&mut self, ports: Vec<RawSaiObjectId>) {
        let match_cond = AclRuleMatch::in_ports(ports);
        self.matches.insert(AclMatchField::InPorts, match_cond);
    }

    /// Returns all match fields used by this rule.
    pub fn match_fields(&self) -> HashSet<AclMatchField> {
        self.matches.keys().copied().collect()
    }

    /// Returns all action types used by this rule.
    pub fn action_types(&self) -> HashSet<AclActionType> {
        self.actions.keys().copied().collect()
    }

    /// Validates the rule.
    pub fn validate(
        &self,
        min_priority: AclPriority,
        max_priority: AclPriority,
    ) -> Result<(), String> {
        // Validate priority
        if self.priority < min_priority || self.priority > max_priority {
            return Err(format!(
                "Priority {} out of range ({}-{})",
                self.priority, min_priority, max_priority
            ));
        }

        // Must have at least one match or action
        if self.matches.is_empty() && self.actions.is_empty() {
            return Err("Rule must have at least one match or action".to_string());
        }

        // Validate rule-type specific requirements
        match self.rule_type {
            AclRuleType::Mirror => {
                if !self.has_action(AclActionType::MirrorIngress)
                    && !self.has_action(AclActionType::MirrorEgress)
                {
                    return Err("Mirror rule must have a mirror action".to_string());
                }
            }
            AclRuleType::DtelWatchlist => {
                if !self.has_action(AclActionType::DtelFlowOp) {
                    return Err("DTEL rule must have a flow operation action".to_string());
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl fmt::Display for AclRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AclRule({}, type={}, priority={}, matches={}, actions={}, state={})",
            self.id,
            self.rule_type,
            self.priority,
            self.matches.len(),
            self.actions.len(),
            self.state
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_type_display() {
        assert_eq!(AclRuleType::Packet.to_string(), "Packet");
        assert_eq!(AclRuleType::Mirror.to_string(), "Mirror");
    }

    #[test]
    fn test_rule_basic() {
        let rule = AclRule::packet("rule1")
            .with_priority(100)
            .with_match(AclRuleMatch::ip_protocol(6)) // TCP
            .with_match(AclRuleMatch::l4_dst_port(80)) // HTTP
            .with_action(AclRuleAction::drop())
            .with_counter(true);

        assert_eq!(rule.id, "rule1");
        assert_eq!(rule.rule_type, AclRuleType::Packet);
        assert_eq!(rule.priority, 100);
        assert!(rule.has_match(AclMatchField::IpProtocol));
        assert!(rule.has_match(AclMatchField::L4DstPort));
        assert!(rule.has_action(AclActionType::PacketAction));
        assert!(rule.counter_enabled);
    }

    #[test]
    fn test_rule_validate() {
        let rule = AclRule::packet("rule1")
            .with_priority(100)
            .with_action(AclRuleAction::drop());

        // Valid
        assert!(rule.validate(0, 1000).is_ok());

        // Priority out of range
        assert!(rule.validate(200, 1000).is_err());
        assert!(rule.validate(0, 50).is_err());

        // Empty rule
        let empty_rule = AclRule::packet("empty");
        assert!(empty_rule.validate(0, 1000).is_err());
    }

    #[test]
    fn test_mirror_rule_validate() {
        // Missing mirror action
        let rule = AclRule::mirror("mirror1").with_priority(100);
        assert!(rule.validate(0, 1000).is_err());

        // With mirror action
        let rule = AclRule::mirror("mirror1")
            .with_priority(100)
            .with_action(AclRuleAction::mirror_ingress("session1"));
        assert!(rule.validate(0, 1000).is_ok());
    }

    #[test]
    fn test_match_helpers() {
        let m = AclRuleMatch::l4_src_port_range(1000, 2000);
        assert_eq!(m.field, AclMatchField::L4SrcPortRange);
        if let AclMatchValue::Range { min, max } = m.value {
            assert_eq!(min, 1000);
            assert_eq!(max, 2000);
        } else {
            panic!("Expected Range value");
        }
    }

    #[test]
    fn test_action_helpers() {
        let a = AclRuleAction::set_dscp(46);
        assert_eq!(a.action_type, AclActionType::SetDscp);
        if let AclActionValue::SetDscp(dscp) = a.value {
            assert_eq!(dscp, 46);
        } else {
            panic!("Expected SetDscp value");
        }
    }

    #[test]
    fn test_redirect_target_display() {
        let t = AclRedirectTarget::NextHop("10.0.0.1@Ethernet0".to_string());
        assert!(t.to_string().contains("NH:"));

        let t = AclRedirectTarget::Port("Ethernet0".to_string());
        assert!(t.to_string().contains("PORT:"));
    }

    #[test]
    fn test_rule_state() {
        let mut rule = AclRule::mirror("rule1");
        assert_eq!(rule.state, AclRuleState::Pending);
        assert!(rule.requires_deferred_activation());

        rule.state = AclRuleState::Active;
        assert!(rule.is_active());
    }

    #[test]
    fn test_in_ports_operations() {
        let mut rule = AclRule::packet("rule1");

        // Add IN_PORTS
        rule.add_match(AclRuleMatch::in_ports(vec![0x1234, 0x5678]));
        let ports = rule.get_in_ports().unwrap();
        assert_eq!(ports.len(), 2);

        // Update IN_PORTS
        rule.update_in_ports(vec![0xABCD]);
        let ports = rule.get_in_ports().unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0], 0xABCD);
    }
}
