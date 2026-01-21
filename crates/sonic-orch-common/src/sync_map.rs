//! Type-safe map wrapper that prevents auto-vivification bugs.
//!
//! This module provides a `SyncMap` type that is a safer alternative to
//! `std::collections::HashMap` for use in orchestration code. It prevents
//! the common C++ bug of accidentally creating map entries when accessing
//! non-existent keys.
//!
//! # The Problem
//!
//! In C++, `map[key].ref_count++` will create a default-constructed entry
//! if `key` doesn't exist. This can lead to subtle bugs where reference
//! counts become incorrect.
//!
//! # The Solution
//!
//! `SyncMap` provides explicit methods that never auto-create entries:
//! - `get()` returns `Option<&V>`
//! - `get_mut()` returns `Option<&mut V>`
//! - `increment_ref()` returns `Result<u32, Error>`

use std::collections::HashMap;
use std::hash::Hash;
use thiserror::Error;

/// Error type for SyncMap operations.
#[derive(Debug, Clone, Error)]
pub enum SyncMapError {
    #[error("Key not found")]
    KeyNotFound,

    #[error("Reference count underflow")]
    RefCountUnderflow,
}

/// Trait for types that have a reference count.
pub trait HasRefCount {
    /// Increments the reference count and returns the new value.
    fn increment_ref(&mut self) -> u32;

    /// Decrements the reference count and returns the new value.
    ///
    /// Returns `None` if the count would underflow.
    fn decrement_ref(&mut self) -> Option<u32>;

    /// Returns the current reference count.
    fn ref_count(&self) -> u32;
}

/// A type-safe map wrapper that prevents auto-vivification bugs.
///
/// Unlike `HashMap`, this type never creates entries implicitly.
/// All operations that might create entries are explicit.
///
/// # Example
///
/// ```
/// use sonic_orch_common::SyncMap;
///
/// let mut map: SyncMap<String, i32> = SyncMap::new();
///
/// // get() returns None for missing keys (doesn't create entry)
/// assert!(map.get(&"missing".to_string()).is_none());
///
/// // Must explicitly insert
/// map.insert("key".to_string(), 42);
/// assert_eq!(map.get(&"key".to_string()), Some(&42));
/// ```
#[derive(Debug, Clone)]
pub struct SyncMap<K, V> {
    inner: HashMap<K, V>,
}

impl<K, V> SyncMap<K, V>
where
    K: Eq + Hash,
{
    /// Creates a new empty map.
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Creates a new map with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: HashMap::with_capacity(capacity),
        }
    }

    /// Returns the number of entries in the map.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns true if the map contains the given key.
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// Returns a reference to the value for the given key.
    ///
    /// Returns `None` if the key is not present.
    /// **This never creates entries.**
    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner.get(key)
    }

    /// Returns a mutable reference to the value for the given key.
    ///
    /// Returns `None` if the key is not present.
    /// **This never creates entries.**
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.inner.get_mut(key)
    }

    /// Inserts a key-value pair into the map.
    ///
    /// Returns the old value if the key was already present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// Removes a key from the map.
    ///
    /// Returns the removed value if the key was present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }

    /// Clears all entries from the map.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns an iterator over key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.inner.iter()
    }

    /// Returns an iterator over keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.inner.keys()
    }

    /// Returns an iterator over values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.inner.values()
    }

    /// Returns a mutable iterator over values.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.inner.values_mut()
    }

    /// Gets the value for a key, or inserts a default value if not present.
    ///
    /// Unlike `get()`, this method **will** create an entry if the key
    /// is not present. Use this when you explicitly want this behavior.
    pub fn get_or_insert_with<F>(&mut self, key: K, f: F) -> &mut V
    where
        F: FnOnce() -> V,
    {
        self.inner.entry(key).or_insert_with(f)
    }

    /// Gets the value for a key, or inserts a default value if not present.
    pub fn get_or_insert(&mut self, key: K, value: V) -> &mut V {
        self.inner.entry(key).or_insert(value)
    }
}

impl<K, V> SyncMap<K, V>
where
    K: Eq + Hash,
    V: HasRefCount,
{
    /// Increments the reference count for the given key.
    ///
    /// Returns the new reference count, or an error if the key is not found.
    ///
    /// **This never creates entries.** This is the safe replacement for
    /// the C++ pattern `map[key].ref_count++`.
    pub fn increment_ref(&mut self, key: &K) -> Result<u32, SyncMapError> {
        match self.inner.get_mut(key) {
            Some(entry) => Ok(entry.increment_ref()),
            None => Err(SyncMapError::KeyNotFound),
        }
    }

    /// Decrements the reference count for the given key.
    ///
    /// Returns the new reference count, or an error if the key is not found
    /// or the count would underflow.
    pub fn decrement_ref(&mut self, key: &K) -> Result<u32, SyncMapError> {
        match self.inner.get_mut(key) {
            Some(entry) => entry
                .decrement_ref()
                .ok_or(SyncMapError::RefCountUnderflow),
            None => Err(SyncMapError::KeyNotFound),
        }
    }

    /// Returns the reference count for the given key.
    ///
    /// Returns `None` if the key is not found.
    pub fn ref_count(&self, key: &K) -> Option<u32> {
        self.inner.get(key).map(|e| e.ref_count())
    }
}

impl<K, V> Default for SyncMap<K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> FromIterator<(K, V)> for SyncMap<K, V>
where
    K: Eq + Hash,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct RefCountedValue {
        data: String,
        ref_count: u32,
    }

    impl RefCountedValue {
        fn new(data: &str) -> Self {
            Self {
                data: data.to_string(),
                ref_count: 0,
            }
        }
    }

    impl HasRefCount for RefCountedValue {
        fn increment_ref(&mut self) -> u32 {
            self.ref_count += 1;
            self.ref_count
        }

        fn decrement_ref(&mut self) -> Option<u32> {
            if self.ref_count == 0 {
                None
            } else {
                self.ref_count -= 1;
                Some(self.ref_count)
            }
        }

        fn ref_count(&self) -> u32 {
            self.ref_count
        }
    }

    #[test]
    fn test_basic_operations() {
        let mut map: SyncMap<String, i32> = SyncMap::new();

        assert!(map.is_empty());
        assert!(map.get(&"key".to_string()).is_none());

        map.insert("key".to_string(), 42);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&"key".to_string()), Some(&42));

        map.remove(&"key".to_string());
        assert!(map.is_empty());
    }

    #[test]
    fn test_get_never_creates() {
        let mut map: SyncMap<String, i32> = SyncMap::new();

        // get() should return None and NOT create an entry
        assert!(map.get(&"missing".to_string()).is_none());
        assert!(map.is_empty());

        // get_mut() should also not create
        assert!(map.get_mut(&"missing".to_string()).is_none());
        assert!(map.is_empty());
    }

    #[test]
    fn test_increment_ref_requires_existing_key() {
        let mut map: SyncMap<String, RefCountedValue> = SyncMap::new();

        // Should fail for missing key
        assert!(map.increment_ref(&"missing".to_string()).is_err());

        // Should succeed for existing key
        map.insert("key".to_string(), RefCountedValue::new("test"));
        assert_eq!(map.increment_ref(&"key".to_string()).unwrap(), 1);
        assert_eq!(map.increment_ref(&"key".to_string()).unwrap(), 2);
    }

    #[test]
    fn test_decrement_ref_underflow_protection() {
        let mut map: SyncMap<String, RefCountedValue> = SyncMap::new();
        map.insert("key".to_string(), RefCountedValue::new("test"));

        // Should fail - ref_count is 0
        assert!(map.decrement_ref(&"key".to_string()).is_err());

        // After increment, decrement should work
        map.increment_ref(&"key".to_string()).unwrap();
        assert_eq!(map.decrement_ref(&"key".to_string()).unwrap(), 0);

        // Second decrement should fail again
        assert!(map.decrement_ref(&"key".to_string()).is_err());
    }

    #[test]
    fn test_get_or_insert() {
        let mut map: SyncMap<String, i32> = SyncMap::new();

        // This explicitly creates the entry
        let value = map.get_or_insert("key".to_string(), 42);
        assert_eq!(*value, 42);
        assert_eq!(map.len(), 1);

        // Second call returns existing value
        let value = map.get_or_insert("key".to_string(), 100);
        assert_eq!(*value, 42); // Not 100
    }
}
