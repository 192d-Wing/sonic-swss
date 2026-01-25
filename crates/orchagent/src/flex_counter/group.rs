//! FlexCounter group definitions and mapping.
//!
//! Defines the 24 counter group types supported by FlexCounterOrch.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// FlexCounter group identifiers.
///
/// Each variant corresponds to a specific type of counter that can be
/// enabled/disabled and polled at configurable intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlexCounterGroup {
    // Port and Interface Counters
    Port,
    PortRates,
    PortBufferDrop,

    // Queue Counters
    Queue,
    QueueWatermark,
    WredEcnQueue,

    // Priority Group (PG) Counters
    PgWatermark,
    PgDrop,

    // Buffer Management
    BufferPoolWatermark,

    // Routing and Interface Counters
    Rif,
    RifRates,

    // ACL and Debug Counters
    DebugCounter,
    DebugMonitorCounter,
    Acl,

    // Tunneling and DASH
    Tunnel,
    Eni,
    DashMeter,

    // Flow Counters
    FlowCntTrap,
    FlowCntRoute,

    // MACSec
    MacsecSa,
    MacsecSaAttr,
    MacsecFlow,

    // PFC Watchdog
    Pfcwd,

    // Port WRED
    WredEcnPort,

    // SRV6 and Switch
    Srv6,
    Switch,
}

impl FlexCounterGroup {
    /// Returns the SAI flex counter group name for this group.
    pub fn sai_group_name(&self) -> &'static str {
        match self {
            Self::Port => "PORT_STAT_COUNTER",
            Self::PortRates => "PORT_RATE_COUNTER",
            Self::PortBufferDrop => "PORT_BUFFER_DROP_STAT",
            Self::Queue => "QUEUE_STAT_COUNTER",
            Self::QueueWatermark => "QUEUE_WATERMARK_STAT_COUNTER",
            Self::WredEcnQueue => "WRED_ECN_QUEUE_STAT_COUNTER",
            Self::PgWatermark => "PG_WATERMARK_STAT_COUNTER",
            Self::PgDrop => "PG_DROP_STAT_COUNTER",
            Self::BufferPoolWatermark => "BUFFER_POOL_WATERMARK_STAT_COUNTER",
            Self::Rif => "RIF_STAT_COUNTER",
            Self::RifRates => "RIF_RATE_COUNTER",
            Self::DebugCounter => "DEBUG_COUNTER",
            Self::DebugMonitorCounter => "DEBUG_DROP_MONITOR",
            Self::Acl => "ACL_STAT_COUNTER",
            Self::Tunnel => "TUNNEL_STAT_COUNTER",
            Self::Eni => "ENI_STAT_COUNTER",
            Self::DashMeter => "METER_STAT_COUNTER",
            Self::FlowCntTrap => "HOSTIF_TRAP_FLOW_COUNTER",
            Self::FlowCntRoute => "ROUTE_FLOW_COUNTER",
            Self::MacsecSa => "MACSEC_SA_COUNTER",
            Self::MacsecSaAttr => "MACSEC_SA_ATTR",
            Self::MacsecFlow => "MACSEC_FLOW_COUNTER",
            Self::Pfcwd => "PFC_WD",
            Self::WredEcnPort => "WRED_ECN_PORT_STAT_COUNTER",
            Self::Srv6 => "SRV6_STAT_COUNTER",
            Self::Switch => "SWITCH_STAT_COUNTER",
        }
    }

    /// Returns the Redis key name for this group.
    pub fn redis_key(&self) -> &'static str {
        match self {
            Self::Port => "PORT",
            Self::PortRates => "PORT_RATES",
            Self::PortBufferDrop => "PORT_BUFFER_DROP",
            Self::Queue => "QUEUE",
            Self::QueueWatermark => "QUEUE_WATERMARK",
            Self::WredEcnQueue => "WRED_ECN_QUEUE",
            Self::PgWatermark => "PG_WATERMARK",
            Self::PgDrop => "PG_DROP",
            Self::BufferPoolWatermark => "BUFFER_POOL_WATERMARK",
            Self::Rif => "RIF",
            Self::RifRates => "RIF_RATES",
            Self::DebugCounter => "DEBUG_COUNTER",
            Self::DebugMonitorCounter => "DEBUG_MONITOR_COUNTER",
            Self::Acl => "ACL",
            Self::Tunnel => "TUNNEL",
            Self::Eni => "ENI",
            Self::DashMeter => "DASH_METER",
            Self::FlowCntTrap => "FLOW_CNT_TRAP",
            Self::FlowCntRoute => "FLOW_CNT_ROUTE",
            Self::MacsecSa => "MACSEC_SA",
            Self::MacsecSaAttr => "MACSEC_SA_ATTR",
            Self::MacsecFlow => "MACSEC_FLOW",
            Self::Pfcwd => "PFCWD",
            Self::WredEcnPort => "WRED_ECN_PORT",
            Self::Srv6 => "SRV6",
            Self::Switch => "SWITCH",
        }
    }

    /// Returns all supported counter groups.
    pub fn all() -> &'static [FlexCounterGroup] {
        &[
            Self::Port,
            Self::PortRates,
            Self::PortBufferDrop,
            Self::Queue,
            Self::QueueWatermark,
            Self::WredEcnQueue,
            Self::PgWatermark,
            Self::PgDrop,
            Self::BufferPoolWatermark,
            Self::Rif,
            Self::RifRates,
            Self::DebugCounter,
            Self::DebugMonitorCounter,
            Self::Acl,
            Self::Tunnel,
            Self::Eni,
            Self::DashMeter,
            Self::FlowCntTrap,
            Self::FlowCntRoute,
            Self::MacsecSa,
            Self::MacsecSaAttr,
            Self::MacsecFlow,
            Self::Pfcwd,
            Self::WredEcnPort,
            Self::Srv6,
            Self::Switch,
        ]
    }

    /// Returns true if this group requires PortsOrch for counter generation.
    pub fn requires_ports_orch(&self) -> bool {
        matches!(
            self,
            Self::Port
                | Self::PortRates
                | Self::PortBufferDrop
                | Self::Queue
                | Self::QueueWatermark
                | Self::WredEcnQueue
                | Self::PgWatermark
                | Self::PgDrop
                | Self::WredEcnPort
        )
    }

    /// Returns true if this group supports gearbox configuration.
    pub fn supports_gearbox(&self) -> bool {
        matches!(
            self,
            Self::Port | Self::PortRates | Self::MacsecSa | Self::MacsecSaAttr | Self::MacsecFlow
        )
    }
}

impl fmt::Display for FlexCounterGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.redis_key())
    }
}

/// Error type for FlexCounterGroup parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFlexCounterGroupError {
    pub invalid_key: String,
}

impl fmt::Display for ParseFlexCounterGroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid flex counter group: {}", self.invalid_key)
    }
}

impl std::error::Error for ParseFlexCounterGroupError {}

impl FromStr for FlexCounterGroup {
    type Err = ParseFlexCounterGroupError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PORT" => Ok(Self::Port),
            "PORT_RATES" => Ok(Self::PortRates),
            "PORT_BUFFER_DROP" => Ok(Self::PortBufferDrop),
            "QUEUE" => Ok(Self::Queue),
            "QUEUE_WATERMARK" => Ok(Self::QueueWatermark),
            "WRED_ECN_QUEUE" => Ok(Self::WredEcnQueue),
            "PG_WATERMARK" => Ok(Self::PgWatermark),
            "PG_DROP" => Ok(Self::PgDrop),
            "BUFFER_POOL_WATERMARK" => Ok(Self::BufferPoolWatermark),
            "RIF" => Ok(Self::Rif),
            "RIF_RATES" => Ok(Self::RifRates),
            "DEBUG_COUNTER" => Ok(Self::DebugCounter),
            "DEBUG_MONITOR_COUNTER" => Ok(Self::DebugMonitorCounter),
            "ACL" => Ok(Self::Acl),
            "TUNNEL" => Ok(Self::Tunnel),
            "ENI" => Ok(Self::Eni),
            "DASH_METER" => Ok(Self::DashMeter),
            "FLOW_CNT_TRAP" => Ok(Self::FlowCntTrap),
            "FLOW_CNT_ROUTE" => Ok(Self::FlowCntRoute),
            "MACSEC_SA" => Ok(Self::MacsecSa),
            "MACSEC_SA_ATTR" => Ok(Self::MacsecSaAttr),
            "MACSEC_FLOW" => Ok(Self::MacsecFlow),
            "PFCWD" => Ok(Self::Pfcwd),
            "WRED_ECN_PORT" => Ok(Self::WredEcnPort),
            "SRV6" => Ok(Self::Srv6),
            "SWITCH" => Ok(Self::Switch),
            _ => Err(ParseFlexCounterGroupError {
                invalid_key: s.to_string(),
            }),
        }
    }
}

/// Mapping of flex counter group names to their SAI group identifiers.
///
/// This is a type-safe replacement for the C++ `flexCounterGroupMap`.
#[derive(Debug, Default)]
pub struct FlexCounterGroupMap {
    /// Enabled state for each group
    enabled: HashMap<FlexCounterGroup, bool>,
    /// Poll interval in milliseconds for each group
    poll_intervals: HashMap<FlexCounterGroup, u64>,
    /// Bulk chunk size for each group (if configured)
    bulk_chunk_sizes: HashMap<FlexCounterGroup, u32>,
}

impl FlexCounterGroupMap {
    /// Creates a new empty group map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the given group is enabled.
    pub fn is_enabled(&self, group: FlexCounterGroup) -> bool {
        self.enabled.get(&group).copied().unwrap_or(false)
    }

    /// Enables or disables a counter group.
    pub fn set_enabled(&mut self, group: FlexCounterGroup, enabled: bool) {
        self.enabled.insert(group, enabled);
    }

    /// Gets the poll interval for a group (in milliseconds).
    pub fn poll_interval(&self, group: FlexCounterGroup) -> Option<u64> {
        self.poll_intervals.get(&group).copied()
    }

    /// Sets the poll interval for a group (in milliseconds).
    pub fn set_poll_interval(&mut self, group: FlexCounterGroup, interval_ms: u64) {
        self.poll_intervals.insert(group, interval_ms);
    }

    /// Gets the bulk chunk size for a group.
    pub fn bulk_chunk_size(&self, group: FlexCounterGroup) -> Option<u32> {
        self.bulk_chunk_sizes.get(&group).copied()
    }

    /// Sets the bulk chunk size for a group.
    pub fn set_bulk_chunk_size(&mut self, group: FlexCounterGroup, size: u32) {
        self.bulk_chunk_sizes.insert(group, size);
    }

    /// Clears the bulk chunk size for a group.
    pub fn clear_bulk_chunk_size(&mut self, group: FlexCounterGroup) {
        self.bulk_chunk_sizes.remove(&group);
    }

    /// Returns an iterator over all enabled groups.
    pub fn enabled_groups(&self) -> impl Iterator<Item = FlexCounterGroup> + '_ {
        self.enabled
            .iter()
            .filter(|(_, &enabled)| enabled)
            .map(|(&group, _)| group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_from_str() {
        assert_eq!(
            "PORT".parse::<FlexCounterGroup>().unwrap(),
            FlexCounterGroup::Port
        );
        assert_eq!(
            "QUEUE".parse::<FlexCounterGroup>().unwrap(),
            FlexCounterGroup::Queue
        );
        assert_eq!(
            "PFCWD".parse::<FlexCounterGroup>().unwrap(),
            FlexCounterGroup::Pfcwd
        );

        assert!("INVALID".parse::<FlexCounterGroup>().is_err());
    }

    #[test]
    fn test_group_sai_name() {
        assert_eq!(FlexCounterGroup::Port.sai_group_name(), "PORT_STAT_COUNTER");
        assert_eq!(
            FlexCounterGroup::Queue.sai_group_name(),
            "QUEUE_STAT_COUNTER"
        );
    }

    #[test]
    fn test_group_redis_key() {
        assert_eq!(FlexCounterGroup::Port.redis_key(), "PORT");
        assert_eq!(
            FlexCounterGroup::QueueWatermark.redis_key(),
            "QUEUE_WATERMARK"
        );
    }

    #[test]
    fn test_group_map() {
        let mut map = FlexCounterGroupMap::new();

        assert!(!map.is_enabled(FlexCounterGroup::Port));

        map.set_enabled(FlexCounterGroup::Port, true);
        map.set_poll_interval(FlexCounterGroup::Port, 10000);

        assert!(map.is_enabled(FlexCounterGroup::Port));
        assert_eq!(map.poll_interval(FlexCounterGroup::Port), Some(10000));

        let enabled: Vec<_> = map.enabled_groups().collect();
        assert_eq!(enabled, vec![FlexCounterGroup::Port]);
    }

    #[test]
    fn test_all_groups_count() {
        assert_eq!(FlexCounterGroup::all().len(), 26);
    }

    #[test]
    fn test_requires_ports_orch() {
        assert!(FlexCounterGroup::Port.requires_ports_orch());
        assert!(FlexCounterGroup::Queue.requires_ports_orch());
        assert!(!FlexCounterGroup::Rif.requires_ports_orch());
        assert!(!FlexCounterGroup::Acl.requires_ports_orch());
    }

    #[test]
    fn test_supports_gearbox() {
        assert!(FlexCounterGroup::Port.supports_gearbox());
        assert!(FlexCounterGroup::MacsecSa.supports_gearbox());
        assert!(!FlexCounterGroup::Queue.supports_gearbox());
    }
}
