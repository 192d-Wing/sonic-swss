//! Counter check types for port counter validation.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CounterCheckKey {
    pub port_name: String,
    pub counter_type: String,
}

impl CounterCheckKey {
    pub fn new(port_name: String, counter_type: String) -> Self {
        Self {
            port_name,
            counter_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CounterCheckConfig {
    pub port_name: String,
    pub counter_type: String,
    pub expected_value: u64,
    pub tolerance: u64,
}

#[derive(Debug, Clone)]
pub struct CounterCheckEntry {
    pub key: CounterCheckKey,
    pub config: CounterCheckConfig,
    pub last_value: u64,
    pub match_count: u64,
}

impl CounterCheckEntry {
    pub fn new(config: CounterCheckConfig) -> Self {
        let key = CounterCheckKey::new(config.port_name.clone(), config.counter_type.clone());
        Self {
            key,
            config,
            last_value: 0,
            match_count: 0,
        }
    }

    pub fn is_within_tolerance(&self, value: u64) -> bool {
        let diff = if value > self.config.expected_value {
            value - self.config.expected_value
        } else {
            self.config.expected_value - value
        };
        diff <= self.config.tolerance
    }
}

#[derive(Debug, Clone, Default)]
pub struct CounterCheckStats {
    pub checks_performed: u64,
    pub matches: u64,
    pub mismatches: u64,
}
