//! Distributed locking for HA coordination
//!
//! Provides distributed locks backed by Redis for cluster-wide coordination.
//! Implements lease-based locking with automatic renewal for fault tolerance.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// Lock lease configuration
#[derive(Debug, Clone)]
pub struct LeaseConfig {
    /// Lock TTL in seconds
    pub ttl_secs: u64,
    /// Renewal interval in seconds (should be < ttl_secs / 2)
    pub renewal_interval_secs: u64,
    /// Max retries for acquiring lock
    pub max_retries: u32,
    /// Retry backoff in milliseconds
    pub retry_backoff_ms: u64,
}

impl Default for LeaseConfig {
    fn default() -> Self {
        Self {
            ttl_secs: 30,
            renewal_interval_secs: 10,
            max_retries: 5,
            retry_backoff_ms: 100,
        }
    }
}

/// Lock holder information
#[derive(Debug, Clone)]
pub struct LockHolder {
    /// Unique lock token
    pub token: String,
    /// Owner instance ID
    pub owner: String,
    /// Acquisition timestamp
    pub acquired_at: u64,
    /// Lease expiry timestamp
    pub expires_at: u64,
}

impl LockHolder {
    /// Create new lock holder
    pub fn new(token: String, owner: String, ttl_secs: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            token,
            owner,
            acquired_at: now,
            expires_at: now + ttl_secs,
        }
    }

    /// Check if lock is still valid
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now < self.expires_at
    }

    /// Get remaining TTL in seconds
    pub fn remaining_ttl(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.expires_at.saturating_sub(now)
    }
}

/// Distributed lock - stateless lock information
///
/// This lock does not hold Redis connections. Instead, it tracks lock state
/// and the caller is responsible for executing Redis operations via RedisAdapter.
#[derive(Clone)]
pub struct DistributedLock {
    /// Lock name/key
    lock_name: String,
    /// Instance ID of the lock holder
    owner_id: String,
    /// Lease configuration
    config: LeaseConfig,
    /// Current lock holder (if held)
    holder: Arc<parking_lot::Mutex<Option<LockHolder>>>,
    /// Lock state
    is_locked: Arc<AtomicBool>,
    /// Lock acquisition count
    acquisition_count: Arc<AtomicU64>,
}

impl DistributedLock {
    /// Create new distributed lock
    pub fn new(lock_name: String, owner_id: String, config: LeaseConfig) -> Self {
        Self {
            lock_name,
            owner_id,
            config,
            holder: Arc::new(parking_lot::Mutex::new(None)),
            is_locked: Arc::new(AtomicBool::new(false)),
            acquisition_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record successful lock acquisition (called after Redis SET NX succeeds)
    pub fn record_acquisition(&self, token: String) {
        let holder = LockHolder::new(token, self.owner_id.clone(), self.config.ttl_secs);
        *self.holder.lock() = Some(holder);
        self.is_locked.store(true, Ordering::SeqCst);
        self.acquisition_count.fetch_add(1, Ordering::SeqCst);

        info!(
            lock = %self.lock_name,
            owner = %self.owner_id,
            ttl = self.config.ttl_secs,
            "Acquired distributed lock"
        );
    }

    /// Record lock release (called after Redis DEL succeeds)
    pub fn record_release(&self) {
        *self.holder.lock() = None;
        self.is_locked.store(false, Ordering::SeqCst);

        info!(
            lock = %self.lock_name,
            owner = %self.owner_id,
            "Released distributed lock"
        );
    }

    /// Check if lock is currently held
    pub fn is_locked(&self) -> bool {
        self.is_locked.load(Ordering::SeqCst)
    }

    /// Get lock holder information
    pub fn get_holder(&self) -> Option<LockHolder> {
        self.holder.lock().clone()
    }

    /// Get lock name
    pub fn lock_name(&self) -> &str {
        &self.lock_name
    }

    /// Get lock owner ID
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    /// Get lock configuration
    pub fn config(&self) -> &LeaseConfig {
        &self.config
    }

    /// Get lock acquisition count
    pub fn acquisition_count(&self) -> u64 {
        self.acquisition_count.load(Ordering::SeqCst)
    }

    /// Get Redis key for this lock
    pub fn redis_key(&self) -> String {
        format!("lock:{}", self.lock_name)
    }
}

/// Lock manager for coordinating multiple locks
pub struct LockManager {
    /// Instance ID
    instance_id: String,
    /// Lease configuration
    config: LeaseConfig,
    /// Active locks
    locks: Arc<parking_lot::Mutex<std::collections::HashMap<String, Arc<DistributedLock>>>>,
}

impl LockManager {
    /// Create new lock manager
    pub fn new(instance_id: String, config: LeaseConfig) -> Self {
        Self {
            instance_id,
            config,
            locks: Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Get or create a lock
    pub fn get_lock(&self, lock_name: String) -> Arc<DistributedLock> {
        let mut locks = self.locks.lock();
        locks
            .entry(lock_name)
            .or_insert_with_key(|name| {
                Arc::new(DistributedLock::new(
                    name.clone(),
                    self.instance_id.clone(),
                    self.config.clone(),
                ))
            })
            .clone()
    }

    /// Get all active locks
    pub fn get_all_locks(&self) -> Vec<String> {
        let locks = self.locks.lock();
        locks.keys().cloned().collect()
    }

    /// Get locks that are currently acquired
    pub fn get_acquired_locks(&self) -> Vec<Arc<DistributedLock>> {
        let locks = self.locks.lock();
        locks
            .values()
            .filter(|lock| lock.is_locked())
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lease_config_default() {
        let config = LeaseConfig::default();
        assert_eq!(config.ttl_secs, 30);
        assert_eq!(config.renewal_interval_secs, 10);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_lock_holder_creation() {
        let holder = LockHolder::new("token123".to_string(), "owner1".to_string(), 30);
        assert_eq!(holder.token, "token123");
        assert_eq!(holder.owner, "owner1");
        assert!(holder.is_valid());
    }

    #[test]
    fn test_lock_holder_ttl() {
        let holder = LockHolder::new("token123".to_string(), "owner1".to_string(), 30);
        let ttl = holder.remaining_ttl();
        assert!(ttl > 0 && ttl <= 30);
    }

    #[test]
    fn test_distributed_lock_creation() {
        let config = LeaseConfig::default();
        let lock = DistributedLock::new("my_lock".to_string(), "owner1".to_string(), config);

        assert_eq!(lock.lock_name(), "my_lock");
        assert_eq!(lock.owner_id(), "owner1");
        assert!(!lock.is_locked());
        assert_eq!(lock.acquisition_count(), 0);
    }

    #[test]
    fn test_lock_acquisition() {
        let config = LeaseConfig::default();
        let lock = DistributedLock::new("test_lock".to_string(), "owner1".to_string(), config);

        let token = "token_abc_123".to_string();
        lock.record_acquisition(token.clone());

        assert!(lock.is_locked());
        assert_eq!(lock.acquisition_count(), 1);

        let holder = lock.get_holder();
        assert!(holder.is_some());
        assert_eq!(holder.unwrap().token, token);
    }

    #[test]
    fn test_lock_release() {
        let config = LeaseConfig::default();
        let lock = DistributedLock::new("test_lock".to_string(), "owner1".to_string(), config);

        lock.record_acquisition("token_123".to_string());
        assert!(lock.is_locked());

        lock.record_release();
        assert!(!lock.is_locked());
        assert!(lock.get_holder().is_none());
    }

    #[test]
    fn test_lock_manager_creation() {
        let manager = LockManager::new("node1".to_string(), LeaseConfig::default());
        assert_eq!(manager.get_all_locks().len(), 0);
    }

    #[test]
    fn test_lock_manager_get_lock() {
        let manager = LockManager::new("node1".to_string(), LeaseConfig::default());
        let lock1 = manager.get_lock("lock1".to_string());
        let lock2 = manager.get_lock("lock1".to_string());

        // Should return same lock instance
        assert_eq!(Arc::as_ptr(&lock1), Arc::as_ptr(&lock2));
    }

    #[test]
    fn test_lock_redis_key() {
        let config = LeaseConfig::default();
        let lock = DistributedLock::new("my_lock".to_string(), "owner1".to_string(), config);
        assert_eq!(lock.redis_key(), "lock:my_lock");
    }

    #[test]
    fn test_get_acquired_locks() {
        let manager = LockManager::new("node1".to_string(), LeaseConfig::default());

        let lock1 = manager.get_lock("lock1".to_string());
        let lock2 = manager.get_lock("lock2".to_string());
        let _lock3 = manager.get_lock("lock3".to_string());

        lock1.record_acquisition("token1".to_string());
        lock2.record_acquisition("token2".to_string());

        let acquired = manager.get_acquired_locks();
        assert_eq!(acquired.len(), 2);
    }
}
