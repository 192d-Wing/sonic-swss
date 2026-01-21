//! Retry cache for task dependency tracking.
//!
//! The retry cache tracks tasks that failed due to unmet dependencies
//! and allows them to be retried when the dependency is satisfied.

use std::collections::{HashMap, HashSet};

/// A constraint representing a dependency on another table/key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Constraint {
    /// The table containing the dependency
    pub table: String,
    /// The key within the table
    pub key: String,
}

impl Constraint {
    /// Creates a new constraint.
    pub fn new(table: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            key: key.into(),
        }
    }

    /// Creates a constraint from a "table:key" string.
    pub fn from_str(s: &str) -> Option<Self> {
        let (table, key) = s.split_once(':')?;
        Some(Self::new(table, key))
    }
}

impl std::fmt::Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.table, self.key)
    }
}

/// Entry in the retry cache.
#[derive(Debug, Clone)]
pub struct RetryEntry<T> {
    /// The task data to retry
    pub data: T,
    /// Constraints that must be satisfied before retry
    pub constraints: HashSet<Constraint>,
}

impl<T> RetryEntry<T> {
    /// Creates a new retry entry.
    pub fn new(data: T, constraints: impl IntoIterator<Item = Constraint>) -> Self {
        Self {
            data,
            constraints: constraints.into_iter().collect(),
        }
    }

    /// Returns true if all constraints are satisfied.
    pub fn is_ready(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Removes a constraint (called when dependency is satisfied).
    pub fn satisfy(&mut self, constraint: &Constraint) -> bool {
        self.constraints.remove(constraint)
    }
}

/// Cache for tasks waiting on dependencies.
///
/// Tasks that fail due to unmet dependencies (e.g., route waiting for
/// interface) are stored here with their constraints. When the dependency
/// is satisfied, the constraint is removed and the task can be retried.
#[derive(Debug)]
pub struct RetryCache<K, T> {
    /// Tasks indexed by their key
    entries: HashMap<K, RetryEntry<T>>,
    /// Reverse index: constraint -> keys waiting on it
    waiters: HashMap<Constraint, HashSet<K>>,
}

impl<K, T> RetryCache<K, T>
where
    K: Eq + std::hash::Hash + Clone,
{
    /// Creates a new empty retry cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            waiters: HashMap::new(),
        }
    }

    /// Returns the number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds a task to the retry cache with its constraints.
    pub fn add(&mut self, key: K, data: T, constraints: impl IntoIterator<Item = Constraint>) {
        let entry = RetryEntry::new(data, constraints);

        // Update reverse index
        for constraint in &entry.constraints {
            self.waiters
                .entry(constraint.clone())
                .or_default()
                .insert(key.clone());
        }

        self.entries.insert(key, entry);
    }

    /// Removes a task from the cache.
    pub fn remove(&mut self, key: &K) -> Option<T> {
        if let Some(entry) = self.entries.remove(key) {
            // Clean up reverse index
            for constraint in &entry.constraints {
                if let Some(waiters) = self.waiters.get_mut(constraint) {
                    waiters.remove(key);
                    if waiters.is_empty() {
                        self.waiters.remove(constraint);
                    }
                }
            }
            Some(entry.data)
        } else {
            None
        }
    }

    /// Notifies the cache that a constraint has been satisfied.
    ///
    /// Returns the keys of tasks that are now ready to retry.
    pub fn satisfy(&mut self, constraint: &Constraint) -> Vec<K> {
        let mut ready = Vec::new();

        if let Some(waiting_keys) = self.waiters.remove(constraint) {
            for key in waiting_keys {
                if let Some(entry) = self.entries.get_mut(&key) {
                    entry.satisfy(constraint);
                    if entry.is_ready() {
                        ready.push(key);
                    }
                }
            }
        }

        ready
    }

    /// Returns all tasks that are ready to retry (no constraints).
    pub fn drain_ready(&mut self) -> Vec<(K, T)> {
        let ready_keys: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, e)| e.is_ready())
            .map(|(k, _)| k.clone())
            .collect();

        ready_keys
            .into_iter()
            .filter_map(|k| self.remove(&k).map(|data| (k, data)))
            .collect()
    }

    /// Returns true if the cache contains the given key.
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns the constraints for a given key.
    pub fn constraints(&self, key: &K) -> Option<&HashSet<Constraint>> {
        self.entries.get(key).map(|e| &e.constraints)
    }

    /// Clears all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.waiters.clear();
    }
}

impl<K, T> Default for RetryCache<K, T>
where
    K: Eq + std::hash::Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint() {
        let c = Constraint::new("PORT_TABLE", "Ethernet0");
        assert_eq!(c.table, "PORT_TABLE");
        assert_eq!(c.key, "Ethernet0");
        assert_eq!(c.to_string(), "PORT_TABLE:Ethernet0");

        let c2 = Constraint::from_str("PORT_TABLE:Ethernet0").unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn test_retry_cache_basic() {
        let mut cache: RetryCache<String, String> = RetryCache::new();

        assert!(cache.is_empty());

        cache.add(
            "route1".to_string(),
            "10.0.0.0/24".to_string(),
            vec![Constraint::new("INTF_TABLE", "Ethernet0")],
        );

        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&"route1".to_string()));
    }

    #[test]
    fn test_satisfy_constraint() {
        let mut cache: RetryCache<String, String> = RetryCache::new();

        // Add task with one constraint
        cache.add(
            "route1".to_string(),
            "10.0.0.0/24".to_string(),
            vec![Constraint::new("INTF_TABLE", "Ethernet0")],
        );

        // Task not ready yet
        assert!(cache.drain_ready().is_empty());

        // Satisfy constraint
        let ready = cache.satisfy(&Constraint::new("INTF_TABLE", "Ethernet0"));
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], "route1");

        // Now ready
        let ready_tasks = cache.drain_ready();
        assert_eq!(ready_tasks.len(), 1);
        assert_eq!(ready_tasks[0].0, "route1");
        assert_eq!(ready_tasks[0].1, "10.0.0.0/24");
    }

    #[test]
    fn test_multiple_constraints() {
        let mut cache: RetryCache<String, String> = RetryCache::new();

        // Add task with two constraints
        cache.add(
            "route1".to_string(),
            "10.0.0.0/24".to_string(),
            vec![
                Constraint::new("INTF_TABLE", "Ethernet0"),
                Constraint::new("NEIGH_TABLE", "192.168.1.1"),
            ],
        );

        // Satisfy first constraint
        let ready = cache.satisfy(&Constraint::new("INTF_TABLE", "Ethernet0"));
        assert!(ready.is_empty()); // Still waiting on second

        // Satisfy second constraint
        let ready = cache.satisfy(&Constraint::new("NEIGH_TABLE", "192.168.1.1"));
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut cache: RetryCache<String, String> = RetryCache::new();

        cache.add(
            "route1".to_string(),
            "10.0.0.0/24".to_string(),
            vec![Constraint::new("INTF_TABLE", "Ethernet0")],
        );

        let removed = cache.remove(&"route1".to_string());
        assert_eq!(removed, Some("10.0.0.0/24".to_string()));
        assert!(cache.is_empty());
    }
}
