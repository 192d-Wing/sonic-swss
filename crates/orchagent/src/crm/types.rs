//! CRM types and data structures.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// CRM resource type enumeration.
///
/// Covers all 57+ resource types tracked by CRM across routing, ACL,
/// forwarding, NAT, MPLS, SRv6, and DASH (DPU) resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrmResourceType {
    // IP routing resources
    Ipv4Route,
    Ipv6Route,

    // Nexthop resources
    Ipv4Nexthop,
    Ipv6Nexthop,
    NexthopGroupMember,
    NexthopGroup,

    // Neighbor resources
    Ipv4Neighbor,
    Ipv6Neighbor,

    // ACL resources
    AclTable,
    AclGroup,
    AclEntry,
    AclCounter,

    // Forwarding resources
    FdbEntry,
    IpmcEntry,

    // NAT resources
    SnatEntry,
    DnatEntry,

    // MPLS resources
    MplsInseg,
    MplsNexthop,

    // SRv6 resources
    Srv6MySidEntry,
    Srv6Nexthop,

    // Extension table (P4RT)
    ExtTable,

    // TWAMP
    TwampEntry,

    // DASH (DPU) resources
    DashVnet,
    DashEni,
    DashEniEther,
    DashIpv4Inbound,
    DashIpv6Inbound,
    DashIpv4Outbound,
    DashIpv6Outbound,
    DashIpv4PaValidation,
    DashIpv6PaValidation,
    DashIpv4OutboundCaToPA,
    DashIpv6OutboundCaToPA,
    DashAclGroup,
    DashAclRule,
}

impl CrmResourceType {
    /// Returns the human-readable name for this resource type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ipv4Route => "ipv4_route",
            Self::Ipv6Route => "ipv6_route",
            Self::Ipv4Nexthop => "ipv4_nexthop",
            Self::Ipv6Nexthop => "ipv6_nexthop",
            Self::NexthopGroupMember => "nexthop_group_member",
            Self::NexthopGroup => "nexthop_group",
            Self::Ipv4Neighbor => "ipv4_neighbor",
            Self::Ipv6Neighbor => "ipv6_neighbor",
            Self::AclTable => "acl_table",
            Self::AclGroup => "acl_group",
            Self::AclEntry => "acl_entry",
            Self::AclCounter => "acl_counter",
            Self::FdbEntry => "fdb_entry",
            Self::IpmcEntry => "ipmc_entry",
            Self::SnatEntry => "snat_entry",
            Self::DnatEntry => "dnat_entry",
            Self::MplsInseg => "mpls_inseg",
            Self::MplsNexthop => "mpls_nexthop",
            Self::Srv6MySidEntry => "srv6_my_sid_entry",
            Self::Srv6Nexthop => "srv6_nexthop",
            Self::ExtTable => "ext_table",
            Self::TwampEntry => "twamp_entry",
            Self::DashVnet => "dash_vnet",
            Self::DashEni => "dash_eni",
            Self::DashEniEther => "dash_eni_ether_address_map",
            Self::DashIpv4Inbound => "dash_ipv4_inbound_routing",
            Self::DashIpv6Inbound => "dash_ipv6_inbound_routing",
            Self::DashIpv4Outbound => "dash_ipv4_outbound_routing",
            Self::DashIpv6Outbound => "dash_ipv6_outbound_routing",
            Self::DashIpv4PaValidation => "dash_ipv4_pa_validation",
            Self::DashIpv6PaValidation => "dash_ipv6_pa_validation",
            Self::DashIpv4OutboundCaToPA => "dash_ipv4_outbound_ca_to_pa",
            Self::DashIpv6OutboundCaToPA => "dash_ipv6_outbound_ca_to_pa",
            Self::DashAclGroup => "dash_acl_group",
            Self::DashAclRule => "dash_acl_rule",
        }
    }

    /// Returns the CONFIG_DB field name for this resource type.
    pub fn config_field(&self) -> &'static str {
        match self {
            Self::Ipv4Route => "ipv4_route",
            Self::Ipv6Route => "ipv6_route",
            Self::Ipv4Nexthop => "ipv4_nexthop",
            Self::Ipv6Nexthop => "ipv6_nexthop",
            Self::NexthopGroupMember => "nexthop_group_member",
            Self::NexthopGroup => "nexthop_group",
            Self::Ipv4Neighbor => "ipv4_neighbor",
            Self::Ipv6Neighbor => "ipv6_neighbor",
            Self::AclTable => "acl_table",
            Self::AclGroup => "acl_group",
            Self::AclEntry => "acl_entry",
            Self::AclCounter => "acl_counter",
            Self::FdbEntry => "fdb_entry",
            Self::IpmcEntry => "ipmc_entry",
            Self::SnatEntry => "snat_entry",
            Self::DnatEntry => "dnat_entry",
            Self::MplsInseg => "mpls_inseg",
            Self::MplsNexthop => "mpls_nexthop",
            Self::Srv6MySidEntry => "srv6_my_sid_entry",
            Self::Srv6Nexthop => "srv6_nexthop",
            Self::ExtTable => "ext_table",
            Self::TwampEntry => "twamp_entry",
            Self::DashVnet => "dash_vnet",
            Self::DashEni => "dash_eni",
            Self::DashEniEther => "dash_eni_ether_address_map",
            Self::DashIpv4Inbound => "dash_ipv4_inbound_routing",
            Self::DashIpv6Inbound => "dash_ipv6_inbound_routing",
            Self::DashIpv4Outbound => "dash_ipv4_outbound_routing",
            Self::DashIpv6Outbound => "dash_ipv6_outbound_routing",
            Self::DashIpv4PaValidation => "dash_ipv4_pa_validation",
            Self::DashIpv6PaValidation => "dash_ipv6_pa_validation",
            Self::DashIpv4OutboundCaToPA => "dash_ipv4_outbound_ca_to_pa",
            Self::DashIpv6OutboundCaToPA => "dash_ipv6_outbound_ca_to_pa",
            Self::DashAclGroup => "dash_acl_group",
            Self::DashAclRule => "dash_acl_rule",
        }
    }

    /// Returns true if this is a DASH (DPU) resource type.
    pub fn is_dash_resource(&self) -> bool {
        matches!(
            self,
            Self::DashVnet
                | Self::DashEni
                | Self::DashEniEther
                | Self::DashIpv4Inbound
                | Self::DashIpv6Inbound
                | Self::DashIpv4Outbound
                | Self::DashIpv6Outbound
                | Self::DashIpv4PaValidation
                | Self::DashIpv6PaValidation
                | Self::DashIpv4OutboundCaToPA
                | Self::DashIpv6OutboundCaToPA
                | Self::DashAclGroup
                | Self::DashAclRule
        )
    }

    /// Returns true if this is an ACL resource type.
    pub fn is_acl_resource(&self) -> bool {
        matches!(
            self,
            Self::AclTable | Self::AclGroup | Self::AclEntry | Self::AclCounter
        )
    }

    /// Returns true if this resource requires per-table tracking.
    pub fn is_per_table_resource(&self) -> bool {
        matches!(self, Self::AclEntry | Self::AclCounter)
    }

    /// Returns all standard (non-DASH) resource types.
    pub fn standard_types() -> &'static [CrmResourceType] {
        &[
            Self::Ipv4Route,
            Self::Ipv6Route,
            Self::Ipv4Nexthop,
            Self::Ipv6Nexthop,
            Self::NexthopGroupMember,
            Self::NexthopGroup,
            Self::Ipv4Neighbor,
            Self::Ipv6Neighbor,
            Self::AclTable,
            Self::AclGroup,
            Self::AclEntry,
            Self::AclCounter,
            Self::FdbEntry,
            Self::IpmcEntry,
            Self::SnatEntry,
            Self::DnatEntry,
            Self::MplsInseg,
            Self::MplsNexthop,
            Self::Srv6MySidEntry,
            Self::Srv6Nexthop,
            Self::ExtTable,
            Self::TwampEntry,
        ]
    }

    /// Returns all DASH resource types.
    pub fn dash_types() -> &'static [CrmResourceType] {
        &[
            Self::DashVnet,
            Self::DashEni,
            Self::DashEniEther,
            Self::DashIpv4Inbound,
            Self::DashIpv6Inbound,
            Self::DashIpv4Outbound,
            Self::DashIpv6Outbound,
            Self::DashIpv4PaValidation,
            Self::DashIpv6PaValidation,
            Self::DashIpv4OutboundCaToPA,
            Self::DashIpv6OutboundCaToPA,
            Self::DashAclGroup,
            Self::DashAclRule,
        ]
    }
}

impl FromStr for CrmResourceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ipv4_route" => Ok(Self::Ipv4Route),
            "ipv6_route" => Ok(Self::Ipv6Route),
            "ipv4_nexthop" => Ok(Self::Ipv4Nexthop),
            "ipv6_nexthop" => Ok(Self::Ipv6Nexthop),
            "nexthop_group_member" => Ok(Self::NexthopGroupMember),
            "nexthop_group" => Ok(Self::NexthopGroup),
            "ipv4_neighbor" => Ok(Self::Ipv4Neighbor),
            "ipv6_neighbor" => Ok(Self::Ipv6Neighbor),
            "acl_table" => Ok(Self::AclTable),
            "acl_group" => Ok(Self::AclGroup),
            "acl_entry" => Ok(Self::AclEntry),
            "acl_counter" => Ok(Self::AclCounter),
            "fdb_entry" => Ok(Self::FdbEntry),
            "ipmc_entry" => Ok(Self::IpmcEntry),
            "snat_entry" => Ok(Self::SnatEntry),
            "dnat_entry" => Ok(Self::DnatEntry),
            "mpls_inseg" => Ok(Self::MplsInseg),
            "mpls_nexthop" => Ok(Self::MplsNexthop),
            "srv6_my_sid_entry" => Ok(Self::Srv6MySidEntry),
            "srv6_nexthop" => Ok(Self::Srv6Nexthop),
            "ext_table" => Ok(Self::ExtTable),
            "twamp_entry" => Ok(Self::TwampEntry),
            "dash_vnet" => Ok(Self::DashVnet),
            "dash_eni" => Ok(Self::DashEni),
            "dash_eni_ether_address_map" => Ok(Self::DashEniEther),
            "dash_ipv4_inbound_routing" => Ok(Self::DashIpv4Inbound),
            "dash_ipv6_inbound_routing" => Ok(Self::DashIpv6Inbound),
            "dash_ipv4_outbound_routing" => Ok(Self::DashIpv4Outbound),
            "dash_ipv6_outbound_routing" => Ok(Self::DashIpv6Outbound),
            "dash_ipv4_pa_validation" => Ok(Self::DashIpv4PaValidation),
            "dash_ipv6_pa_validation" => Ok(Self::DashIpv6PaValidation),
            "dash_ipv4_outbound_ca_to_pa" => Ok(Self::DashIpv4OutboundCaToPA),
            "dash_ipv6_outbound_ca_to_pa" => Ok(Self::DashIpv6OutboundCaToPA),
            "dash_acl_group" => Ok(Self::DashAclGroup),
            "dash_acl_rule" => Ok(Self::DashAclRule),
            _ => Err(format!("Unknown CRM resource type: {}", s)),
        }
    }
}

impl fmt::Display for CrmResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// CRM threshold type for resource monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CrmThresholdType {
    /// Percentage of total capacity.
    #[default]
    Percentage,
    /// Absolute number of entries used.
    Used,
    /// Absolute number of entries available.
    Free,
}

impl CrmThresholdType {
    /// Returns the CONFIG_DB field value for this threshold type.
    pub fn config_value(&self) -> &'static str {
        match self {
            Self::Percentage => "percentage",
            Self::Used => "used",
            Self::Free => "free",
        }
    }
}

impl FromStr for CrmThresholdType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "percentage" => Ok(Self::Percentage),
            "used" => Ok(Self::Used),
            "free" => Ok(Self::Free),
            _ => Err(format!("Unknown threshold type: {}", s)),
        }
    }
}

impl fmt::Display for CrmThresholdType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.config_value())
    }
}

/// CRM resource support status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CrmResourceStatus {
    /// Resource is supported by the platform.
    #[default]
    Supported,
    /// Resource is not supported by the platform.
    NotSupported,
}

/// Counter data for a single CRM resource context.
#[derive(Debug, Clone, Default)]
pub struct CrmResourceCounter {
    /// SAI object ID (used for ACL tables, DASH ACL groups).
    pub id: u64,
    /// Available entries.
    pub available: u32,
    /// Used entries.
    pub used: u32,
    /// Exceeded log counter (for rate limiting, max 10).
    pub exceeded_log_count: u32,
}

impl CrmResourceCounter {
    /// Creates a new counter with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a counter with a specific SAI object ID.
    pub fn with_id(id: u64) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    /// Returns the utilization percentage (0-100).
    pub fn utilization_percent(&self) -> u32 {
        let total = self.used + self.available;
        if total == 0 {
            0
        } else {
            (self.used * 100) / total
        }
    }

    /// Increments the used counter, returning the new value.
    pub fn increment_used(&mut self) -> u32 {
        self.used = self.used.saturating_add(1);
        self.used
    }

    /// Decrements the used counter, returning the new value.
    /// Returns None if the counter would underflow.
    pub fn decrement_used(&mut self) -> Option<u32> {
        if self.used > 0 {
            self.used -= 1;
            Some(self.used)
        } else {
            None
        }
    }

    /// Checks if threshold is exceeded and updates log counter.
    /// Returns true if an event should be published.
    pub fn check_threshold(&mut self, threshold_type: CrmThresholdType, high: u32, low: u32) -> ThresholdCheck {
        let utilization = match threshold_type {
            CrmThresholdType::Percentage => self.utilization_percent(),
            CrmThresholdType::Used => self.used,
            CrmThresholdType::Free => self.available,
        };

        if utilization >= high && self.exceeded_log_count < CRM_EXCEEDED_MSG_MAX {
            self.exceeded_log_count += 1;
            ThresholdCheck::Exceeded {
                utilization,
                threshold: high,
            }
        } else if utilization <= low && self.exceeded_log_count > 0 {
            self.exceeded_log_count = 0;
            ThresholdCheck::Recovered {
                utilization,
                threshold: low,
            }
        } else {
            ThresholdCheck::Normal
        }
    }
}

/// Maximum number of exceeded messages before rate limiting.
pub const CRM_EXCEEDED_MSG_MAX: u32 = 10;

/// Result of threshold check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdCheck {
    /// Utilization is normal.
    Normal,
    /// High threshold exceeded.
    Exceeded { utilization: u32, threshold: u32 },
    /// Recovered below low threshold.
    Recovered { utilization: u32, threshold: u32 },
}

/// Default low threshold percentage.
pub const DEFAULT_LOW_THRESHOLD: u32 = 70;

/// Default high threshold percentage.
pub const DEFAULT_HIGH_THRESHOLD: u32 = 85;

/// Default polling interval in seconds (5 minutes).
pub const DEFAULT_POLLING_INTERVAL: u64 = 5 * 60;

/// Counter table key for global/default resources.
pub const CRM_COUNTERS_TABLE_KEY: &str = "STATS";

/// CRM resource entry tracking thresholds and counters.
#[derive(Debug, Clone)]
pub struct CrmResourceEntry {
    /// Resource type.
    pub resource_type: CrmResourceType,
    /// Threshold type.
    pub threshold_type: CrmThresholdType,
    /// Low threshold value.
    pub low_threshold: u32,
    /// High threshold value.
    pub high_threshold: u32,
    /// Counters map, keyed by context (e.g., "STATS", "ACL_STATS:INGRESS:PORT").
    pub counters: HashMap<String, CrmResourceCounter>,
    /// Resource support status.
    pub status: CrmResourceStatus,
}

impl CrmResourceEntry {
    /// Creates a new resource entry with default thresholds.
    pub fn new(resource_type: CrmResourceType) -> Self {
        let mut counters = HashMap::new();
        // Initialize default counter for global resources
        if !resource_type.is_acl_resource() && !resource_type.is_dash_resource() {
            counters.insert(CRM_COUNTERS_TABLE_KEY.to_string(), CrmResourceCounter::new());
        }
        Self {
            resource_type,
            threshold_type: CrmThresholdType::default(),
            low_threshold: DEFAULT_LOW_THRESHOLD,
            high_threshold: DEFAULT_HIGH_THRESHOLD,
            counters,
            status: CrmResourceStatus::default(),
        }
    }

    /// Gets or creates a counter for the given key.
    pub fn get_or_create_counter(&mut self, key: &str) -> &mut CrmResourceCounter {
        self.counters
            .entry(key.to_string())
            .or_insert_with(CrmResourceCounter::new)
    }

    /// Gets a counter by key if it exists.
    pub fn get_counter(&self, key: &str) -> Option<&CrmResourceCounter> {
        self.counters.get(key)
    }

    /// Gets a mutable counter by key if it exists.
    pub fn get_counter_mut(&mut self, key: &str) -> Option<&mut CrmResourceCounter> {
        self.counters.get_mut(key)
    }

    /// Removes a counter by key.
    pub fn remove_counter(&mut self, key: &str) -> Option<CrmResourceCounter> {
        self.counters.remove(key)
    }

    /// Returns all counter keys.
    pub fn counter_keys(&self) -> impl Iterator<Item = &String> {
        self.counters.keys()
    }
}

/// ACL stage for CRM tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AclStage {
    Ingress,
    Egress,
}

impl AclStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ingress => "INGRESS",
            Self::Egress => "EGRESS",
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

impl fmt::Display for AclStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// ACL bind point type for CRM tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AclBindPoint {
    Port,
    Lag,
    Vlan,
    Rif,
    Switch,
}

impl AclBindPoint {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Port => "PORT",
            Self::Lag => "LAG",
            Self::Vlan => "VLAN",
            Self::Rif => "RIF",
            Self::Switch => "SWITCH",
        }
    }
}

impl FromStr for AclBindPoint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PORT" => Ok(Self::Port),
            "LAG" => Ok(Self::Lag),
            "VLAN" => Ok(Self::Vlan),
            "RIF" => Ok(Self::Rif),
            "SWITCH" => Ok(Self::Switch),
            _ => Err(format!("Unknown ACL bind point: {}", s)),
        }
    }
}

impl fmt::Display for AclBindPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Generates the CRM counter key for ACL resources.
pub fn crm_acl_key(stage: AclStage, bind_point: AclBindPoint) -> String {
    format!("ACL_STATS:{}:{}", stage.as_str(), bind_point.as_str())
}

/// Generates the CRM counter key for per-table ACL resources.
pub fn crm_acl_table_key(table_id: u64) -> String {
    format!("ACL_TABLE_STATS:0x{:x}", table_id)
}

/// Generates the CRM counter key for extension tables.
pub fn crm_ext_table_key(table_name: &str) -> String {
    format!("EXT_TABLE_STATS:{}", table_name)
}

/// Generates the CRM counter key for DASH ACL groups.
pub fn crm_dash_acl_group_key(group_id: u64) -> String {
    format!("DASH_ACL_GROUP_STATS:0x{:x}", group_id)
}

/// CRM threshold field types for configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrmThresholdField {
    Type,
    Low,
    High,
}

impl CrmThresholdField {
    /// Returns the CONFIG_DB field suffix.
    pub fn suffix(&self) -> &'static str {
        match self {
            Self::Type => "_threshold_type",
            Self::Low => "_low_threshold",
            Self::High => "_high_threshold",
        }
    }

    /// Parses a CONFIG_DB field name to extract resource and field type.
    pub fn parse_field(field: &str) -> Option<(String, CrmThresholdField)> {
        if let Some(resource) = field.strip_suffix("_threshold_type") {
            Some((resource.to_string(), Self::Type))
        } else if let Some(resource) = field.strip_suffix("_low_threshold") {
            Some((resource.to_string(), Self::Low))
        } else if let Some(resource) = field.strip_suffix("_high_threshold") {
            Some((resource.to_string(), Self::High))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_type_parse() {
        assert_eq!(
            "ipv4_route".parse::<CrmResourceType>().unwrap(),
            CrmResourceType::Ipv4Route
        );
        assert_eq!(
            "dash_vnet".parse::<CrmResourceType>().unwrap(),
            CrmResourceType::DashVnet
        );
        assert!("invalid".parse::<CrmResourceType>().is_err());
    }

    #[test]
    fn test_resource_type_is_dash() {
        assert!(!CrmResourceType::Ipv4Route.is_dash_resource());
        assert!(CrmResourceType::DashVnet.is_dash_resource());
        assert!(CrmResourceType::DashAclRule.is_dash_resource());
    }

    #[test]
    fn test_resource_type_is_acl() {
        assert!(CrmResourceType::AclTable.is_acl_resource());
        assert!(CrmResourceType::AclEntry.is_acl_resource());
        assert!(!CrmResourceType::Ipv4Route.is_acl_resource());
    }

    #[test]
    fn test_threshold_type_parse() {
        assert_eq!(
            "percentage".parse::<CrmThresholdType>().unwrap(),
            CrmThresholdType::Percentage
        );
        assert_eq!(
            "used".parse::<CrmThresholdType>().unwrap(),
            CrmThresholdType::Used
        );
        assert_eq!(
            "free".parse::<CrmThresholdType>().unwrap(),
            CrmThresholdType::Free
        );
    }

    #[test]
    fn test_resource_counter() {
        let mut counter = CrmResourceCounter::new();
        assert_eq!(counter.used, 0);
        assert_eq!(counter.available, 0);

        counter.available = 100;
        counter.increment_used();
        counter.increment_used();
        assert_eq!(counter.used, 2);
        // utilization = used * 100 / (used + available) = 2 * 100 / 102 = 1 (integer division)
        assert_eq!(counter.utilization_percent(), 1);

        counter.decrement_used();
        assert_eq!(counter.used, 1);
        // utilization = 1 * 100 / 101 = 0 (integer division)
        assert_eq!(counter.utilization_percent(), 0);

        // Test higher utilization
        counter.used = 50;
        counter.available = 50;
        // utilization = 50 * 100 / 100 = 50
        assert_eq!(counter.utilization_percent(), 50);

        // Test underflow protection
        counter.used = 0;
        assert!(counter.decrement_used().is_none());
    }

    #[test]
    fn test_threshold_check() {
        let mut counter = CrmResourceCounter::new();
        counter.available = 15;
        counter.used = 85;

        // First check should report exceeded
        let check = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(check, ThresholdCheck::Exceeded { .. }));
        assert_eq!(counter.exceeded_log_count, 1);

        // Lower utilization below low threshold
        counter.used = 50;
        counter.available = 50;
        let check = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
        assert!(matches!(check, ThresholdCheck::Recovered { .. }));
        assert_eq!(counter.exceeded_log_count, 0);
    }

    #[test]
    fn test_threshold_rate_limiting() {
        let mut counter = CrmResourceCounter::new();
        counter.available = 0;
        counter.used = 100;

        // Should stop reporting after CRM_EXCEEDED_MSG_MAX
        for i in 0..CRM_EXCEEDED_MSG_MAX + 5 {
            let check = counter.check_threshold(CrmThresholdType::Percentage, 85, 70);
            if i < CRM_EXCEEDED_MSG_MAX {
                assert!(matches!(check, ThresholdCheck::Exceeded { .. }));
            } else {
                assert!(matches!(check, ThresholdCheck::Normal));
            }
        }
        assert_eq!(counter.exceeded_log_count, CRM_EXCEEDED_MSG_MAX);
    }

    #[test]
    fn test_resource_entry() {
        let mut entry = CrmResourceEntry::new(CrmResourceType::Ipv4Route);
        assert_eq!(entry.low_threshold, DEFAULT_LOW_THRESHOLD);
        assert_eq!(entry.high_threshold, DEFAULT_HIGH_THRESHOLD);

        // Global resources have default counter
        assert!(entry.counters.contains_key(CRM_COUNTERS_TABLE_KEY));

        // Get or create counter
        let counter = entry.get_or_create_counter("test_key");
        counter.used = 10;
        assert_eq!(entry.get_counter("test_key").unwrap().used, 10);
    }

    #[test]
    fn test_acl_key_generation() {
        assert_eq!(
            crm_acl_key(AclStage::Ingress, AclBindPoint::Port),
            "ACL_STATS:INGRESS:PORT"
        );
        assert_eq!(
            crm_acl_key(AclStage::Egress, AclBindPoint::Vlan),
            "ACL_STATS:EGRESS:VLAN"
        );
    }

    #[test]
    fn test_table_key_generation() {
        assert_eq!(crm_acl_table_key(0x1234), "ACL_TABLE_STATS:0x1234");
        assert_eq!(crm_ext_table_key("my_table"), "EXT_TABLE_STATS:my_table");
        assert_eq!(
            crm_dash_acl_group_key(0xabcd),
            "DASH_ACL_GROUP_STATS:0xabcd"
        );
    }

    #[test]
    fn test_threshold_field_parse() {
        let (resource, field) = CrmThresholdField::parse_field("ipv4_route_threshold_type").unwrap();
        assert_eq!(resource, "ipv4_route");
        assert_eq!(field, CrmThresholdField::Type);

        let (resource, field) = CrmThresholdField::parse_field("acl_entry_low_threshold").unwrap();
        assert_eq!(resource, "acl_entry");
        assert_eq!(field, CrmThresholdField::Low);

        assert!(CrmThresholdField::parse_field("invalid_field").is_none());
    }
}
