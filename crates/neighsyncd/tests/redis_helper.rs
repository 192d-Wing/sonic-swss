//! Redis test utilities for integration tests
//!
//! Provides helper functions to start and manage Redis containers for testing using testcontainers.

use redis::{AsyncCommands, Client};
use std::time::Duration;
use testcontainers::{
    GenericImage,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
};

/// Redis test environment with containerized Redis instance
pub struct RedisTestEnv {
    _container: testcontainers::ContainerAsync<GenericImage>,
    pub client: Client,
    pub host: String,
    pub port: u16,
}

impl RedisTestEnv {
    /// Start a new Redis container for testing
    ///
    /// # Returns
    /// A `RedisTestEnv` with a running Redis container and connected client
    ///
    /// # Errors
    /// Returns error if container fails to start or client connection fails
    pub async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        // Create Redis container with explicit configuration
        let container = GenericImage::new("redis", "7-alpine")
            .with_exposed_port(ContainerPort::Tcp(6379))
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
            .with_wait_for(WaitFor::seconds(2))
            .start()
            .await?;

        // Get host and port
        let host = container.get_host().await?.to_string();
        let port = container.get_host_port_ipv4(6379).await?;

        // Create Redis client
        let redis_url = format!("redis://{}:{}", host, port);
        let client = Client::open(redis_url.as_str())?;

        // Verify connection with retry
        for _ in 0..5 {
            match client.get_connection_with_timeout(Duration::from_secs(1)) {
                Ok(_) => break,
                Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }

        Ok(Self {
            _container: container,
            client,
            host,
            port,
        })
    }

    /// Get an async Redis connection
    ///
    /// # Returns
    /// An async connection to the Redis instance
    ///
    /// # Errors
    /// Returns error if connection fails
    pub async fn get_async_connection(
        &self,
    ) -> Result<redis::aio::MultiplexedConnection, redis::RedisError> {
        self.client.get_multiplexed_tokio_connection().await
    }

    /// Flush all keys from all databases
    ///
    /// # Errors
    /// Returns error if FLUSHALL command fails
    pub async fn flush_all(&self) -> Result<(), redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        redis::cmd("FLUSHALL").query_async::<()>(&mut conn).await?;
        Ok(())
    }

    /// Get a specific key value
    ///
    /// # Errors
    /// Returns error if GET command fails
    pub async fn get(&self, key: &str) -> Result<Option<String>, redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.get(key).await
    }

    /// Set a key value
    ///
    /// # Errors
    /// Returns error if SET command fails
    pub async fn set(&self, key: &str, value: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.set(key, value).await
    }

    /// Delete a key
    ///
    /// # Errors
    /// Returns error if DEL command fails
    pub async fn del(&self, key: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.del(key).await
    }

    /// Check if a key exists
    ///
    /// # Errors
    /// Returns error if EXISTS command fails
    pub async fn exists(&self, key: &str) -> Result<bool, redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.exists(key).await
    }

    /// Get all keys matching a pattern
    ///
    /// # Errors
    /// Returns error if KEYS command fails
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>, redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.keys(pattern).await
    }

    /// Get the number of keys in the database
    ///
    /// # Errors
    /// Returns error if DBSIZE command fails
    pub async fn dbsize(&self) -> Result<usize, redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        redis::cmd("DBSIZE").query_async::<usize>(&mut conn).await
    }

    /// Set a hash field
    ///
    /// # Errors
    /// Returns error if HSET command fails
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.hset(key, field, value).await
    }

    /// Get a hash field
    ///
    /// # Errors
    /// Returns error if HGET command fails
    pub async fn hget(&self, key: &str, field: &str) -> Result<Option<String>, redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.hget(key, field).await
    }

    /// Get all hash fields and values
    ///
    /// # Errors
    /// Returns error if HGETALL command fails
    pub async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>, redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.hgetall(key).await
    }

    /// Delete a hash field
    ///
    /// # Errors
    /// Returns error if HDEL command fails
    pub async fn hdel(&self, key: &str, field: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.get_async_connection().await?;
        conn.hdel(key, field).await
    }
}

// Container is automatically stopped and removed when RedisTestEnv is dropped

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Docker"]
    async fn test_redis_test_env_start() {
        let env = RedisTestEnv::start().await.expect("Failed to start Redis");
        assert!(env.port > 0);
        assert!(!env.host.is_empty());
    }

    #[tokio::test]
    #[ignore = "Requires Docker"]
    async fn test_redis_basic_operations() {
        let env = RedisTestEnv::start().await.expect("Failed to start Redis");

        // Test SET and GET
        env.set("test_key", "test_value")
            .await
            .expect("Failed to set key");
        let value = env.get("test_key").await.expect("Failed to get key");
        assert_eq!(value, Some("test_value".to_string()));

        // Test EXISTS
        let exists = env
            .exists("test_key")
            .await
            .expect("Failed to check exists");
        assert!(exists);

        // Test DEL
        env.del("test_key").await.expect("Failed to delete key");
        let exists = env
            .exists("test_key")
            .await
            .expect("Failed to check exists");
        assert!(!exists);
    }

    #[tokio::test]
    #[ignore = "Requires Docker"]
    async fn test_redis_hash_operations() {
        let env = RedisTestEnv::start().await.expect("Failed to start Redis");

        // Test HSET and HGET
        env.hset("test_hash", "field1", "value1")
            .await
            .expect("Failed to hset");
        let value = env
            .hget("test_hash", "field1")
            .await
            .expect("Failed to hget");
        assert_eq!(value, Some("value1".to_string()));

        // Test HGETALL
        env.hset("test_hash", "field2", "value2")
            .await
            .expect("Failed to hset");
        let all = env.hgetall("test_hash").await.expect("Failed to hgetall");
        assert_eq!(all.len(), 2);

        // Test HDEL
        env.hdel("test_hash", "field1")
            .await
            .expect("Failed to hdel");
        let value = env
            .hget("test_hash", "field1")
            .await
            .expect("Failed to hget");
        assert_eq!(value, None);
    }

    #[tokio::test]
    #[ignore = "Requires Docker"]
    async fn test_redis_flush_all() {
        let env = RedisTestEnv::start().await.expect("Failed to start Redis");

        // Add some keys
        env.set("key1", "value1").await.expect("Failed to set");
        env.set("key2", "value2").await.expect("Failed to set");

        // Flush all
        env.flush_all().await.expect("Failed to flush");

        // Verify empty
        let size = env.dbsize().await.expect("Failed to get dbsize");
        assert_eq!(size, 0);
    }

    #[tokio::test]
    #[ignore = "Requires Docker"]
    async fn test_redis_keys_pattern() {
        let env = RedisTestEnv::start().await.expect("Failed to start Redis");

        // Add keys with pattern
        env.set("NEIGH_TABLE:eth0:2001:db8::1", "value1")
            .await
            .expect("Failed to set");
        env.set("NEIGH_TABLE:eth0:2001:db8::2", "value2")
            .await
            .expect("Failed to set");
        env.set("OTHER_TABLE:key", "value3")
            .await
            .expect("Failed to set");

        // Get keys matching pattern
        let keys = env.keys("NEIGH_TABLE:*").await.expect("Failed to get keys");
        assert_eq!(keys.len(), 2);
    }
}
