//! ACL range types for L4 port range matching.
//!
//! ACL ranges are used when matching on port ranges (e.g., L4_SRC_PORT_RANGE).
//! They are shared resources that can be reused across multiple rules.

use sonic_sai::types::RawSaiObjectId;
use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

/// ACL range type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AclRangeType {
    /// L4 source port range.
    L4SrcPort,
    /// L4 destination port range.
    L4DstPort,
    /// Outer VLAN range.
    OuterVlan,
    /// Inner VLAN range.
    InnerVlan,
    /// Packet length range.
    PacketLength,
}

impl fmt::Display for AclRangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::L4SrcPort => write!(f, "L4_SRC_PORT"),
            Self::L4DstPort => write!(f, "L4_DST_PORT"),
            Self::OuterVlan => write!(f, "OUTER_VLAN"),
            Self::InnerVlan => write!(f, "INNER_VLAN"),
            Self::PacketLength => write!(f, "PACKET_LENGTH"),
        }
    }
}

/// Properties that uniquely identify an ACL range.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AclRangeProperties {
    /// Range type.
    pub range_type: AclRangeType,
    /// Minimum value (inclusive).
    pub min: u32,
    /// Maximum value (inclusive).
    pub max: u32,
}

impl AclRangeProperties {
    /// Creates new range properties.
    pub fn new(range_type: AclRangeType, min: u32, max: u32) -> Self {
        Self {
            range_type,
            min,
            max,
        }
    }

    /// Validates the range.
    pub fn validate(&self) -> Result<(), String> {
        if self.min > self.max {
            return Err(format!(
                "Invalid range: min ({}) > max ({})",
                self.min, self.max
            ));
        }

        // Validate based on type
        match self.range_type {
            AclRangeType::L4SrcPort | AclRangeType::L4DstPort => {
                if self.max > 65535 {
                    return Err(format!("Port range max ({}) exceeds 65535", self.max));
                }
            }
            AclRangeType::OuterVlan | AclRangeType::InnerVlan => {
                if self.max > 4094 {
                    return Err(format!("VLAN range max ({}) exceeds 4094", self.max));
                }
            }
            AclRangeType::PacketLength => {
                // No specific limit for packet length
            }
        }

        Ok(())
    }
}

impl fmt::Display for AclRangeProperties {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}-{}", self.range_type, self.min, self.max)
    }
}

/// ACL range with SAI object tracking.
#[derive(Debug)]
pub struct AclRange {
    /// Range properties.
    pub properties: AclRangeProperties,
    /// SAI object ID for the range.
    pub oid: RawSaiObjectId,
    /// Reference count (number of rules using this range).
    ref_count: u32,
}

impl AclRange {
    /// Creates a new ACL range.
    pub fn new(properties: AclRangeProperties, oid: RawSaiObjectId) -> Self {
        Self {
            properties,
            oid,
            ref_count: 1, // Created with one reference
        }
    }

    /// Increments the reference count.
    pub fn increment_ref(&mut self) -> u32 {
        self.ref_count += 1;
        self.ref_count
    }

    /// Decrements the reference count.
    /// Returns the new count (0 means can be removed).
    pub fn decrement_ref(&mut self) -> u32 {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
        self.ref_count
    }

    /// Returns the current reference count.
    pub fn ref_count(&self) -> u32 {
        self.ref_count
    }

    /// Returns true if this range can be removed (ref count is 0).
    pub fn can_remove(&self) -> bool {
        self.ref_count == 0
    }
}

/// Global ACL range cache.
///
/// Ranges are shared across rules, so we maintain a global cache to avoid
/// creating duplicate SAI objects for the same range.
///
/// This replaces the C++ static `map<acl_range_properties_t, AclRange*>`.
#[derive(Debug, Default)]
pub struct AclRangeCache {
    /// Ranges indexed by properties.
    ranges: RwLock<HashMap<AclRangeProperties, AclRange>>,
}

impl AclRangeCache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self {
            ranges: RwLock::new(HashMap::new()),
        }
    }

    /// Gets or creates a range.
    ///
    /// If the range already exists, increments its reference count.
    /// If not, calls the creator function to create it.
    pub fn get_or_create<F>(
        &self,
        properties: AclRangeProperties,
        create_fn: F,
    ) -> Result<RawSaiObjectId, String>
    where
        F: FnOnce(&AclRangeProperties) -> Result<RawSaiObjectId, String>,
    {
        // First try to get existing range
        {
            let mut ranges = self.ranges.write().map_err(|e| e.to_string())?;
            if let Some(range) = ranges.get_mut(&properties) {
                range.increment_ref();
                return Ok(range.oid);
            }
        }

        // Create new range
        let oid = create_fn(&properties)?;

        // Insert into cache
        let mut ranges = self.ranges.write().map_err(|e| e.to_string())?;
        ranges.insert(properties.clone(), AclRange::new(properties, oid));

        Ok(oid)
    }

    /// Releases a range reference.
    ///
    /// If the reference count reaches 0, calls the remove function and
    /// removes the range from the cache.
    pub fn release<F>(&self, properties: &AclRangeProperties, remove_fn: F) -> Result<(), String>
    where
        F: FnOnce(RawSaiObjectId) -> Result<(), String>,
    {
        let mut ranges = self.ranges.write().map_err(|e| e.to_string())?;

        if let Some(range) = ranges.get_mut(properties) {
            let new_count = range.decrement_ref();
            if new_count == 0 {
                let oid = range.oid;
                ranges.remove(properties);
                return remove_fn(oid);
            }
        }

        Ok(())
    }

    /// Gets a range by properties (if it exists).
    pub fn get(&self, properties: &AclRangeProperties) -> Option<RawSaiObjectId> {
        self.ranges.read().ok()?.get(properties).map(|r| r.oid)
    }

    /// Returns the number of cached ranges.
    pub fn len(&self) -> usize {
        self.ranges.read().map(|r| r.len()).unwrap_or(0)
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Configuration for parsing a port range from config.
#[derive(Debug, Clone)]
pub struct AclRangeConfig {
    /// Range type.
    pub range_type: AclRangeType,
    /// Minimum value.
    pub min: u32,
    /// Maximum value.
    pub max: u32,
}

impl AclRangeConfig {
    /// Creates a new range config.
    pub fn new(range_type: AclRangeType, min: u32, max: u32) -> Self {
        Self {
            range_type,
            min,
            max,
        }
    }

    /// Parses a range config from a string like "1000-2000".
    pub fn parse(range_type: AclRangeType, value: &str) -> Result<Self, String> {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid range format: {}", value));
        }

        let min = parts[0]
            .trim()
            .parse()
            .map_err(|_| format!("Invalid range min: {}", parts[0]))?;
        let max = parts[1]
            .trim()
            .parse()
            .map_err(|_| format!("Invalid range max: {}", parts[1]))?;

        let config = Self::new(range_type, min, max);
        config.validate()?;
        Ok(config)
    }

    /// Validates the range config.
    pub fn validate(&self) -> Result<(), String> {
        AclRangeProperties::new(self.range_type, self.min, self.max).validate()
    }

    /// Converts to range properties.
    pub fn to_properties(&self) -> AclRangeProperties {
        AclRangeProperties::new(self.range_type, self.min, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_type_display() {
        assert_eq!(AclRangeType::L4SrcPort.to_string(), "L4_SRC_PORT");
        assert_eq!(AclRangeType::L4DstPort.to_string(), "L4_DST_PORT");
    }

    #[test]
    fn test_range_properties_validate() {
        // Valid port range
        let props = AclRangeProperties::new(AclRangeType::L4SrcPort, 1000, 2000);
        assert!(props.validate().is_ok());

        // Invalid: min > max
        let props = AclRangeProperties::new(AclRangeType::L4SrcPort, 2000, 1000);
        assert!(props.validate().is_err());

        // Invalid: port > 65535
        let props = AclRangeProperties::new(AclRangeType::L4DstPort, 0, 70000);
        assert!(props.validate().is_err());

        // Invalid: VLAN > 4094
        let props = AclRangeProperties::new(AclRangeType::OuterVlan, 1, 5000);
        assert!(props.validate().is_err());
    }

    #[test]
    fn test_range_properties_display() {
        let props = AclRangeProperties::new(AclRangeType::L4SrcPort, 1000, 2000);
        assert_eq!(props.to_string(), "L4_SRC_PORT:1000-2000");
    }

    #[test]
    fn test_acl_range() {
        let props = AclRangeProperties::new(AclRangeType::L4SrcPort, 1000, 2000);
        let mut range = AclRange::new(props, 0x1234);

        assert_eq!(range.ref_count(), 1);
        assert!(!range.can_remove());

        range.increment_ref();
        assert_eq!(range.ref_count(), 2);

        range.decrement_ref();
        assert_eq!(range.ref_count(), 1);

        range.decrement_ref();
        assert_eq!(range.ref_count(), 0);
        assert!(range.can_remove());

        // Decrementing past 0 should stay at 0
        range.decrement_ref();
        assert_eq!(range.ref_count(), 0);
    }

    #[test]
    fn test_range_config_parse() {
        let config = AclRangeConfig::parse(AclRangeType::L4SrcPort, "1000-2000").unwrap();
        assert_eq!(config.min, 1000);
        assert_eq!(config.max, 2000);

        // Invalid format
        assert!(AclRangeConfig::parse(AclRangeType::L4SrcPort, "1000").is_err());
        assert!(AclRangeConfig::parse(AclRangeType::L4SrcPort, "abc-def").is_err());
    }

    #[test]
    fn test_range_cache() {
        let cache = AclRangeCache::new();
        assert!(cache.is_empty());

        let props = AclRangeProperties::new(AclRangeType::L4SrcPort, 1000, 2000);

        // Create first range
        let oid1 = cache.get_or_create(props.clone(), |_| Ok(0x1234)).unwrap();
        assert_eq!(oid1, 0x1234);
        assert_eq!(cache.len(), 1);

        // Get same range (should increment ref count)
        let oid2 = cache.get_or_create(props.clone(), |_| Ok(0x5678)).unwrap();
        assert_eq!(oid2, 0x1234); // Same OID, not new
        assert_eq!(cache.len(), 1);

        // Release one reference
        cache.release(&props, |_| Ok(())).unwrap();
        assert_eq!(cache.len(), 1); // Still exists (ref count = 1)

        // Release last reference
        cache.release(&props, |_| Ok(())).unwrap();
        assert!(cache.is_empty()); // Now removed
    }
}
