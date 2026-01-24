//! Counter check orchestration logic.

use super::types::{CounterCheckEntry, CounterCheckKey, CounterCheckStats};
use std::collections::HashMap;
use thiserror::Error;
use crate::audit_log;

#[derive(Debug, Clone, Error)]
pub enum CounterCheckOrchError {
    #[error("Check not found: {0:?}")]
    CheckNotFound(CounterCheckKey),
    #[error("Port not found: {0}")]
    PortNotFound(String),
}

#[derive(Debug, Clone, Default)]
pub struct CounterCheckOrchConfig {}

#[derive(Debug, Clone, Default)]
pub struct CounterCheckOrchStats {
    pub stats: CounterCheckStats,
}

pub trait CounterCheckOrchCallbacks: Send + Sync {}

pub struct CounterCheckOrch {
    config: CounterCheckOrchConfig,
    stats: CounterCheckOrchStats,
    checks: HashMap<CounterCheckKey, CounterCheckEntry>,
}

impl CounterCheckOrch {
    pub fn new(config: CounterCheckOrchConfig) -> Self {
        Self {
            config,
            stats: CounterCheckOrchStats::default(),
            checks: HashMap::new(),
        }
    }

    pub fn get_check(&self, key: &CounterCheckKey) -> Option<&CounterCheckEntry> {
        self.checks.get(key)
    }

    pub fn add_check(&mut self, key: CounterCheckKey, entry: CounterCheckEntry) {
        let resource_id = format!("{}_{}", key.port_name, key.counter_type);
        self.checks.insert(key.clone(), entry.clone());

        audit_log!(
            resource_id: &resource_id,
            action: "add_counter_check",
            category: "ResourceCreate",
            outcome: "SUCCESS",
            details: serde_json::json!({
                "port_name": key.port_name,
                "counter_type": key.counter_type,
                "expected_value": entry.config.expected_value,
                "tolerance": entry.config.tolerance,
            })
        );
    }

    pub fn remove_check(&mut self, key: &CounterCheckKey) -> Option<CounterCheckEntry> {
        let resource_id = format!("{}_{}", key.port_name, key.counter_type);
        let removed = self.checks.remove(key);

        if removed.is_some() {
            audit_log!(
                resource_id: &resource_id,
                action: "remove_counter_check",
                category: "ResourceDelete",
                outcome: "SUCCESS",
                details: serde_json::json!({
                    "port_name": key.port_name,
                    "counter_type": key.counter_type,
                })
            );
        } else {
            audit_log!(
                resource_id: &resource_id,
                action: "remove_counter_check",
                category: "ResourceDelete",
                outcome: "FAIL",
                details: serde_json::json!({
                    "error": "Check not found",
                    "port_name": key.port_name,
                    "counter_type": key.counter_type,
                })
            );
        }

        removed
    }

    pub fn check_count(&self) -> usize {
        self.checks.len()
    }

    pub fn validate_counters(&mut self) -> Result<usize, CounterCheckOrchError> {
        let mut validated = 0usize;

        for (key, entry) in &self.checks {
            let is_valid = entry.is_within_tolerance(entry.last_value);

            audit_log!(
                resource_id: &format!("{}_{}", key.port_name, key.counter_type),
                action: "validate_counters",
                category: "SecurityPolicy",
                outcome: if is_valid { "SUCCESS" } else { "FAIL" },
                details: serde_json::json!({
                    "port_name": key.port_name,
                    "counter_type": key.counter_type,
                    "last_value": entry.last_value,
                    "expected_value": entry.config.expected_value,
                    "tolerance": entry.config.tolerance,
                    "within_tolerance": is_valid,
                })
            );

            if is_valid {
                validated += 1;
            }
        }

        Ok(validated)
    }

    pub fn stats(&self) -> &CounterCheckOrchStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::countercheck::types::{CounterCheckConfig, CounterCheckEntry, CounterCheckKey};

    // Helper function to create a test config
    fn create_test_config(
        port_name: &str,
        counter_type: &str,
        expected_value: u64,
        tolerance: u64,
    ) -> CounterCheckConfig {
        CounterCheckConfig {
            port_name: port_name.to_string(),
            counter_type: counter_type.to_string(),
            expected_value,
            tolerance,
        }
    }

    // Helper function to create a test key
    fn create_test_key(port_name: &str, counter_type: &str) -> CounterCheckKey {
        CounterCheckKey::new(port_name.to_string(), counter_type.to_string())
    }

    // ============================================================================
    // 1. Counter Check Configuration Tests
    // ============================================================================

    #[test]
    fn test_new_countercheck_orch_with_default_config() {
        let config = CounterCheckOrchConfig::default();
        let orch = CounterCheckOrch::new(config);

        assert_eq!(orch.checks.len(), 0);
        assert_eq!(orch.stats.stats.checks_performed, 0);
        assert_eq!(orch.stats.stats.matches, 0);
        assert_eq!(orch.stats.stats.mismatches, 0);
    }

    #[test]
    fn test_new_countercheck_orch_with_custom_config() {
        let config = CounterCheckOrchConfig {};
        let orch = CounterCheckOrch::new(config);

        assert!(orch.checks.is_empty());
        assert_eq!(orch.stats().stats.checks_performed, 0);
    }

    #[test]
    fn test_get_check_returns_none_when_empty() {
        let orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());
        let key = create_test_key("Ethernet0", "RX_PACKETS");

        assert!(orch.get_check(&key).is_none());
    }

    // ============================================================================
    // 2. Port Counter Checks Tests
    // ============================================================================

    #[test]
    fn test_counter_check_entry_rx_packets() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.port_name, "Ethernet0");
        assert_eq!(entry.key.counter_type, "RX_PACKETS");
        assert_eq!(entry.config.expected_value, 1000);
        assert_eq!(entry.config.tolerance, 100);
        assert_eq!(entry.last_value, 0);
        assert_eq!(entry.match_count, 0);
    }

    #[test]
    fn test_counter_check_entry_tx_packets() {
        let config = create_test_config("Ethernet1", "TX_PACKETS", 2000, 200);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.port_name, "Ethernet1");
        assert_eq!(entry.key.counter_type, "TX_PACKETS");
        assert_eq!(entry.config.expected_value, 2000);
        assert_eq!(entry.config.tolerance, 200);
    }

    #[test]
    fn test_counter_check_entry_rx_bytes() {
        let config = create_test_config("Ethernet2", "RX_BYTES", 1500000, 10000);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.port_name, "Ethernet2");
        assert_eq!(entry.key.counter_type, "RX_BYTES");
        assert_eq!(entry.config.expected_value, 1500000);
    }

    #[test]
    fn test_counter_check_entry_tx_bytes() {
        let config = create_test_config("Ethernet3", "TX_BYTES", 2500000, 20000);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.port_name, "Ethernet3");
        assert_eq!(entry.key.counter_type, "TX_BYTES");
        assert_eq!(entry.config.expected_value, 2500000);
    }

    #[test]
    fn test_counter_check_entry_error_counters() {
        let config = create_test_config("Ethernet4", "RX_ERRORS", 0, 5);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.counter_type, "RX_ERRORS");
        assert_eq!(entry.config.expected_value, 0);
        assert_eq!(entry.config.tolerance, 5);
    }

    #[test]
    fn test_counter_check_entry_discard_counters() {
        let config = create_test_config("Ethernet5", "RX_DISCARDS", 0, 10);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.counter_type, "RX_DISCARDS");
        assert_eq!(entry.config.expected_value, 0);
    }

    // ============================================================================
    // 3. Threshold Configuration Tests
    // ============================================================================

    #[test]
    fn test_is_within_tolerance_exact_match() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(1000));
    }

    #[test]
    fn test_is_within_tolerance_within_upper_bound() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(1050));
        assert!(entry.is_within_tolerance(1100));
    }

    #[test]
    fn test_is_within_tolerance_within_lower_bound() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(950));
        assert!(entry.is_within_tolerance(900));
    }

    #[test]
    fn test_is_within_tolerance_exceeds_upper_threshold() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert!(!entry.is_within_tolerance(1101));
        assert!(!entry.is_within_tolerance(1500));
    }

    #[test]
    fn test_is_within_tolerance_exceeds_lower_threshold() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert!(!entry.is_within_tolerance(899));
        assert!(!entry.is_within_tolerance(500));
    }

    #[test]
    fn test_threshold_zero_tolerance() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 0);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(1000));
        assert!(!entry.is_within_tolerance(1001));
        assert!(!entry.is_within_tolerance(999));
    }

    #[test]
    fn test_threshold_large_tolerance() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, u64::MAX - 1001);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(0));
        assert!(entry.is_within_tolerance(u64::MAX - 1));
    }

    // ============================================================================
    // 4. Check Operations Tests
    // ============================================================================

    #[test]
    fn test_counter_check_key_equality() {
        let key1 = create_test_key("Ethernet0", "RX_PACKETS");
        let key2 = create_test_key("Ethernet0", "RX_PACKETS");
        let key3 = create_test_key("Ethernet1", "RX_PACKETS");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_counter_check_key_hashing() {
        let mut map = HashMap::new();
        let key = create_test_key("Ethernet0", "RX_PACKETS");
        map.insert(key.clone(), "test_value");

        assert_eq!(map.get(&key), Some(&"test_value"));
    }

    #[test]
    fn test_get_check_retrieval() {
        let mut orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);
        let key = entry.key.clone();

        orch.checks.insert(key.clone(), entry);

        let retrieved = orch.get_check(&key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().key.port_name, "Ethernet0");
    }

    // ============================================================================
    // 5. Statistics Tests
    // ============================================================================

    #[test]
    fn test_stats_initialization() {
        let orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());
        let stats = orch.stats();

        assert_eq!(stats.stats.checks_performed, 0);
        assert_eq!(stats.stats.matches, 0);
        assert_eq!(stats.stats.mismatches, 0);
    }

    #[test]
    fn test_stats_default_values() {
        let stats = CounterCheckStats::default();

        assert_eq!(stats.checks_performed, 0);
        assert_eq!(stats.matches, 0);
        assert_eq!(stats.mismatches, 0);
    }

    #[test]
    fn test_orch_stats_default_values() {
        let orch_stats = CounterCheckOrchStats::default();

        assert_eq!(orch_stats.stats.checks_performed, 0);
        assert_eq!(orch_stats.stats.matches, 0);
        assert_eq!(orch_stats.stats.mismatches, 0);
    }

    // ============================================================================
    // 6. Error Handling Tests
    // ============================================================================

    #[test]
    fn test_error_check_not_found() {
        let key = create_test_key("Ethernet0", "RX_PACKETS");
        let error = CounterCheckOrchError::CheckNotFound(key.clone());

        match error {
            CounterCheckOrchError::CheckNotFound(k) => {
                assert_eq!(k.port_name, "Ethernet0");
                assert_eq!(k.counter_type, "RX_PACKETS");
            }
            _ => panic!("Expected CheckNotFound error"),
        }
    }

    #[test]
    fn test_error_port_not_found() {
        let error = CounterCheckOrchError::PortNotFound("Ethernet99".to_string());

        match error {
            CounterCheckOrchError::PortNotFound(port) => {
                assert_eq!(port, "Ethernet99");
            }
            _ => panic!("Expected PortNotFound error"),
        }
    }

    #[test]
    fn test_error_clone() {
        let error = CounterCheckOrchError::PortNotFound("Ethernet0".to_string());
        let cloned = error.clone();

        match cloned {
            CounterCheckOrchError::PortNotFound(port) => {
                assert_eq!(port, "Ethernet0");
            }
            _ => panic!("Expected PortNotFound error"),
        }
    }

    // ============================================================================
    // 7. Edge Cases Tests
    // ============================================================================

    #[test]
    fn test_counter_max_value_threshold() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", u64::MAX, 0);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(u64::MAX));
        assert_eq!(entry.config.expected_value, u64::MAX);
    }

    #[test]
    fn test_counter_zero_expected_value() {
        let config = create_test_config("Ethernet0", "RX_ERRORS", 0, 5);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(0));
        assert!(entry.is_within_tolerance(5));
        assert!(!entry.is_within_tolerance(6));
    }

    #[test]
    fn test_counter_overflow_handling_near_max() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", u64::MAX - 100, 50);
        let entry = CounterCheckEntry::new(config);

        assert!(entry.is_within_tolerance(u64::MAX - 50));
        assert!(entry.is_within_tolerance(u64::MAX - 150));
        assert!(!entry.is_within_tolerance(u64::MAX));

        // Test with larger tolerance
        let config2 = create_test_config("Ethernet0", "RX_PACKETS", u64::MAX - 100, 100);
        let entry2 = CounterCheckEntry::new(config2);
        assert!(entry2.is_within_tolerance(u64::MAX));
    }

    #[test]
    fn test_multiple_ports_same_counter_type() {
        let mut orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());

        let config1 = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry1 = CounterCheckEntry::new(config1);
        let key1 = entry1.key.clone();

        let config2 = create_test_config("Ethernet1", "RX_PACKETS", 2000, 200);
        let entry2 = CounterCheckEntry::new(config2);
        let key2 = entry2.key.clone();

        orch.checks.insert(key1.clone(), entry1);
        orch.checks.insert(key2.clone(), entry2);

        assert_eq!(orch.checks.len(), 2);
        assert!(orch.get_check(&key1).is_some());
        assert!(orch.get_check(&key2).is_some());
    }

    #[test]
    fn test_same_port_multiple_counter_types() {
        let mut orch = CounterCheckOrch::new(CounterCheckOrchConfig::default());

        let config1 = create_test_config("Ethernet0", "RX_PACKETS", 1000, 100);
        let entry1 = CounterCheckEntry::new(config1);
        let key1 = entry1.key.clone();

        let config2 = create_test_config("Ethernet0", "TX_PACKETS", 2000, 200);
        let entry2 = CounterCheckEntry::new(config2);
        let key2 = entry2.key.clone();

        orch.checks.insert(key1.clone(), entry1);
        orch.checks.insert(key2.clone(), entry2);

        assert_eq!(orch.checks.len(), 2);
        let check1 = orch.get_check(&key1).unwrap();
        let check2 = orch.get_check(&key2).unwrap();

        assert_eq!(check1.config.expected_value, 1000);
        assert_eq!(check2.config.expected_value, 2000);
    }

    #[test]
    fn test_rapid_counter_changes_tolerance() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 1000, 500);
        let entry = CounterCheckEntry::new(config);

        // Simulate rapid changes within tolerance
        assert!(entry.is_within_tolerance(500));
        assert!(entry.is_within_tolerance(750));
        assert!(entry.is_within_tolerance(1000));
        assert!(entry.is_within_tolerance(1250));
        assert!(entry.is_within_tolerance(1500));

        // Beyond tolerance
        assert!(!entry.is_within_tolerance(1501));
        assert!(!entry.is_within_tolerance(499));
    }

    #[test]
    fn test_entry_initial_state() {
        let config = create_test_config("Ethernet0", "RX_PACKETS", 5000, 100);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.last_value, 0);
        assert_eq!(entry.match_count, 0);
        assert_eq!(entry.config.expected_value, 5000);
        assert_eq!(entry.config.tolerance, 100);
    }

    #[test]
    fn test_port_name_special_characters() {
        let config = create_test_config("Ethernet-1/2/3", "RX_PACKETS", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.port_name, "Ethernet-1/2/3");
    }

    #[test]
    fn test_counter_type_custom_names() {
        let config = create_test_config("Ethernet0", "CUSTOM_COUNTER_TYPE", 1000, 100);
        let entry = CounterCheckEntry::new(config);

        assert_eq!(entry.key.counter_type, "CUSTOM_COUNTER_TYPE");
    }
}
