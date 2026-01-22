//! Debug counter types and structures.

use sonic_sai::types::RawSaiObjectId;
use std::collections::{HashMap, HashSet};

/// Debug counter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugCounterType {
    /// Port ingress drops.
    PortIngressDrops,
    /// Port egress drops.
    PortEgressDrops,
    /// Switch ingress drops.
    SwitchIngressDrops,
    /// Switch egress drops.
    SwitchEgressDrops,
}

impl DebugCounterType {
    /// Parses a debug counter type from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "PORT_INGRESS_DROPS" => Some(Self::PortIngressDrops),
            "PORT_EGRESS_DROPS" => Some(Self::PortEgressDrops),
            "SWITCH_INGRESS_DROPS" => Some(Self::SwitchIngressDrops),
            "SWITCH_EGRESS_DROPS" => Some(Self::SwitchEgressDrops),
            _ => None,
        }
    }

    /// Converts to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PortIngressDrops => "PORT_INGRESS_DROPS",
            Self::PortEgressDrops => "PORT_EGRESS_DROPS",
            Self::SwitchIngressDrops => "SWITCH_INGRESS_DROPS",
            Self::SwitchEgressDrops => "SWITCH_EGRESS_DROPS",
        }
    }

    /// Returns true if this is a port-level counter.
    pub fn is_port_counter(&self) -> bool {
        matches!(self, Self::PortIngressDrops | Self::PortEgressDrops)
    }

    /// Returns true if this is a switch-level counter.
    pub fn is_switch_counter(&self) -> bool {
        matches!(self, Self::SwitchIngressDrops | Self::SwitchEgressDrops)
    }

    /// Returns true if this is an ingress counter.
    pub fn is_ingress(&self) -> bool {
        matches!(self, Self::PortIngressDrops | Self::SwitchIngressDrops)
    }

    /// Returns true if this is an egress counter.
    pub fn is_egress(&self) -> bool {
        matches!(self, Self::PortEgressDrops | Self::SwitchEgressDrops)
    }
}

/// Drop reason (ingress or egress).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DropReason {
    /// Drop reason name.
    pub name: String,
    /// Whether this is an ingress drop reason.
    pub is_ingress: bool,
}

impl DropReason {
    /// Creates a new drop reason.
    pub fn new(name: String, is_ingress: bool) -> Self {
        Self { name, is_ingress }
    }

    /// Creates an ingress drop reason.
    pub fn ingress(name: String) -> Self {
        Self::new(name, true)
    }

    /// Creates an egress drop reason.
    pub fn egress(name: String) -> Self {
        Self::new(name, false)
    }
}

/// Debug counter configuration.
#[derive(Debug, Clone)]
pub struct DebugCounterConfig {
    /// Counter name.
    pub name: String,
    /// Counter type.
    pub counter_type: DebugCounterType,
    /// Description.
    pub description: Option<String>,
    /// Drop reasons.
    pub drop_reasons: HashSet<String>,
}

impl DebugCounterConfig {
    /// Creates a new debug counter configuration.
    pub fn new(name: String, counter_type: DebugCounterType) -> Self {
        Self {
            name,
            counter_type,
            description: None,
            drop_reasons: HashSet::new(),
        }
    }

    /// Sets the description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Adds a drop reason.
    pub fn add_drop_reason(&mut self, reason: String) -> bool {
        self.drop_reasons.insert(reason)
    }

    /// Removes a drop reason.
    pub fn remove_drop_reason(&mut self, reason: &str) -> bool {
        self.drop_reasons.remove(reason)
    }

    /// Gets the number of drop reasons.
    pub fn drop_reason_count(&self) -> usize {
        self.drop_reasons.len()
    }
}

/// Debug counter entry.
#[derive(Debug, Clone)]
pub struct DebugCounterEntry {
    /// Counter name.
    pub name: String,
    /// Counter type.
    pub counter_type: DebugCounterType,
    /// SAI debug counter OID.
    pub counter_id: RawSaiObjectId,
    /// Description.
    pub description: Option<String>,
    /// Drop reasons.
    pub drop_reasons: HashSet<String>,
}

impl DebugCounterEntry {
    /// Creates a new debug counter entry.
    pub fn new(
        name: String,
        counter_type: DebugCounterType,
        counter_id: RawSaiObjectId,
    ) -> Self {
        Self {
            name,
            counter_type,
            counter_id,
            description: None,
            drop_reasons: HashSet::new(),
        }
    }

    /// Adds a drop reason.
    pub fn add_drop_reason(&mut self, reason: String) -> bool {
        self.drop_reasons.insert(reason)
    }

    /// Removes a drop reason.
    pub fn remove_drop_reason(&mut self, reason: &str) -> bool {
        self.drop_reasons.remove(reason)
    }

    /// Checks if a drop reason exists.
    pub fn has_drop_reason(&mut self, reason: &str) -> bool {
        self.drop_reasons.contains(reason)
    }

    /// Gets the number of drop reasons.
    pub fn drop_reason_count(&self) -> usize {
        self.drop_reasons.len()
    }
}

/// Free counter tracking (counter without drop reasons).
#[derive(Debug, Clone)]
pub struct FreeCounter {
    /// Counter name.
    pub name: String,
    /// Counter type.
    pub counter_type: String,
}

impl FreeCounter {
    /// Creates a new free counter.
    pub fn new(name: String, counter_type: String) -> Self {
        Self { name, counter_type }
    }
}

/// Drop monitor statistics.
#[derive(Debug, Clone, Default)]
pub struct DropMonitorStats {
    /// Number of drops detected.
    pub drops_detected: u64,
    /// Number of monitors active.
    pub monitors_active: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_counter_type_parse() {
        assert_eq!(
            DebugCounterType::parse("PORT_INGRESS_DROPS"),
            Some(DebugCounterType::PortIngressDrops)
        );
        assert_eq!(
            DebugCounterType::parse("port_ingress_drops"),
            Some(DebugCounterType::PortIngressDrops)
        );
        assert_eq!(
            DebugCounterType::parse("SWITCH_EGRESS_DROPS"),
            Some(DebugCounterType::SwitchEgressDrops)
        );
        assert_eq!(DebugCounterType::parse("INVALID"), None);
    }

    #[test]
    fn test_debug_counter_type_classification() {
        assert!(DebugCounterType::PortIngressDrops.is_port_counter());
        assert!(DebugCounterType::PortIngressDrops.is_ingress());
        assert!(!DebugCounterType::PortIngressDrops.is_switch_counter());
        assert!(!DebugCounterType::PortIngressDrops.is_egress());

        assert!(DebugCounterType::SwitchEgressDrops.is_switch_counter());
        assert!(DebugCounterType::SwitchEgressDrops.is_egress());
        assert!(!DebugCounterType::SwitchEgressDrops.is_port_counter());
        assert!(!DebugCounterType::SwitchEgressDrops.is_ingress());
    }

    #[test]
    fn test_drop_reason() {
        let ingress = DropReason::ingress("L3_ANY".to_string());
        assert!(ingress.is_ingress);
        assert_eq!(ingress.name, "L3_ANY");

        let egress = DropReason::egress("L2_ANY".to_string());
        assert!(!egress.is_ingress);
        assert_eq!(egress.name, "L2_ANY");
    }

    #[test]
    fn test_debug_counter_config() {
        let mut config =
            DebugCounterConfig::new("counter1".to_string(), DebugCounterType::PortIngressDrops)
                .with_description("Test counter".to_string());

        assert_eq!(config.name, "counter1");
        assert_eq!(config.counter_type, DebugCounterType::PortIngressDrops);
        assert_eq!(config.description, Some("Test counter".to_string()));

        assert!(config.add_drop_reason("L3_ANY".to_string()));
        assert!(!config.add_drop_reason("L3_ANY".to_string())); // Duplicate

        assert_eq!(config.drop_reason_count(), 1);

        assert!(config.remove_drop_reason("L3_ANY"));
        assert!(!config.remove_drop_reason("L3_ANY"));
        assert_eq!(config.drop_reason_count(), 0);
    }

    #[test]
    fn test_debug_counter_entry() {
        let mut entry = DebugCounterEntry::new(
            "counter1".to_string(),
            DebugCounterType::SwitchIngressDrops,
            0x1234,
        );

        assert_eq!(entry.counter_id, 0x1234);
        assert_eq!(entry.drop_reason_count(), 0);

        assert!(entry.add_drop_reason("L3_DEST_MISS".to_string()));
        assert!(entry.add_drop_reason("L3_SRC_MISS".to_string()));
        assert_eq!(entry.drop_reason_count(), 2);

        assert!(entry.has_drop_reason("L3_DEST_MISS"));
        assert!(!entry.has_drop_reason("L2_ANY"));

        assert!(entry.remove_drop_reason("L3_DEST_MISS"));
        assert_eq!(entry.drop_reason_count(), 1);
    }
}
