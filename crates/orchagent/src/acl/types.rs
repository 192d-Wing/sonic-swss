//! ACL types and enums.
//!
//! This module defines the core types used throughout the ACL subsystem.
//! These replace magic numbers and strings in the C++ implementation with
//! type-safe Rust enums.

use std::fmt;
use std::str::FromStr;

/// ACL stage (ingress or egress).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AclStage {
    /// Ingress ACL (applied to incoming packets).
    #[default]
    Ingress,
    /// Egress ACL (applied to outgoing packets).
    Egress,
}

impl fmt::Display for AclStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ingress => write!(f, "INGRESS"),
            Self::Egress => write!(f, "EGRESS"),
        }
    }
}

impl FromStr for AclStage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "INGRESS" => Ok(Self::Ingress),
            "EGRESS" => Ok(Self::Egress),
            _ => Err(format!("Unknown ACL stage: {}", s)),
        }
    }
}

/// ACL bind point type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AclBindPointType {
    /// Bind to physical port.
    Port,
    /// Bind to LAG.
    Lag,
    /// Bind to VLAN.
    Vlan,
    /// Bind to router interface.
    RouterInterface,
    /// Bind to switch (global).
    Switch,
}

impl fmt::Display for AclBindPointType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Port => write!(f, "PORT"),
            Self::Lag => write!(f, "LAG"),
            Self::Vlan => write!(f, "VLAN"),
            Self::RouterInterface => write!(f, "ROUTER_INTERFACE"),
            Self::Switch => write!(f, "SWITCH"),
        }
    }
}

impl FromStr for AclBindPointType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PORT" => Ok(Self::Port),
            "LAG" => Ok(Self::Lag),
            "VLAN" => Ok(Self::Vlan),
            "ROUTER_INTERFACE" | "RIF" => Ok(Self::RouterInterface),
            "SWITCH" => Ok(Self::Switch),
            _ => Err(format!("Unknown ACL bind point type: {}", s)),
        }
    }
}

/// ACL match field types.
///
/// These correspond to SAI ACL table match field attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AclMatchField {
    // IPv4 fields
    SrcIp,
    DstIp,
    EtherType,
    IpProtocol,
    Dscp,
    Ecn,
    TcpFlags,
    IcmpType,
    IcmpCode,

    // IPv6 fields
    SrcIpv6,
    DstIpv6,
    Ipv6NextHeader,
    Icmpv6Type,
    Icmpv6Code,

    // L4 fields
    L4SrcPort,
    L4DstPort,
    L4SrcPortRange,
    L4DstPortRange,

    // L2 fields
    SrcMac,
    DstMac,
    VlanId,
    OuterVlanId,

    // Port fields
    InPorts,
    OutPorts,
    OutPort,

    // Tunnel fields
    TunnelVni,
    InnerEtherType,
    InnerIpProtocol,
    InnerSrcIp,
    InnerDstIp,
    InnerSrcIpv6,
    InnerDstIpv6,
    InnerL4SrcPort,
    InnerL4DstPort,
    InnerSrcMac,
    InnerDstMac,

    // Traffic class
    Tc,

    // RDMA fields
    BthOpcode,
    AethSyndrome,

    // Metadata
    AclMetaData,
}

impl fmt::Display for AclMatchField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SrcIp => write!(f, "SRC_IP"),
            Self::DstIp => write!(f, "DST_IP"),
            Self::EtherType => write!(f, "ETHER_TYPE"),
            Self::IpProtocol => write!(f, "IP_PROTOCOL"),
            Self::Dscp => write!(f, "DSCP"),
            Self::Ecn => write!(f, "ECN"),
            Self::TcpFlags => write!(f, "TCP_FLAGS"),
            Self::IcmpType => write!(f, "ICMP_TYPE"),
            Self::IcmpCode => write!(f, "ICMP_CODE"),
            Self::SrcIpv6 => write!(f, "SRC_IPV6"),
            Self::DstIpv6 => write!(f, "DST_IPV6"),
            Self::Ipv6NextHeader => write!(f, "IPV6_NEXT_HEADER"),
            Self::Icmpv6Type => write!(f, "ICMPV6_TYPE"),
            Self::Icmpv6Code => write!(f, "ICMPV6_CODE"),
            Self::L4SrcPort => write!(f, "L4_SRC_PORT"),
            Self::L4DstPort => write!(f, "L4_DST_PORT"),
            Self::L4SrcPortRange => write!(f, "L4_SRC_PORT_RANGE"),
            Self::L4DstPortRange => write!(f, "L4_DST_PORT_RANGE"),
            Self::SrcMac => write!(f, "SRC_MAC"),
            Self::DstMac => write!(f, "DST_MAC"),
            Self::VlanId => write!(f, "VLAN_ID"),
            Self::OuterVlanId => write!(f, "OUTER_VLAN_ID"),
            Self::InPorts => write!(f, "IN_PORTS"),
            Self::OutPorts => write!(f, "OUT_PORTS"),
            Self::OutPort => write!(f, "OUT_PORT"),
            Self::TunnelVni => write!(f, "TUNNEL_VNI"),
            Self::InnerEtherType => write!(f, "INNER_ETHER_TYPE"),
            Self::InnerIpProtocol => write!(f, "INNER_IP_PROTOCOL"),
            Self::InnerSrcIp => write!(f, "INNER_SRC_IP"),
            Self::InnerDstIp => write!(f, "INNER_DST_IP"),
            Self::InnerSrcIpv6 => write!(f, "INNER_SRC_IPV6"),
            Self::InnerDstIpv6 => write!(f, "INNER_DST_IPV6"),
            Self::InnerL4SrcPort => write!(f, "INNER_L4_SRC_PORT"),
            Self::InnerL4DstPort => write!(f, "INNER_L4_DST_PORT"),
            Self::InnerSrcMac => write!(f, "INNER_SRC_MAC"),
            Self::InnerDstMac => write!(f, "INNER_DST_MAC"),
            Self::Tc => write!(f, "TC"),
            Self::BthOpcode => write!(f, "BTH_OPCODE"),
            Self::AethSyndrome => write!(f, "AETH_SYNDROME"),
            Self::AclMetaData => write!(f, "ACL_META_DATA"),
        }
    }
}

impl FromStr for AclMatchField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "SRC_IP" => Ok(Self::SrcIp),
            "DST_IP" => Ok(Self::DstIp),
            "ETHER_TYPE" => Ok(Self::EtherType),
            "IP_PROTOCOL" => Ok(Self::IpProtocol),
            "DSCP" => Ok(Self::Dscp),
            "ECN" => Ok(Self::Ecn),
            "TCP_FLAGS" => Ok(Self::TcpFlags),
            "ICMP_TYPE" => Ok(Self::IcmpType),
            "ICMP_CODE" => Ok(Self::IcmpCode),
            "SRC_IPV6" => Ok(Self::SrcIpv6),
            "DST_IPV6" => Ok(Self::DstIpv6),
            "IPV6_NEXT_HEADER" | "NEXT_HEADER" => Ok(Self::Ipv6NextHeader),
            "ICMPV6_TYPE" => Ok(Self::Icmpv6Type),
            "ICMPV6_CODE" => Ok(Self::Icmpv6Code),
            "L4_SRC_PORT" => Ok(Self::L4SrcPort),
            "L4_DST_PORT" => Ok(Self::L4DstPort),
            "L4_SRC_PORT_RANGE" => Ok(Self::L4SrcPortRange),
            "L4_DST_PORT_RANGE" => Ok(Self::L4DstPortRange),
            "SRC_MAC" => Ok(Self::SrcMac),
            "DST_MAC" => Ok(Self::DstMac),
            "VLAN_ID" => Ok(Self::VlanId),
            "OUTER_VLAN_ID" => Ok(Self::OuterVlanId),
            "IN_PORTS" => Ok(Self::InPorts),
            "OUT_PORTS" => Ok(Self::OutPorts),
            "OUT_PORT" => Ok(Self::OutPort),
            "TUNNEL_VNI" => Ok(Self::TunnelVni),
            "INNER_ETHER_TYPE" => Ok(Self::InnerEtherType),
            "INNER_IP_PROTOCOL" => Ok(Self::InnerIpProtocol),
            "INNER_SRC_IP" => Ok(Self::InnerSrcIp),
            "INNER_DST_IP" => Ok(Self::InnerDstIp),
            "INNER_SRC_IPV6" => Ok(Self::InnerSrcIpv6),
            "INNER_DST_IPV6" => Ok(Self::InnerDstIpv6),
            "INNER_L4_SRC_PORT" => Ok(Self::InnerL4SrcPort),
            "INNER_L4_DST_PORT" => Ok(Self::InnerL4DstPort),
            "INNER_SRC_MAC" => Ok(Self::InnerSrcMac),
            "INNER_DST_MAC" => Ok(Self::InnerDstMac),
            "TC" => Ok(Self::Tc),
            "BTH_OPCODE" => Ok(Self::BthOpcode),
            "AETH_SYNDROME" => Ok(Self::AethSyndrome),
            "ACL_META_DATA" | "META_DATA" => Ok(Self::AclMetaData),
            _ => Err(format!("Unknown ACL match field: {}", s)),
        }
    }
}

/// ACL action types.
///
/// These correspond to SAI ACL entry action attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AclActionType {
    /// Packet action (forward, drop, copy).
    PacketAction,
    /// Redirect to next-hop, port, LAG, or tunnel.
    Redirect,
    /// Mirror ingress.
    MirrorIngress,
    /// Mirror egress.
    MirrorEgress,
    /// Set DSCP value.
    SetDscp,
    /// Set traffic class.
    SetTc,
    /// Set packet color.
    SetPacketColor,
    /// Set ACL metadata.
    SetAclMetaData,
    /// Counter.
    Counter,
    /// Do not NAT.
    DoNotNat,
    /// Disable packet trimming.
    DisableTrim,
    /// Set inner source MAC (for VXLAN).
    SetInnerSrcMac,
    /// DTEL flow operation.
    DtelFlowOp,
    /// DTEL INT session.
    DtelIntSession,
    /// DTEL drop report enable.
    DtelDropReportEnable,
    /// DTEL tail drop report enable.
    DtelTailDropReportEnable,
    /// DTEL flow sample percent.
    DtelFlowSamplePercent,
    /// DTEL report all packets.
    DtelReportAllPackets,
}

impl fmt::Display for AclActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketAction => write!(f, "PACKET_ACTION"),
            Self::Redirect => write!(f, "REDIRECT_ACTION"),
            Self::MirrorIngress => write!(f, "MIRROR_INGRESS_ACTION"),
            Self::MirrorEgress => write!(f, "MIRROR_EGRESS_ACTION"),
            Self::SetDscp => write!(f, "SET_DSCP"),
            Self::SetTc => write!(f, "SET_TC"),
            Self::SetPacketColor => write!(f, "SET_PACKET_COLOR"),
            Self::SetAclMetaData => write!(f, "SET_ACL_META_DATA"),
            Self::Counter => write!(f, "COUNTER"),
            Self::DoNotNat => write!(f, "DO_NOT_NAT_ACTION"),
            Self::DisableTrim => write!(f, "DISABLE_TRIM_ACTION"),
            Self::SetInnerSrcMac => write!(f, "SET_INNER_SRC_MAC"),
            Self::DtelFlowOp => write!(f, "FLOW_OP"),
            Self::DtelIntSession => write!(f, "INT_SESSION"),
            Self::DtelDropReportEnable => write!(f, "DROP_REPORT_ENABLE"),
            Self::DtelTailDropReportEnable => write!(f, "TAIL_DROP_REPORT_ENABLE"),
            Self::DtelFlowSamplePercent => write!(f, "FLOW_SAMPLE_PERCENT"),
            Self::DtelReportAllPackets => write!(f, "REPORT_ALL_PACKETS"),
        }
    }
}

impl FromStr for AclActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PACKET_ACTION" => Ok(Self::PacketAction),
            "REDIRECT_ACTION" => Ok(Self::Redirect),
            "MIRROR_INGRESS_ACTION" => Ok(Self::MirrorIngress),
            "MIRROR_EGRESS_ACTION" => Ok(Self::MirrorEgress),
            "SET_DSCP" => Ok(Self::SetDscp),
            "SET_TC" => Ok(Self::SetTc),
            "SET_PACKET_COLOR" => Ok(Self::SetPacketColor),
            "SET_ACL_META_DATA" => Ok(Self::SetAclMetaData),
            "COUNTER" => Ok(Self::Counter),
            "DO_NOT_NAT_ACTION" => Ok(Self::DoNotNat),
            "DISABLE_TRIM_ACTION" => Ok(Self::DisableTrim),
            "SET_INNER_SRC_MAC" => Ok(Self::SetInnerSrcMac),
            "FLOW_OP" => Ok(Self::DtelFlowOp),
            "INT_SESSION" => Ok(Self::DtelIntSession),
            "DROP_REPORT_ENABLE" => Ok(Self::DtelDropReportEnable),
            "TAIL_DROP_REPORT_ENABLE" => Ok(Self::DtelTailDropReportEnable),
            "FLOW_SAMPLE_PERCENT" => Ok(Self::DtelFlowSamplePercent),
            "REPORT_ALL_PACKETS" => Ok(Self::DtelReportAllPackets),
            _ => Err(format!("Unknown ACL action type: {}", s)),
        }
    }
}

/// ACL packet action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AclPacketAction {
    /// Forward the packet.
    #[default]
    Forward,
    /// Drop the packet.
    Drop,
    /// Copy the packet to CPU.
    Copy,
    /// Copy and cancel (trap).
    CopyCancel,
    /// Trap (copy to CPU and drop).
    Trap,
    /// Log (copy to CPU and forward).
    Log,
    /// Deny (drop without logging).
    Deny,
    /// Transit (forward to next pipeline).
    Transit,
}

impl fmt::Display for AclPacketAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forward => write!(f, "FORWARD"),
            Self::Drop => write!(f, "DROP"),
            Self::Copy => write!(f, "COPY"),
            Self::CopyCancel => write!(f, "COPY_CANCEL"),
            Self::Trap => write!(f, "TRAP"),
            Self::Log => write!(f, "LOG"),
            Self::Deny => write!(f, "DENY"),
            Self::Transit => write!(f, "TRANSIT"),
        }
    }
}

impl FromStr for AclPacketAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "FORWARD" => Ok(Self::Forward),
            "DROP" => Ok(Self::Drop),
            "COPY" => Ok(Self::Copy),
            "COPY_CANCEL" => Ok(Self::CopyCancel),
            "TRAP" => Ok(Self::Trap),
            "LOG" => Ok(Self::Log),
            "DENY" => Ok(Self::Deny),
            "TRANSIT" => Ok(Self::Transit),
            _ => Err(format!("Unknown packet action: {}", s)),
        }
    }
}

/// ACL table identifier (string).
pub type AclTableId = String;

/// ACL rule identifier (string).
pub type AclRuleId = String;

/// ACL priority (u32, higher = more specific).
pub type AclPriority = u32;

/// Metadata value (12-bit, 0-4095).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MetaDataValue(u16);

impl MetaDataValue {
    /// Minimum valid metadata value.
    pub const MIN: u16 = 0;
    /// Maximum valid metadata value (12-bit).
    pub const MAX: u16 = 4095;

    /// Creates a new metadata value if within valid range.
    pub fn new(value: u16) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the raw value.
    pub fn value(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for MetaDataValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u16> for MetaDataValue {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
            .ok_or_else(|| format!("Metadata value {} out of range (0-{})", value, Self::MAX))
    }
}

/// Pre-defined ACL table type names.
pub mod table_type_names {
    pub const L3: &str = "L3";
    pub const L3V6: &str = "L3V6";
    pub const L3V4V6: &str = "L3V4V6";
    pub const MIRROR: &str = "MIRROR";
    pub const MIRRORV6: &str = "MIRRORV6";
    pub const MIRROR_DSCP: &str = "MIRROR_DSCP";
    pub const MCLAG: &str = "MCLAG";
    pub const MUX: &str = "MUX";
    pub const DROP: &str = "DROP";
    pub const PFCWD: &str = "PFCWD";
    pub const CTRLPLANE: &str = "CTRLPLANE";
    pub const MARK_META: &str = "MARK_META";
    pub const MARK_METAV6: &str = "MARK_METAV6";
    pub const EGR_SET_DSCP: &str = "EGR_SET_DSCP";
    pub const UNDERLAY_SET_DSCP: &str = "UNDERLAY_SET_DSCP";
    pub const UNDERLAY_SET_DSCPV6: &str = "UNDERLAY_SET_DSCPV6";
    pub const DTEL_FLOW_WATCHLIST: &str = "DTEL_FLOW_WATCHLIST";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_stage_parse() {
        assert_eq!("INGRESS".parse::<AclStage>().unwrap(), AclStage::Ingress);
        assert_eq!("EGRESS".parse::<AclStage>().unwrap(), AclStage::Egress);
        assert!("INVALID".parse::<AclStage>().is_err());
    }

    #[test]
    fn test_acl_stage_display() {
        assert_eq!(AclStage::Ingress.to_string(), "INGRESS");
        assert_eq!(AclStage::Egress.to_string(), "EGRESS");
    }

    #[test]
    fn test_acl_bind_point_parse() {
        assert_eq!(
            "PORT".parse::<AclBindPointType>().unwrap(),
            AclBindPointType::Port
        );
        assert_eq!(
            "LAG".parse::<AclBindPointType>().unwrap(),
            AclBindPointType::Lag
        );
        assert_eq!(
            "SWITCH".parse::<AclBindPointType>().unwrap(),
            AclBindPointType::Switch
        );
    }

    #[test]
    fn test_acl_match_field_parse() {
        assert_eq!(
            "SRC_IP".parse::<AclMatchField>().unwrap(),
            AclMatchField::SrcIp
        );
        assert_eq!(
            "DST_IPV6".parse::<AclMatchField>().unwrap(),
            AclMatchField::DstIpv6
        );
        assert_eq!(
            "TCP_FLAGS".parse::<AclMatchField>().unwrap(),
            AclMatchField::TcpFlags
        );
        assert_eq!(
            "IN_PORTS".parse::<AclMatchField>().unwrap(),
            AclMatchField::InPorts
        );
    }

    #[test]
    fn test_acl_action_type_parse() {
        assert_eq!(
            "PACKET_ACTION".parse::<AclActionType>().unwrap(),
            AclActionType::PacketAction
        );
        assert_eq!(
            "REDIRECT_ACTION".parse::<AclActionType>().unwrap(),
            AclActionType::Redirect
        );
        assert_eq!(
            "SET_DSCP".parse::<AclActionType>().unwrap(),
            AclActionType::SetDscp
        );
    }

    #[test]
    fn test_acl_packet_action_parse() {
        assert_eq!(
            "FORWARD".parse::<AclPacketAction>().unwrap(),
            AclPacketAction::Forward
        );
        assert_eq!(
            "DROP".parse::<AclPacketAction>().unwrap(),
            AclPacketAction::Drop
        );
        assert_eq!(
            "TRAP".parse::<AclPacketAction>().unwrap(),
            AclPacketAction::Trap
        );
    }

    #[test]
    fn test_metadata_value() {
        assert!(MetaDataValue::new(0).is_some());
        assert!(MetaDataValue::new(4095).is_some());
        assert!(MetaDataValue::new(4096).is_none());

        let meta = MetaDataValue::new(100).unwrap();
        assert_eq!(meta.value(), 100);
    }

    #[test]
    fn test_metadata_value_try_from() {
        assert!(MetaDataValue::try_from(100u16).is_ok());
        assert!(MetaDataValue::try_from(5000u16).is_err());
    }
}
