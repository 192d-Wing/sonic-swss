//! Consumer trait and implementations for Redis table consumption.

use std::collections::{BTreeMap, VecDeque};

/// Operation type from Redis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    /// Set operation (add or update)
    Set,
    /// Delete operation
    Del,
}

impl Operation {
    /// Returns true if this is a Set operation.
    pub fn is_set(&self) -> bool {
        matches!(self, Operation::Set)
    }

    /// Returns true if this is a Del operation.
    pub fn is_del(&self) -> bool {
        matches!(self, Operation::Del)
    }
}

/// A field-value pair from a Redis hash entry.
pub type FieldValue = (String, String);

/// Key, operation, and field-values tuple from Redis.
///
/// This is the fundamental unit of data consumed from Redis tables.
#[derive(Debug, Clone)]
pub struct KeyOpFieldsValues {
    /// The key (e.g., "Ethernet0", "10.0.0.0/24")
    pub key: String,
    /// The operation (Set or Del)
    pub op: Operation,
    /// Field-value pairs (empty for Del operations)
    pub fvs: Vec<FieldValue>,
}

impl KeyOpFieldsValues {
    /// Creates a new entry.
    pub fn new(key: impl Into<String>, op: Operation, fvs: Vec<FieldValue>) -> Self {
        Self {
            key: key.into(),
            op,
            fvs,
        }
    }

    /// Creates a Set entry.
    pub fn set(key: impl Into<String>, fvs: Vec<FieldValue>) -> Self {
        Self::new(key, Operation::Set, fvs)
    }

    /// Creates a Del entry.
    pub fn del(key: impl Into<String>) -> Self {
        Self::new(key, Operation::Del, vec![])
    }

    /// Returns the value for a field, if present.
    pub fn get_field(&self, field: &str) -> Option<&str> {
        self.fvs
            .iter()
            .find(|(f, _)| f == field)
            .map(|(_, v)| v.as_str())
    }

    /// Returns true if this entry has the given field.
    pub fn has_field(&self, field: &str) -> bool {
        self.fvs.iter().any(|(f, _)| f == field)
    }
}

/// Configuration for a Consumer.
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Table name (e.g., "PORT_TABLE", "ROUTE_TABLE")
    pub table_name: String,
    /// Priority (lower = higher priority)
    pub priority: i32,
    /// Pop batch size
    pub batch_size: usize,
}

impl ConsumerConfig {
    /// Creates a new consumer config.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            priority: 0,
            batch_size: 128,
        }
    }

    /// Sets the priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the batch size.
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }
}

/// Consumer for Redis table entries.
///
/// A Consumer reads entries from a Redis table and provides them
/// to an Orch for processing. It handles:
///
/// - Batched reading from Redis
/// - Deduplication of operations on the same key
/// - Retry queue for failed operations
///
/// # Deduplication Logic
///
/// When multiple operations arrive for the same key:
/// - Multiple DEL: Keep only the latest
/// - Multiple SET: Merge field-values (newer overwrites older)
/// - DEL then SET: Keep both (maintain ordering)
pub struct Consumer {
    config: ConsumerConfig,
    /// Pending tasks indexed by key for deduplication
    to_sync: BTreeMap<String, VecDeque<KeyOpFieldsValues>>,
    /// Total count of pending entries
    pending_count: usize,
}

impl Consumer {
    /// Creates a new consumer with the given configuration.
    pub fn new(config: ConsumerConfig) -> Self {
        Self {
            config,
            to_sync: BTreeMap::new(),
            pending_count: 0,
        }
    }

    /// Returns the table name.
    pub fn table_name(&self) -> &str {
        &self.config.table_name
    }

    /// Returns the priority.
    pub fn priority(&self) -> i32 {
        self.config.priority
    }

    /// Returns true if there are pending entries.
    pub fn has_pending(&self) -> bool {
        self.pending_count > 0
    }

    /// Returns the number of pending entries.
    pub fn pending_count(&self) -> usize {
        self.pending_count
    }

    /// Adds entries to the sync queue with deduplication.
    ///
    /// This implements the C++ merging logic safely:
    /// - For same-key operations, newer SET values override older ones
    /// - DEL operations clear pending SETs for the same key
    pub fn add_to_sync(&mut self, entries: Vec<KeyOpFieldsValues>) {
        for entry in entries {
            self.add_single_entry(entry);
        }
    }

    fn add_single_entry(&mut self, entry: KeyOpFieldsValues) {
        let queue = self.to_sync.entry(entry.key.clone()).or_default();

        match entry.op {
            Operation::Del => {
                // DEL clears any pending SETs and replaces with DEL
                if !queue.is_empty() {
                    self.pending_count -= queue.len();
                    queue.clear();
                }
                queue.push_back(entry);
                self.pending_count += 1;
            }
            Operation::Set => {
                // SET merges with existing SET or appends
                if let Some(last) = queue.back_mut() {
                    if last.op == Operation::Set {
                        // Merge: newer values override
                        for (field, value) in entry.fvs {
                            if let Some(existing) = last.fvs.iter_mut().find(|(f, _)| *f == field) {
                                existing.1 = value;
                            } else {
                                last.fvs.push((field, value));
                            }
                        }
                        // Don't increment count - we merged
                        return;
                    }
                }
                // Either empty queue or last was DEL - append SET
                queue.push_back(entry);
                self.pending_count += 1;
            }
        }
    }

    /// Drains all pending entries in order.
    ///
    /// Returns entries grouped by key, maintaining operation order.
    pub fn drain(&mut self) -> Vec<KeyOpFieldsValues> {
        let mut result = Vec::with_capacity(self.pending_count);

        for (_key, mut queue) in std::mem::take(&mut self.to_sync) {
            while let Some(entry) = queue.pop_front() {
                result.push(entry);
            }
        }

        self.pending_count = 0;
        result
    }

    /// Peeks at pending entries without removing them.
    pub fn peek(&self) -> impl Iterator<Item = &KeyOpFieldsValues> {
        self.to_sync.values().flat_map(|q| q.iter())
    }

    /// Re-adds an entry to the retry queue.
    ///
    /// Use this when an entry failed processing but should be retried.
    pub fn retry(&mut self, entry: KeyOpFieldsValues) {
        // Add to front of queue for this key
        let queue = self.to_sync.entry(entry.key.clone()).or_default();
        queue.push_front(entry);
        self.pending_count += 1;
    }

    /// Clears all pending entries.
    pub fn clear(&mut self) {
        self.to_sync.clear();
        self.pending_count = 0;
    }

    /// Dumps pending entries for debugging.
    pub fn dump(&self) -> Vec<String> {
        self.to_sync
            .iter()
            .flat_map(|(key, queue)| {
                queue.iter().map(move |e| {
                    format!("{}: {} {:?}", key, if e.op.is_set() { "SET" } else { "DEL" }, e.fvs)
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_op_fields_values() {
        let entry = KeyOpFieldsValues::set(
            "Ethernet0",
            vec![("speed".to_string(), "100000".to_string())],
        );

        assert_eq!(entry.key, "Ethernet0");
        assert!(entry.op.is_set());
        assert_eq!(entry.get_field("speed"), Some("100000"));
        assert!(entry.has_field("speed"));
        assert!(!entry.has_field("mtu"));
    }

    #[test]
    fn test_consumer_basic() {
        let config = ConsumerConfig::new("PORT_TABLE");
        let mut consumer = Consumer::new(config);

        assert_eq!(consumer.table_name(), "PORT_TABLE");
        assert!(!consumer.has_pending());

        consumer.add_to_sync(vec![
            KeyOpFieldsValues::set("Ethernet0", vec![("speed".to_string(), "100000".to_string())]),
        ]);

        assert!(consumer.has_pending());
        assert_eq!(consumer.pending_count(), 1);
    }

    #[test]
    fn test_consumer_set_merge() {
        let config = ConsumerConfig::new("PORT_TABLE");
        let mut consumer = Consumer::new(config);

        // First SET
        consumer.add_to_sync(vec![KeyOpFieldsValues::set(
            "Ethernet0",
            vec![("speed".to_string(), "100000".to_string())],
        )]);

        // Second SET for same key - should merge
        consumer.add_to_sync(vec![KeyOpFieldsValues::set(
            "Ethernet0",
            vec![
                ("speed".to_string(), "40000".to_string()),  // Override
                ("mtu".to_string(), "9000".to_string()),      // New field
            ],
        )]);

        // Should still be 1 entry (merged)
        assert_eq!(consumer.pending_count(), 1);

        let entries = consumer.drain();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].get_field("speed"), Some("40000"));
        assert_eq!(entries[0].get_field("mtu"), Some("9000"));
    }

    #[test]
    fn test_consumer_del_clears_set() {
        let config = ConsumerConfig::new("PORT_TABLE");
        let mut consumer = Consumer::new(config);

        // SET then DEL
        consumer.add_to_sync(vec![KeyOpFieldsValues::set(
            "Ethernet0",
            vec![("speed".to_string(), "100000".to_string())],
        )]);
        consumer.add_to_sync(vec![KeyOpFieldsValues::del("Ethernet0")]);

        // Should be 1 entry (DEL replaced SET)
        assert_eq!(consumer.pending_count(), 1);

        let entries = consumer.drain();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].op.is_del());
    }

    #[test]
    fn test_consumer_del_then_set() {
        let config = ConsumerConfig::new("PORT_TABLE");
        let mut consumer = Consumer::new(config);

        // DEL then SET
        consumer.add_to_sync(vec![KeyOpFieldsValues::del("Ethernet0")]);
        consumer.add_to_sync(vec![KeyOpFieldsValues::set(
            "Ethernet0",
            vec![("speed".to_string(), "100000".to_string())],
        )]);

        // Should be 2 entries (DEL followed by SET)
        assert_eq!(consumer.pending_count(), 2);

        let entries = consumer.drain();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].op.is_del());
        assert!(entries[1].op.is_set());
    }

    #[test]
    fn test_consumer_retry() {
        let config = ConsumerConfig::new("PORT_TABLE");
        let mut consumer = Consumer::new(config);

        let entry = KeyOpFieldsValues::set("Ethernet0", vec![]);
        consumer.retry(entry);

        assert_eq!(consumer.pending_count(), 1);
    }
}
