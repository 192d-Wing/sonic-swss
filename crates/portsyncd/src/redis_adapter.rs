//! Redis adapter for production database access
//!
//! This module provides a production-ready Redis backend for the portsyncd daemon,
//! replacing the mock DatabaseConnection for real SWSS database operations.
//! Uses real redis client in production, mock storage in tests.

use crate::error::Result;
use std::collections::HashMap;

/// Redis-backed database connection for production
///
/// Provides unified interface for both real Redis (production) and mock storage (testing)
/// via conditional compilation. In tests, uses Arc<Mutex<HashMap>>, in production uses
/// redis client with connection pooling.
#[derive(Clone)]
pub struct RedisAdapter {
    /// Database name for logging and debugging
    db_name: String,
    /// Connection parameters
    host: String,
    port: u16,
    db_number: u32,

    /// Mock data storage (for testing only)
    #[cfg(test)]
    data: std::sync::Arc<tokio::sync::Mutex<HashMap<String, HashMap<String, String>>>>,

    /// Real Redis connection (production only)
    #[cfg(not(test))]
    connection: std::sync::Arc<tokio::sync::Mutex<Option<redis::aio::ConnectionManager>>>,
}

impl std::fmt::Debug for RedisAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisAdapter")
            .field("db_name", &self.db_name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("db_number", &self.db_number)
            .finish()
    }
}

impl RedisAdapter {
    /// Create a new Redis adapter
    pub fn new(
        db_name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        db_number: u32,
    ) -> Self {
        Self {
            db_name: db_name.into(),
            host: host.into(),
            port,
            db_number,
            #[cfg(test)]
            data: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            #[cfg(not(test))]
            connection: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create adapter for CONFIG_DB (db_number=4)
    pub fn config_db(host: impl Into<String>, port: u16) -> Self {
        Self::new("CONFIG_DB", host, port, 4)
    }

    /// Create adapter for APP_DB (db_number=0)
    pub fn app_db(host: impl Into<String>, port: u16) -> Self {
        Self::new("APP_DB", host, port, 0)
    }

    /// Create adapter for STATE_DB (db_number=6)
    pub fn state_db(host: impl Into<String>, port: u16) -> Self {
        Self::new("STATE_DB", host, port, 6)
    }

    /// Connect to Redis server (production mode)
    #[cfg(not(test))]
    pub async fn connect(&mut self) -> Result<()> {
        let redis_url = format!("redis://{}:{}/{}", self.host, self.port, self.db_number);
        let client = redis::Client::open(redis_url.as_str()).map_err(|e| {
            crate::error::PortsyncError::Database(format!("Failed to create Redis client: {}", e))
        })?;

        let connection_manager = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| {
                crate::error::PortsyncError::Database(format!("Failed to connect to Redis: {}", e))
            })?;

        let mut conn = self.connection.lock().await;
        *conn = Some(connection_manager);

        eprintln!(
            "portsyncd: Connected to Redis ({}:{})",
            self.host, self.port
        );
        Ok(())
    }

    /// Connect to Redis server (test mode - no-op)
    #[cfg(test)]
    pub async fn connect(&mut self) -> Result<()> {
        // No-op in test mode
        Ok(())
    }

    /// Get hash values from database
    pub async fn hgetall(&self, _key: &str) -> Result<HashMap<String, String>> {
        #[cfg(test)]
        {
            let key = _key;
            let data = self.data.lock().await;
            Ok(data.get(key).cloned().unwrap_or_default())
        }

        #[cfg(not(test))]
        {
            let key = _key;
            let conn_ref = self.connection.lock().await;
            let mut conn = conn_ref.clone().ok_or_else(|| {
                crate::error::PortsyncError::Database("Not connected to Redis".to_string())
            })?;

            redis::AsyncCommands::hgetall(&mut conn, key)
                .await
                .map_err(|e| {
                    crate::error::PortsyncError::Database(format!("HGETALL failed: {}", e))
                })
        }
    }

    /// Set hash field values in database
    pub async fn hset(&mut self, _key: &str, _fields: &[(String, String)]) -> Result<()> {
        #[cfg(test)]
        {
            let key = _key;
            let fields = _fields;
            let mut data = self.data.lock().await;
            let entry = data.entry(key.to_string()).or_default();
            for (field, value) in fields {
                entry.insert(field.clone(), value.clone());
            }
            Ok(())
        }

        #[cfg(not(test))]
        {
            let key = _key;
            let fields = _fields;
            let conn_ref = self.connection.lock().await;
            let mut conn = conn_ref.clone().ok_or_else(|| {
                crate::error::PortsyncError::Database("Not connected to Redis".to_string())
            })?;

            for (field, value) in fields {
                let _: () =
                    redis::AsyncCommands::hset(&mut conn, key, field.as_str(), value.as_str())
                        .await
                        .map_err(|e| {
                            crate::error::PortsyncError::Database(format!("HSET failed: {}", e))
                        })?;
            }
            Ok(())
        }
    }

    /// Delete key from database
    pub async fn delete(&mut self, _key: &str) -> Result<()> {
        #[cfg(test)]
        {
            let key = _key;
            let mut data = self.data.lock().await;
            data.remove(key);
            Ok(())
        }

        #[cfg(not(test))]
        {
            let key = _key;
            let conn_ref = self.connection.lock().await;
            let mut conn = conn_ref.clone().ok_or_else(|| {
                crate::error::PortsyncError::Database("Not connected to Redis".to_string())
            })?;

            let _: () = redis::AsyncCommands::del(&mut conn, key)
                .await
                .map_err(|e| crate::error::PortsyncError::Database(format!("DEL failed: {}", e)))?;
            Ok(())
        }
    }

    /// Get all keys matching pattern
    pub async fn keys(&self, _pattern: &str) -> Result<Vec<String>> {
        #[cfg(test)]
        {
            let pattern = _pattern;
            let data = self.data.lock().await;
            let keys: Vec<_> = data
                .keys()
                .filter(|k| {
                    if pattern == "*" {
                        true
                    } else if pattern.ends_with('*') {
                        let prefix = pattern.strip_suffix('*').unwrap();
                        k.starts_with(prefix)
                    } else {
                        k.as_str() == pattern
                    }
                })
                .cloned()
                .collect();
            Ok(keys)
        }

        #[cfg(not(test))]
        {
            let pattern = _pattern;
            let conn_ref = self.connection.lock().await;
            let mut conn = conn_ref.clone().ok_or_else(|| {
                crate::error::PortsyncError::Database("Not connected to Redis".to_string())
            })?;

            redis::AsyncCommands::keys(&mut conn, pattern)
                .await
                .map_err(|e| crate::error::PortsyncError::Database(format!("KEYS failed: {}", e)))
        }
    }

    /// Get database name
    pub fn database_name(&self) -> &str {
        &self.db_name
    }

    /// Get host
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Get port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get database number
    pub fn db_number(&self) -> u32 {
        self.db_number
    }
}

#[async_trait::async_trait]
impl crate::config::DatabaseAdapter for RedisAdapter {
    async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>> {
        RedisAdapter::hgetall(self, key).await
    }

    async fn hset(&mut self, key: &str, fields: &[(String, String)]) -> Result<()> {
        RedisAdapter::hset(self, key, fields).await
    }

    async fn delete(&mut self, key: &str) -> Result<()> {
        RedisAdapter::delete(self, key).await
    }

    async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        RedisAdapter::keys(self, pattern).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_adapter_creation() {
        let adapter = RedisAdapter::new("TEST_DB", "127.0.0.1", 6379, 0);
        assert_eq!(adapter.database_name(), "TEST_DB");
        assert_eq!(adapter.host(), "127.0.0.1");
        assert_eq!(adapter.port(), 6379);
        assert_eq!(adapter.db_number(), 0);
    }

    #[test]
    fn test_config_db_adapter() {
        let adapter = RedisAdapter::config_db("127.0.0.1", 6379);
        assert_eq!(adapter.database_name(), "CONFIG_DB");
        assert_eq!(adapter.db_number(), 4);
    }

    #[test]
    fn test_app_db_adapter() {
        let adapter = RedisAdapter::app_db("127.0.0.1", 6379);
        assert_eq!(adapter.database_name(), "APP_DB");
        assert_eq!(adapter.db_number(), 0);
    }

    #[test]
    fn test_state_db_adapter() {
        let adapter = RedisAdapter::state_db("127.0.0.1", 6379);
        assert_eq!(adapter.database_name(), "STATE_DB");
        assert_eq!(adapter.db_number(), 6);
    }

    #[tokio::test]
    async fn test_hset_and_hgetall() {
        let mut adapter = RedisAdapter::new("TEST_DB", "127.0.0.1", 6379, 0);
        let fields = vec![
            ("field1".to_string(), "value1".to_string()),
            ("field2".to_string(), "value2".to_string()),
        ];

        adapter
            .hset("test_key", &fields)
            .await
            .expect("Failed to set");

        let result = adapter.hgetall("test_key").await.expect("Failed to get");

        assert_eq!(result.get("field1"), Some(&"value1".to_string()));
        assert_eq!(result.get("field2"), Some(&"value2".to_string()));
    }

    #[tokio::test]
    async fn test_delete() {
        let mut adapter = RedisAdapter::new("TEST_DB", "127.0.0.1", 6379, 0);
        let fields = vec![("field".to_string(), "value".to_string())];

        adapter
            .hset("test_key", &fields)
            .await
            .expect("Failed to set");

        let result = adapter.hgetall("test_key").await.expect("Failed to get");
        assert!(!result.is_empty());

        adapter.delete("test_key").await.expect("Failed to delete");

        let result = adapter.hgetall("test_key").await.expect("Failed to get");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_keys_pattern_matching() {
        let mut adapter = RedisAdapter::new("TEST_DB", "127.0.0.1", 6379, 0);

        adapter
            .hset("PORT|Ethernet0", &[])
            .await
            .expect("Failed to set");
        adapter
            .hset("PORT|Ethernet4", &[])
            .await
            .expect("Failed to set");
        adapter.hset("other_key", &[]).await.expect("Failed to set");

        let keys = adapter.keys("PORT|*").await.expect("Failed to get keys");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"PORT|Ethernet0".to_string()));
        assert!(keys.contains(&"PORT|Ethernet4".to_string()));
    }

    #[tokio::test]
    async fn test_connect() {
        let mut adapter = RedisAdapter::new("TEST_DB", "127.0.0.1", 6379, 0);
        assert!(adapter.connect().await.is_ok());
    }

    #[tokio::test]
    async fn test_hgetall_empty_key() {
        let adapter = RedisAdapter::new("TEST_DB", "127.0.0.1", 6379, 0);
        let result = adapter
            .hgetall("nonexistent_key")
            .await
            .expect("Should not fail");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_operations() {
        let mut adapter = RedisAdapter::new("TEST_DB", "127.0.0.1", 6379, 0);

        // Set multiple keys
        adapter
            .hset("key1", &[("f1".to_string(), "v1".to_string())])
            .await
            .expect("Failed");
        adapter
            .hset("key2", &[("f2".to_string(), "v2".to_string())])
            .await
            .expect("Failed");

        // Get all keys
        let keys = adapter.keys("*").await.expect("Failed");
        assert_eq!(keys.len(), 2);

        // Get specific key
        let data = adapter.hgetall("key1").await.expect("Failed");
        assert_eq!(data.get("f1"), Some(&"v1".to_string()));

        // Delete key
        adapter.delete("key1").await.expect("Failed");
        let keys = adapter.keys("*").await.expect("Failed");
        assert_eq!(keys.len(), 1);
    }
}
