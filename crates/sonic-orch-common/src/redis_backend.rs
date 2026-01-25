//! Redis database backend for SONiC orchestration.
//!
//! This module provides Redis connection management and table polling for the Orch framework.
//! It enables Consumer to read from real Redis databases (CONFIG_DB, APPL_DB, STATE_DB).
//!
//! # NIST Controls
//! - SC-7: Boundary Protection - Database communication security
//! - SC-8: Transmission Confidentiality - Redis connection encryption
//! - SI-4: System Monitoring - Event polling from Redis

use crate::{Consumer, ConsumerConfig, KeyOpFieldsValues, Operation};
use log::{debug, info};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Errors from Redis operations.
#[derive(Error, Debug)]
pub enum RedisBackendError {
    #[error("Redis connection error: {0}")]
    ConnectionError(String),

    #[error("Redis command error: {0}")]
    CommandError(String),

    #[error("Table not found: {0}")]
    TableNotFound(String),

    #[error("Invalid data format: {0}")]
    InvalidData(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result type for Redis backend operations.
pub type Result<T> = std::result::Result<T, RedisBackendError>;

/// Redis database selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RedisDb {
    /// CONFIG_DB (database 4) - switch configuration
    ConfigDb = 4,
    /// APPL_DB (database 0) - application state
    ApplDb = 0,
    /// STATE_DB (database 6) - hardware state and statistics
    StateDb = 6,
    /// COUNTER_DB (database 2) - counter statistics
    CounterDb = 2,
}

/// Configuration for Redis connection.
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis server hostname or IP
    pub host: String,
    /// Redis server port
    pub port: u16,
    /// Database selector
    pub db: RedisDb,
}

impl RedisConfig {
    /// Creates a new Redis configuration.
    pub fn new(host: impl Into<String>, port: u16, db: RedisDb) -> Self {
        Self {
            host: host.into(),
            port,
            db,
        }
    }

    /// Creates CONFIG_DB connection config.
    pub fn config_db(host: impl Into<String>, port: u16) -> Self {
        Self::new(host, port, RedisDb::ConfigDb)
    }

    /// Creates APPL_DB connection config.
    pub fn appl_db(host: impl Into<String>, port: u16) -> Self {
        Self::new(host, port, RedisDb::ApplDb)
    }

    /// Creates STATE_DB connection config.
    pub fn state_db(host: impl Into<String>, port: u16) -> Self {
        Self::new(host, port, RedisDb::StateDb)
    }

    /// Creates COUNTER_DB connection config.
    pub fn counter_db(host: impl Into<String>, port: u16) -> Self {
        Self::new(host, port, RedisDb::CounterDb)
    }

    /// Returns the Redis connection URI.
    fn uri(&self) -> String {
        format!("redis://{}:{}/{}", self.host, self.port, self.db as u8)
    }
}

/// Redis database connection with table polling support.
pub struct RedisDatabase {
    config: RedisConfig,
    connection: ConnectionManager,
}

impl RedisDatabase {
    /// Creates a new Redis database connection.
    pub async fn new(config: RedisConfig) -> Result<Self> {
        let uri = config.uri();

        let client = redis::Client::open(uri.clone())
            .map_err(|e| RedisBackendError::ConnectionError(format!("{}: {}", uri, e)))?;

        let connection = client.get_connection_manager().await.map_err(|e| {
            RedisBackendError::ConnectionError(format!("Failed to create connection pool: {}", e))
        })?;

        info!(
            "Connected to Redis: {} (db={})",
            config.host, config.db as u8
        );

        Ok(Self { config, connection })
    }

    /// Returns the database configuration.
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }

    /// Polls table entries from Redis using BLPOP.
    ///
    /// This blocks until entries are available or timeout occurs.
    /// Returns entries in the format: [key, op, field1, value1, field2, value2, ...]
    pub async fn poll_table(
        &mut self,
        table_name: &str,
        batch_size: usize,
        timeout_secs: f64,
    ) -> Result<Vec<KeyOpFieldsValues>> {
        let queue_key = format!("{}:*", table_name);
        debug!(
            "Polling {} (batch={}, timeout={}s)",
            table_name, batch_size, timeout_secs
        );

        let mut entries = Vec::new();

        // BLPOP with timeout (blocking list pop)
        for _ in 0..batch_size {
            let result: Option<Vec<String>> = self
                .connection
                .blpop(&queue_key, timeout_secs)
                .await
                .map_err(|e| RedisBackendError::CommandError(format!("BLPOP failed: {}", e)))?;

            match result {
                Some(data) => {
                    if let Ok(entry) = parse_redis_entry(&data) {
                        entries.push(entry);
                    }
                }
                None => break, // Timeout or no more data
            }
        }

        debug!("Polled {} entries from {}", entries.len(), table_name);
        Ok(entries)
    }

    /// Reads all entries from a table (used for initial load).
    ///
    /// This uses HGETALL to read the entire table at once.
    pub async fn read_table(&mut self, table_name: &str) -> Result<Vec<KeyOpFieldsValues>> {
        debug!("Reading entire table: {}", table_name);

        let table_key = format!("{}|*", table_name);

        // Get all keys matching the pattern
        let keys: Vec<String> = self
            .connection
            .keys(&table_key)
            .await
            .map_err(|e| RedisBackendError::CommandError(format!("KEYS failed: {}", e)))?;

        let mut entries = Vec::new();

        // For each key, get all field-value pairs
        for key in keys {
            let fvs: HashMap<String, String> =
                self.connection.hgetall(&key).await.map_err(|e| {
                    RedisBackendError::CommandError(format!("HGETALL failed: {}", e))
                })?;

            let key_name = key.split('|').nth(1).unwrap_or("").to_string();
            let fvs_vec: Vec<(String, String)> = fvs.into_iter().collect();

            entries.push(KeyOpFieldsValues::set(key_name, fvs_vec));
        }

        debug!("Read {} entries from table {}", entries.len(), table_name);
        Ok(entries)
    }

    /// Checks if table exists by attempting to get keys.
    pub async fn table_exists(&mut self, table_name: &str) -> Result<bool> {
        let pattern = format!("{}|*", table_name);
        let keys: Vec<String> = self.connection.keys(&pattern).await.unwrap_or_default();

        Ok(!keys.is_empty())
    }

    /// Deletes an entry from a table.
    pub async fn delete_entry(&mut self, table_name: &str, key: &str) -> Result<()> {
        let redis_key = format!("{}|{}", table_name, key);
        let _: () = self
            .connection
            .del(&redis_key)
            .await
            .map_err(|e| RedisBackendError::CommandError(format!("DEL failed: {}", e)))?;

        Ok(())
    }

    /// Sets an entry in a table.
    pub async fn set_entry(
        &mut self,
        table_name: &str,
        key: &str,
        fields: &[(String, String)],
    ) -> Result<()> {
        let redis_key = format!("{}|{}", table_name, key);

        for (field, value) in fields {
            let _: () = self
                .connection
                .hset(&redis_key, field, value)
                .await
                .map_err(|e| RedisBackendError::CommandError(format!("HSET failed: {}", e)))?;
        }

        Ok(())
    }
}

/// Parses a Redis entry from the list format.
/// Format: [key, op, field1, value1, field2, value2, ...]
fn parse_redis_entry(data: &[String]) -> Result<KeyOpFieldsValues> {
    if data.len() < 2 {
        return Err(RedisBackendError::InvalidData(
            "Entry must have at least key and operation".to_string(),
        ));
    }

    let key = data[0].clone();
    let op = match data[1].as_str() {
        "SET" => Operation::Set,
        "DEL" => Operation::Del,
        unknown => {
            return Err(RedisBackendError::InvalidData(format!(
                "Unknown operation: {}",
                unknown
            )));
        }
    };

    let mut fvs = Vec::new();

    // Parse field-value pairs
    for i in (2..data.len()).step_by(2) {
        if i + 1 < data.len() {
            fvs.push((data[i].clone(), data[i + 1].clone()));
        }
    }

    Ok(KeyOpFieldsValues::new(key, op, fvs))
}

/// Extended Consumer with Redis backend support.
pub struct RedisBoundConsumer {
    consumer: Consumer,
    database: Arc<RwLock<RedisDatabase>>,
}

impl RedisBoundConsumer {
    /// Creates a new Consumer bound to a Redis database.
    pub fn new(config: ConsumerConfig, database: Arc<RwLock<RedisDatabase>>) -> Self {
        Self {
            consumer: Consumer::new(config),
            database,
        }
    }

    /// Populates the consumer from Redis with BLPOP polling.
    pub async fn populate_from_redis(
        &mut self,
        batch_size: usize,
        timeout_secs: f64,
    ) -> Result<()> {
        let mut db = self.database.write().await;

        let entries = db
            .poll_table(self.consumer.table_name(), batch_size, timeout_secs)
            .await?;

        if !entries.is_empty() {
            self.consumer.add_to_sync(entries);
        }

        Ok(())
    }

    /// Loads initial table state from Redis.
    pub async fn load_initial_state(&mut self) -> Result<()> {
        let mut db = self.database.write().await;

        let entries = db.read_table(self.consumer.table_name()).await?;

        if !entries.is_empty() {
            self.consumer.add_to_sync(entries);
        }

        Ok(())
    }

    /// Returns the underlying Consumer.
    pub fn consumer(&self) -> &Consumer {
        &self.consumer
    }

    /// Returns the underlying Consumer (mutable).
    pub fn consumer_mut(&mut self) -> &mut Consumer {
        &mut self.consumer
    }

    /// Drains all pending entries.
    pub fn drain(&mut self) -> Vec<KeyOpFieldsValues> {
        self.consumer.drain()
    }

    /// Returns true if there are pending entries.
    pub fn has_pending(&self) -> bool {
        self.consumer.has_pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config() {
        let config = RedisConfig::config_db("127.0.0.1", 6379);
        assert_eq!(config.db, RedisDb::ConfigDb);
        assert_eq!(config.uri(), "redis://127.0.0.1:6379/4");
    }

    #[test]
    fn test_parse_redis_entry_set() {
        let data = vec![
            "Ethernet0".to_string(),
            "SET".to_string(),
            "admin_status".to_string(),
            "up".to_string(),
            "speed".to_string(),
            "100000".to_string(),
        ];

        let entry = parse_redis_entry(&data).unwrap();
        assert_eq!(entry.key, "Ethernet0");
        assert_eq!(entry.op, Operation::Set);
        assert_eq!(entry.fvs.len(), 2);
        assert_eq!(entry.get_field("admin_status"), Some("up"));
        assert_eq!(entry.get_field("speed"), Some("100000"));
    }

    #[test]
    fn test_parse_redis_entry_del() {
        let data = vec!["Ethernet0".to_string(), "DEL".to_string()];

        let entry = parse_redis_entry(&data).unwrap();
        assert_eq!(entry.key, "Ethernet0");
        assert_eq!(entry.op, Operation::Del);
        assert!(entry.fvs.is_empty());
    }

    #[test]
    fn test_parse_redis_entry_invalid() {
        let data = vec!["Ethernet0".to_string()];
        let result = parse_redis_entry(&data);
        assert!(result.is_err());
    }
}
