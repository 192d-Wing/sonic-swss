//! CoPP Manager Type Definitions

use sonic_cfgmgr_common::FieldValues;
use std::collections::HashMap;

/// CoPP trap configuration
#[derive(Debug, Clone, PartialEq)]
pub struct CoppTrapConf {
    /// Comma-separated list of trap IDs (e.g., "arp_req,arp_resp")
    pub trap_ids: String,

    /// Trap group name this trap belongs to
    pub trap_group: String,

    /// Whether trap is always enabled regardless of feature state
    pub is_always_enabled: bool,
}

impl CoppTrapConf {
    pub fn new(trap_ids: String, trap_group: String, is_always_enabled: bool) -> Self {
        Self {
            trap_ids,
            trap_group,
            is_always_enabled,
        }
    }

    /// Parse always_enabled field from string
    pub fn parse_always_enabled(value: &str) -> bool {
        value == "true"
    }
}

/// Trap name → trap configuration mapping
pub type CoppTrapConfMap = HashMap<String, CoppTrapConf>;

/// Trap ID → group name mapping
///
/// Maps individual trap IDs to their group names.
/// Example: "arp_req" → "queue1_group1"
pub type CoppTrapIdGroupMap = HashMap<String, String>;

/// Configuration map: Key → field values
///
/// Used for both trap and group configurations from JSON and CONFIG_DB
pub type CoppCfg = HashMap<String, FieldValues>;

/// Group field values: Group → (field → value) nested map
///
/// Stores the current field values for each COPP_GROUP
pub type CoppGroupFvs = HashMap<String, HashMap<String, String>>;

/// Feature configurations: Feature → field values
///
/// Tracks FEATURE table entries (state: enabled/disabled)
pub type FeaturesCfg = HashMap<String, FieldValues>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copp_trap_conf_new() {
        let conf = CoppTrapConf::new(
            "arp_req,arp_resp".to_string(),
            "queue1_group1".to_string(),
            true,
        );

        assert_eq!(conf.trap_ids, "arp_req,arp_resp");
        assert_eq!(conf.trap_group, "queue1_group1");
        assert!(conf.is_always_enabled);
    }

    #[test]
    fn test_parse_always_enabled() {
        assert!(CoppTrapConf::parse_always_enabled("true"));
        assert!(!CoppTrapConf::parse_always_enabled("false"));
        assert!(!CoppTrapConf::parse_always_enabled(""));
        assert!(!CoppTrapConf::parse_always_enabled("invalid"));
    }
}
