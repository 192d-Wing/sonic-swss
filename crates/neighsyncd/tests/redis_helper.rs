//! Redis Test Utilities
//!
//! Provides utilities for integration testing with real Redis instances
//! using testcontainers for containerized Redis.

use redis::{Client, Commands, Connection, RedisResult};
use std::time::Duration;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage};

/// Redis test environment with containerized Redis instance
pub struct RedisTestEnv {
    container: ContainerAsync<GenericImage>,
    client: Client,
    port: u16,
}

impl RedisTestEnv {
    /// Start a new Redis container and connect to it
    pub async fn new() -> RedisResult<Self> {
        // Create Redis container
        let image = GenericImage::new("redis", "7-alpine")
            .with_exposed_port(6379.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"));

        let container = image
            .start()
            .await
            .map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Container start failed", e.to_string())))?;

        // Get the mapped port
        let port = container
            .get_host_port_ipv4(6379)
            .await
            .map_err(|e| redis::RedisError::from((redis::ErrorKind::IoError, "Port mapping failed", e.to_string())))?;

        // Connect to Redis
        let url = format!("redis://127.0.0.1:{}", port);
        let client = Client::open(url.as_str())?;

        // Wait for Redis to be ready
        let mut retries = 0;
        let max_retries = 10;
        loop {
            match client.get_connection() {
                Ok(_) => break,
                Err(e) if retries < max_retries => {
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(Self {
            container,
            client,
            port,
        })
    }

    /// Get a connection to Redis
    pub fn get_connection(&self) -> RedisResult<Connection> {
        self.client.get_connection()
    }

    /// Get Redis connection URL
    pub fn url(&self) -> String {
        format!("redis://127.0.0.1:{}", self.port)
    }

    /// Flush all data from all databases
    pub fn flush_all(&self) -> RedisResult<()> {
        let mut conn = self.get_connection()?;
        redis::cmd("FLUSHALL").execute(&mut conn);
        Ok(())
    }

    /// Get all keys matching pattern
    pub fn keys(&self, pattern: &str) -> RedisResult<Vec<String>> {
        let mut conn = self.get_connection()?;
        conn.keys(pattern)
    }

    /// Get value for a hash field
    pub fn hget<T: redis::FromRedisValue>(&self, key: &str, field: &str) -> RedisResult<T> {
        let mut conn = self.get_connection()?;
        conn.hget(key, field)
    }

    /// Get all hash fields and values
    pub fn hgetall(&self, key: &str) -> RedisResult<std::collections::HashMap<String, String>> {
        let mut conn = self.get_connection()?;
        conn.hgetall(key)
    }

    /// Set a hash field
    pub fn hset(&self, key: &str, field: &str, value: &str) -> RedisResult<()> {
        let mut conn = self.get_connection()?;
        let _: () = conn.hset(key, field, value)?;
        Ok(())
    }

    /// Delete a key
    pub fn del(&self, key: &str) -> RedisResult<()> {
        let mut conn = self.get_connection()?;
        let _: () = conn.del(key)?;
        Ok(())
    }

    /// Check if key exists
    pub fn exists(&self, key: &str) -> RedisResult<bool> {
        let mut conn = self.get_connection()?;
        conn.exists(key)
    }

    /// Get number of keys in database
    pub fn dbsize(&self) -> RedisResult<usize> {
        let mut conn = self.get_connection()?;
        redis::cmd("DBSIZE").query(&mut conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Docker
    async fn test_redis_env_creation() {
        let env = RedisTestEnv::new().await.expect("Failed to create Redis env");

        // Verify connection
        let mut conn = env.get_connection().expect("Failed to get connection");
        let pong: String = redis::cmd("PING").query(&mut conn).expect("PING failed");
        assert_eq!(pong, "PONG");
    }

    #[tokio::test]
    #[ignore] // Requires Docker
    async fn test_redis_operations() {
        let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
        env.flush_all().expect("Failed to flush");

        // Test HSET/HGET
        env.hset("test:key", "field1", "value1").expect("HSET failed");
        let value: String = env.hget("test:key", "field1").expect("HGET failed");
        assert_eq!(value, "value1");

        // Test EXISTS
        assert!(env.exists("test:key").expect("EXISTS failed"));

        // Test DEL
        env.del("test:key").expect("DEL failed");
        assert!(!env.exists("test:key").expect("EXISTS failed"));

        // Test DBSIZE
        let size = env.dbsize().expect("DBSIZE failed");
        assert_eq!(size, 0);
    }
}
