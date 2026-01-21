//! FlexCounter state management for queues and priority groups.
//!
//! These types track which specific queue indices or PG indices have
//! counters enabled, allowing for selective counter collection.

use std::collections::HashMap;

/// Tracks enabled counter states for queues on a port.
///
/// This is a type-safe replacement for the C++ `FlexCounterQueueStates`.
/// Unlike the C++ version which uses raw vector indexing, this version
/// provides bounds-checked access.
#[derive(Debug, Clone, Default)]
pub struct FlexCounterQueueStates {
    /// Per-queue enable states (index = queue index)
    queue_states: Vec<bool>,
}

impl FlexCounterQueueStates {
    /// Creates a new state tracker with the given maximum queue count.
    pub fn new(max_queues: usize) -> Self {
        Self {
            queue_states: vec![false; max_queues],
        }
    }

    /// Creates a new state tracker with all queues enabled.
    pub fn all_enabled(max_queues: usize) -> Self {
        Self {
            queue_states: vec![true; max_queues],
        }
    }

    /// Returns the number of queues tracked.
    pub fn len(&self) -> usize {
        self.queue_states.len()
    }

    /// Returns true if no queues are tracked.
    pub fn is_empty(&self) -> bool {
        self.queue_states.is_empty()
    }

    /// Returns true if the specified queue index has counters enabled.
    ///
    /// Returns false if the index is out of bounds.
    pub fn is_queue_counter_enabled(&self, index: usize) -> bool {
        self.queue_states.get(index).copied().unwrap_or(false)
    }

    /// Enables counters for a specific queue index.
    ///
    /// Does nothing if the index is out of bounds.
    pub fn enable_queue_counter(&mut self, index: usize) {
        if let Some(state) = self.queue_states.get_mut(index) {
            *state = true;
        }
    }

    /// Disables counters for a specific queue index.
    ///
    /// Does nothing if the index is out of bounds.
    pub fn disable_queue_counter(&mut self, index: usize) {
        if let Some(state) = self.queue_states.get_mut(index) {
            *state = false;
        }
    }

    /// Enables counters for a range of queue indices [start, end].
    ///
    /// Both start and end are inclusive. Out-of-bounds indices are ignored.
    pub fn enable_queue_counters(&mut self, start: usize, end: usize) {
        for index in start..=end {
            self.enable_queue_counter(index);
        }
    }

    /// Returns an iterator over enabled queue indices.
    pub fn enabled_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.queue_states
            .iter()
            .enumerate()
            .filter(|(_, &enabled)| enabled)
            .map(|(idx, _)| idx)
    }

    /// Returns the count of enabled queues.
    pub fn enabled_count(&self) -> usize {
        self.queue_states.iter().filter(|&&s| s).count()
    }
}

/// Tracks enabled counter states for priority groups on a port.
///
/// This is a type-safe replacement for the C++ `FlexCounterPgStates`.
#[derive(Debug, Clone, Default)]
pub struct FlexCounterPgStates {
    /// Per-PG enable states (index = PG index)
    pg_states: Vec<bool>,
}

impl FlexCounterPgStates {
    /// Creates a new state tracker with the given maximum PG count.
    pub fn new(max_pgs: usize) -> Self {
        Self {
            pg_states: vec![false; max_pgs],
        }
    }

    /// Creates a new state tracker with all PGs enabled.
    pub fn all_enabled(max_pgs: usize) -> Self {
        Self {
            pg_states: vec![true; max_pgs],
        }
    }

    /// Returns the number of PGs tracked.
    pub fn len(&self) -> usize {
        self.pg_states.len()
    }

    /// Returns true if no PGs are tracked.
    pub fn is_empty(&self) -> bool {
        self.pg_states.is_empty()
    }

    /// Returns true if the specified PG index has counters enabled.
    ///
    /// Returns false if the index is out of bounds.
    pub fn is_pg_counter_enabled(&self, index: usize) -> bool {
        self.pg_states.get(index).copied().unwrap_or(false)
    }

    /// Enables counters for a specific PG index.
    ///
    /// Does nothing if the index is out of bounds.
    pub fn enable_pg_counter(&mut self, index: usize) {
        if let Some(state) = self.pg_states.get_mut(index) {
            *state = true;
        }
    }

    /// Disables counters for a specific PG index.
    ///
    /// Does nothing if the index is out of bounds.
    pub fn disable_pg_counter(&mut self, index: usize) {
        if let Some(state) = self.pg_states.get_mut(index) {
            *state = false;
        }
    }

    /// Enables counters for a range of PG indices [start, end].
    ///
    /// Both start and end are inclusive. Out-of-bounds indices are ignored.
    pub fn enable_pg_counters(&mut self, start: usize, end: usize) {
        for index in start..=end {
            self.enable_pg_counter(index);
        }
    }

    /// Returns an iterator over enabled PG indices.
    pub fn enabled_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.pg_states
            .iter()
            .enumerate()
            .filter(|(_, &enabled)| enabled)
            .map(|(idx, _)| idx)
    }

    /// Returns the count of enabled PGs.
    pub fn enabled_count(&self) -> usize {
        self.pg_states.iter().filter(|&&s| s).count()
    }
}

/// Special key used when all buffers should be created (not selective).
pub const CREATE_ALL_AVAILABLE_BUFFERS: &str = "create_all_available_buffers";

/// Configuration for queue counters per port.
pub type QueueConfigurations = HashMap<String, FlexCounterQueueStates>;

/// Configuration for PG counters per port.
pub type PgConfigurations = HashMap<String, FlexCounterPgStates>;

/// Parses a range string like "0-7" or "3" into (start, end) inclusive bounds.
///
/// Returns None if the format is invalid.
pub fn parse_index_range(s: &str) -> Option<(usize, usize)> {
    if let Some((start_str, end_str)) = s.split_once('-') {
        let start = start_str.trim().parse().ok()?;
        let end = end_str.trim().parse().ok()?;
        Some((start, end))
    } else {
        let index = s.trim().parse().ok()?;
        Some((index, index))
    }
}

/// Parses a port list like "Ethernet0,Ethernet4,Ethernet8" into individual port names.
pub fn parse_port_list(s: &str) -> Vec<&str> {
    s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_states_new() {
        let states = FlexCounterQueueStates::new(8);
        assert_eq!(states.len(), 8);
        assert!(!states.is_queue_counter_enabled(0));
        assert!(!states.is_queue_counter_enabled(7));
    }

    #[test]
    fn test_queue_states_all_enabled() {
        let states = FlexCounterQueueStates::all_enabled(8);
        assert!(states.is_queue_counter_enabled(0));
        assert!(states.is_queue_counter_enabled(7));
        assert_eq!(states.enabled_count(), 8);
    }

    #[test]
    fn test_queue_states_enable_disable() {
        let mut states = FlexCounterQueueStates::new(8);

        states.enable_queue_counter(3);
        assert!(states.is_queue_counter_enabled(3));

        states.disable_queue_counter(3);
        assert!(!states.is_queue_counter_enabled(3));
    }

    #[test]
    fn test_queue_states_enable_range() {
        let mut states = FlexCounterQueueStates::new(8);

        states.enable_queue_counters(2, 5);

        assert!(!states.is_queue_counter_enabled(0));
        assert!(!states.is_queue_counter_enabled(1));
        assert!(states.is_queue_counter_enabled(2));
        assert!(states.is_queue_counter_enabled(3));
        assert!(states.is_queue_counter_enabled(4));
        assert!(states.is_queue_counter_enabled(5));
        assert!(!states.is_queue_counter_enabled(6));
        assert!(!states.is_queue_counter_enabled(7));
    }

    #[test]
    fn test_queue_states_out_of_bounds() {
        let mut states = FlexCounterQueueStates::new(8);

        // These should not panic, just do nothing
        states.enable_queue_counter(100);
        assert!(!states.is_queue_counter_enabled(100));
    }

    #[test]
    fn test_queue_states_enabled_indices() {
        let mut states = FlexCounterQueueStates::new(8);
        states.enable_queue_counter(1);
        states.enable_queue_counter(3);
        states.enable_queue_counter(5);

        let indices: Vec<_> = states.enabled_indices().collect();
        assert_eq!(indices, vec![1, 3, 5]);
    }

    #[test]
    fn test_pg_states_basic() {
        let mut states = FlexCounterPgStates::new(8);
        assert_eq!(states.len(), 8);
        assert!(!states.is_pg_counter_enabled(0));

        states.enable_pg_counters(0, 7);
        assert_eq!(states.enabled_count(), 8);
    }

    #[test]
    fn test_parse_index_range() {
        assert_eq!(parse_index_range("0-7"), Some((0, 7)));
        assert_eq!(parse_index_range("3"), Some((3, 3)));
        assert_eq!(parse_index_range("0-0"), Some((0, 0)));
        assert_eq!(parse_index_range(" 2 - 5 "), Some((2, 5)));
        assert_eq!(parse_index_range("invalid"), None);
        assert_eq!(parse_index_range("1-abc"), None);
    }

    #[test]
    fn test_parse_port_list() {
        assert_eq!(
            parse_port_list("Ethernet0,Ethernet4,Ethernet8"),
            vec!["Ethernet0", "Ethernet4", "Ethernet8"]
        );
        assert_eq!(parse_port_list("Ethernet0"), vec!["Ethernet0"]);
        assert_eq!(
            parse_port_list(" Ethernet0 , Ethernet4 "),
            vec!["Ethernet0", "Ethernet4"]
        );
        assert!(parse_port_list("").is_empty());
    }
}
