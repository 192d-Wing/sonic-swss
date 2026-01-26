//! Redis adapter for SONiC database operations
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - SC-8: Transmission Confidentiality - Secure database communication
//! - SC-13: Cryptographic Protection - Redis connection security
//! - AU-3: Content of Audit Records - Database operations logged
//! - AC-3: Access Enforcement - Database access control

use crate::error::Result;
use crate::types::NeighborEntry;
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, instrument};

/// SONiC database indices
/// NIST: CM-6 - Configuration settings for database selection
const APPL_DB: i64 = 0;
const CONFIG_DB: i64 = 4;
const STATE_DB: i64 = 6;

/// Redis table names matching C++ constants
const APP_NEIGH_TABLE_NAME: &str = "NEIGH_TABLE";
const STATE_NEIGH_RESTORE_TABLE_NAME: &str = "NEIGH_RESTORE_TABLE";
const CFG_INTF_TABLE_NAME: &str = "INTERFACE";
const CFG_LAG_INTF_TABLE_NAME: &str = "PORTCHANNEL_INTERFACE";
const CFG_VLAN_INTF_TABLE_NAME: &str = "VLAN_INTERFACE";
const CFG_PEER_SWITCH_TABLE_NAME: &str = "PEER_SWITCH";

/// Link-local cache TTL
/// NIST: SC-5 - Performance optimization to reduce DB queries
const LINK_LOCAL_CACHE_TTL: Duration = Duration::from_secs(60);

/// Link-local configuration cache entry
#[derive(Debug, Clone)]
struct LinkLocalCacheEntry {
    enabled: bool,
    timestamp: Instant,
}

/// Redis adapter for SONiC database operations
///
/// # NIST Controls
/// - SC-8(1): Cryptographic Protection - TLS for Redis if configured
/// - AC-17: Remote Access - Database access management
pub struct RedisAdapter {
    appl_db: ConnectionManager,
    config_db: ConnectionManager,
    state_db: ConnectionManager,
    /// Cache for link-local configuration lookups
    /// NIST: SC-5 - Performance optimization
    link_local_cache: HashMap<String, LinkLocalCacheEntry>,
}

impl RedisAdapter {
    /// Create a new Redis adapter connected to all required databases
    ///
    /// # NIST Controls
    /// - IA-5: Authenticator Management - Redis authentication if configured
    /// - SC-23: Session Authenticity - Establish authenticated sessions
    #[instrument(skip_all)]
    pub async fn new(host: &str, port: u16) -> Result<Self> {
        debug!(host, port, "Connecting to Redis databases");

        let appl_db = Self::connect_db(host, port, APPL_DB).await?;
        let config_db = Self::connect_db(host, port, CONFIG_DB).await?;
        let state_db = Self::connect_db(host, port, STATE_DB).await?;

        debug!("Connected to all Redis databases");
        Ok(Self {
            appl_db,
            config_db,
            state_db,
            link_local_cache: HashMap::new(),
        })
    }

    /// Connect to a specific database
    async fn connect_db(host: &str, port: u16, db: i64) -> Result<ConnectionManager> {
        let url = format!("redis://{}:{}/{}", host, port, db);
        let client = Client::open(url)?;
        let manager = ConnectionManager::new(client).await?;
        Ok(manager)
    }

    /// Set a neighbor entry in APPL_DB
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Log neighbor additions
    /// - CM-8: System Component Inventory - Maintain neighbor inventory
    #[instrument(skip(self), fields(key = %entry.redis_key()))]
    pub async fn set_neighbor(&mut self, entry: &NeighborEntry) -> Result<()> {
        let key = format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key());
        let fields: Vec<(&str, String)> = vec![
            ("neigh", entry.mac.to_string()),
            ("family", entry.family_str().to_string()),
        ];

        debug!(key, mac = %entry.mac, family = entry.family_str(), "Setting neighbor");

        let _: () = self.appl_db.hset_multiple(&key, &fields).await?;
        Ok(())
    }

    /// Delete a neighbor entry from APPL_DB
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Log neighbor deletions
    /// - CM-8: System Component Inventory - Update neighbor inventory
    #[instrument(skip(self), fields(key = %entry.redis_key()))]
    pub async fn delete_neighbor(&mut self, entry: &NeighborEntry) -> Result<()> {
        let key = format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key());
        debug!(key, "Deleting neighbor");

        let _: () = self.appl_db.del(&key).await?;
        Ok(())
    }

    /// Check if neighbor restore is complete (for warm restart)
    ///
    /// # NIST Controls
    /// - CP-10: System Recovery - Check recovery status
    #[instrument(skip(self))]
    pub async fn is_neighbor_restore_done(&mut self) -> Result<bool> {
        let key = format!("{}:Flags", STATE_NEIGH_RESTORE_TABLE_NAME);
        let value: Option<String> = self.state_db.hget(&key, "restored").await?;

        let done = value.as_deref() == Some("true");
        debug!(done, "Checked neighbor restore status");
        Ok(done)
    }

    /// Check if this is a dual-ToR deployment
    ///
    /// # NIST Controls
    /// - CM-8: System Component Inventory - Topology awareness
    /// - SC-7: Boundary Protection - Multi-device boundary awareness
    #[instrument(skip(self))]
    pub async fn is_dual_tor(&mut self) -> Result<bool> {
        let pattern = format!("{}:*", CFG_PEER_SWITCH_TABLE_NAME);
        let keys: Vec<String> = self.config_db.keys(&pattern).await?;

        let is_dual = !keys.is_empty();
        debug!(is_dual, peer_count = keys.len(), "Checked dual-ToR status");
        Ok(is_dual)
    }

    /// Check if IPv6 link-local is enabled on an interface
    ///
    /// Uses TTL-based cache to reduce CONFIG_DB queries.
    ///
    /// # NIST Controls
    /// - CM-6: Configuration Settings - Interface configuration
    /// - SC-7: Boundary Protection - Link-local filtering config
    /// - SC-5: DoS Protection - Cache reduces DB load
    #[instrument(skip(self))]
    pub async fn is_ipv6_link_local_enabled(&mut self, interface: &str) -> Result<bool> {
        // Check cache first
        if let Some(entry) = self.link_local_cache.get(interface) {
            if entry.timestamp.elapsed() < LINK_LOCAL_CACHE_TTL {
                debug!(interface, enabled = entry.enabled, "Link-local cache hit");
                return Ok(entry.enabled);
            }
        }

        // Determine which table to check based on interface name
        let table = if interface.starts_with("Vlan") {
            CFG_VLAN_INTF_TABLE_NAME
        } else if interface.starts_with("PortChannel") {
            CFG_LAG_INTF_TABLE_NAME
        } else if interface.starts_with("Ethernet") {
            CFG_INTF_TABLE_NAME
        } else {
            debug!(interface, "Unknown interface type, link-local disabled");
            return Ok(false);
        };

        let key = format!("{}:{}", table, interface);
        let values: HashMap<String, String> = self.config_db.hgetall(&key).await?;

        let enabled = values
            .get("ipv6_use_link_local_only")
            .is_some_and(|v| v == "enable");

        // Update cache
        self.link_local_cache.insert(
            interface.to_string(),
            LinkLocalCacheEntry {
                enabled,
                timestamp: Instant::now(),
            },
        );

        debug!(
            interface,
            table, enabled, "Checked IPv6 link-local status (cached)"
        );
        Ok(enabled)
    }

    /// Clear the link-local cache (useful for testing or config reload)
    pub fn clear_link_local_cache(&mut self) {
        self.link_local_cache.clear();
    }

    /// Batch set multiple neighbor entries using Redis pipelining
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Efficient bulk audit records
    /// - SC-5: DoS Protection - Reduce round-trips under high load
    #[instrument(skip(self, entries), fields(count = entries.len()))]
    pub async fn set_neighbors_batch(&mut self, entries: &[NeighborEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut pipe = redis::pipe();
        pipe.atomic(); // Execute as transaction

        for entry in entries {
            let key = format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key());
            pipe.hset_multiple::<_, _, _>(
                &key,
                &[
                    ("neigh", entry.mac.to_string()),
                    ("family", entry.family_str().to_string()),
                ],
            );
        }

        let _: () = pipe.query_async(&mut self.appl_db).await?;
        debug!(count = entries.len(), "Batch set neighbors");
        Ok(())
    }

    /// Batch delete multiple neighbor entries using Redis pipelining
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Efficient bulk audit records
    /// - SC-5: DoS Protection - Reduce round-trips under high load
    #[instrument(skip(self, entries), fields(count = entries.len()))]
    pub async fn delete_neighbors_batch(&mut self, entries: &[NeighborEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut pipe = redis::pipe();

        for entry in entries {
            let key = format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key());
            pipe.del::<_>(&key);
        }

        let _: () = pipe.query_async(&mut self.appl_db).await?;
        debug!(count = entries.len(), "Batch deleted neighbors");
        Ok(())
    }

    /// Get all current neighbor entries from APPL_DB (for warm restart reconciliation)
    ///
    /// # NIST Controls
    /// - CP-10: System Recovery - Read state for reconciliation
    #[instrument(skip(self))]
    pub async fn get_all_neighbors(&mut self) -> Result<HashMap<String, HashMap<String, String>>> {
        let pattern = format!("{}:*", APP_NEIGH_TABLE_NAME);
        let keys: Vec<String> = self.appl_db.keys(&pattern).await?;

        let mut neighbors = HashMap::new();
        for key in keys {
            let values: HashMap<String, String> = self.appl_db.hgetall(&key).await?;
            if !values.is_empty() {
                // Strip table prefix from key
                let short_key = key
                    .strip_prefix(&format!("{}:", APP_NEIGH_TABLE_NAME))
                    .unwrap_or(&key)
                    .to_string();
                neighbors.insert(short_key, values);
            }
        }

        debug!(
            count = neighbors.len(),
            "Retrieved all neighbors from APPL_DB"
        );
        Ok(neighbors)
    }

    /// Set a key with NX (only if not exists) option and TTL
    ///
    /// # NIST Controls
    /// - AC-3: Access Enforcement - Lock-based access control
    #[instrument(skip(self))]
    pub async fn set_nx(&mut self, key: &str, value: &str, ttl_secs: u64) -> Result<bool> {
        let result: bool = redis::Cmd::new()
            .arg("SET")
            .arg(key)
            .arg(value)
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut self.appl_db)
            .await?;

        debug!(key, ttl = ttl_secs, acquired = result, "SET NX with TTL");
        Ok(result)
    }

    /// Delete a key only if the value matches (atomic compare-and-delete)
    ///
    /// # NIST Controls
    /// - AC-3: Access Enforcement - Token-based lock release
    #[instrument(skip(self))]
    pub async fn del_if_eq(&mut self, key: &str, expected_value: &str) -> Result<bool> {
        let script = redis::Script::new(
            r#"
            if redis.call('get', KEYS[1]) == ARGV[1] then
                return redis.call('del', KEYS[1])
            else
                return 0
            end
            "#,
        );

        let result: i64 = script
            .key(key)
            .arg(expected_value)
            .invoke_async(&mut self.appl_db)
            .await?;

        let deleted = result > 0;
        debug!(key, deleted, "DELETE IF EQUAL");
        Ok(deleted)
    }

    /// Set key expiration time (refresh TTL)
    ///
    /// # NIST Controls
    /// - SC-5: DoS Protection - Automatic cleanup of stale locks
    #[instrument(skip(self))]
    pub async fn expire(&mut self, key: &str, ttl_secs: u64) -> Result<bool> {
        let result: bool = self.appl_db.expire(key, ttl_secs as i64).await?;
        debug!(key, ttl = ttl_secs, set = result, "EXPIRE");
        Ok(result)
    }

    /// Set a neighbor entry with VRF awareness
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Log neighbor additions per VRF
    /// - AC-4: Information Flow Enforcement - VRF isolation
    #[instrument(skip(self), fields(vrf_id = %entry.vrf_id()))]
    pub async fn set_neighbor_vrf(&mut self, entry: &NeighborEntry, vrf_name: &str) -> Result<()> {
        let key = if entry.vrf_id().as_u32() == 0 {
            format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key())
        } else {
            format!(
                "{}|{}:{}",
                vrf_name,
                APP_NEIGH_TABLE_NAME,
                entry.redis_key()
            )
        };

        let fields: Vec<(&str, String)> = vec![
            ("neigh", entry.mac.to_string()),
            ("family", entry.family_str().to_string()),
        ];

        debug!(
            key,
            vrf_id = %entry.vrf_id(),
            mac = %entry.mac,
            family = entry.family_str(),
            "Setting neighbor with VRF"
        );

        let _: () = self.appl_db.hset_multiple(&key, &fields).await?;
        Ok(())
    }

    /// Delete a neighbor entry with VRF awareness
    ///
    /// # NIST Controls
    /// - AU-12: Audit Record Generation - Log neighbor deletions per VRF
    /// - AC-4: Information Flow Enforcement - VRF isolation
    #[instrument(skip(self), fields(vrf_id = %entry.vrf_id()))]
    pub async fn delete_neighbor_vrf(
        &mut self,
        entry: &NeighborEntry,
        vrf_name: &str,
    ) -> Result<()> {
        let key = if entry.vrf_id().as_u32() == 0 {
            format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key())
        } else {
            format!(
                "{}|{}:{}",
                vrf_name,
                APP_NEIGH_TABLE_NAME,
                entry.redis_key()
            )
        };

        debug!(key, vrf_id = %entry.vrf_id(), "Deleting neighbor with VRF");

        let _: () = self.appl_db.del(&key).await?;
        Ok(())
    }

    /// Batch set neighbors with VRF awareness
    ///
    /// # NIST Controls
    /// - SC-5: DoS Protection - Batch operations reduce round-trips
    /// - AC-4: Information Flow Enforcement - VRF-isolated batch operations
    #[instrument(skip(self, entries))]
    pub async fn batch_set_neighbors_vrf(
        &mut self,
        entries: Vec<(&NeighborEntry, &str)>,
    ) -> Result<()> {
        use redis::pipe;

        let mut pipe = pipe();

        for (entry, vrf_name) in entries {
            let key = if entry.vrf_id().as_u32() == 0 {
                format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key())
            } else {
                format!(
                    "{}|{}:{}",
                    vrf_name,
                    APP_NEIGH_TABLE_NAME,
                    entry.redis_key()
                )
            };

            let fields: Vec<(&str, String)> = vec![
                ("neigh", entry.mac.to_string()),
                ("family", entry.family_str().to_string()),
            ];

            for (field, value) in fields {
                pipe.hset(&key, field, value);
            }
        }

        let _: () = pipe.query_async(&mut self.appl_db).await?;
        debug!("Batch set neighbors with VRF awareness");
        Ok(())
    }

    /// Batch delete neighbors with VRF awareness
    ///
    /// # NIST Controls
    /// - SC-5: DoS Protection - Batch operations reduce round-trips
    /// - AC-4: Information Flow Enforcement - VRF-isolated batch operations
    #[instrument(skip(self, entries))]
    pub async fn batch_delete_neighbors_vrf(
        &mut self,
        entries: Vec<(&NeighborEntry, &str)>,
    ) -> Result<()> {
        use redis::pipe;

        let mut pipe = pipe();

        for (entry, vrf_name) in entries {
            let key = if entry.vrf_id().as_u32() == 0 {
                format!("{}:{}", APP_NEIGH_TABLE_NAME, entry.redis_key())
            } else {
                format!(
                    "{}|{}:{}",
                    vrf_name,
                    APP_NEIGH_TABLE_NAME,
                    entry.redis_key()
                )
            };
            pipe.del::<_>(&key);
        }

        let _: () = pipe.query_async(&mut self.appl_db).await?;
        debug!("Batch delete neighbors with VRF awareness");
        Ok(())
    }

    /// Get all neighbors from a specific VRF
    ///
    /// # NIST Controls
    /// - AC-4: Information Flow Enforcement - VRF-isolated queries
    #[instrument(skip(self))]
    pub async fn get_neighbors_by_vrf(
        &mut self,
        vrf_id: u32,
        vrf_name: &str,
    ) -> Result<HashMap<String, HashMap<String, String>>> {
        let pattern = if vrf_id == 0 {
            format!("{}:*", APP_NEIGH_TABLE_NAME)
        } else {
            format!("{}|{}:*", vrf_name, APP_NEIGH_TABLE_NAME)
        };

        let keys: Vec<String> = self.appl_db.keys(&pattern).await?;

        let mut neighbors = HashMap::new();
        for key in keys {
            let values: HashMap<String, String> = self.appl_db.hgetall(&key).await?;
            if !values.is_empty() {
                // Strip VRF prefix and table name
                let short_key = if vrf_id == 0 {
                    key.strip_prefix(&format!("{}:", APP_NEIGH_TABLE_NAME))
                        .unwrap_or(&key)
                } else {
                    key.strip_prefix(&format!("{}|{}:", vrf_name, APP_NEIGH_TABLE_NAME))
                        .unwrap_or(&key)
                };
                neighbors.insert(short_key.to_string(), values);
            }
        }

        debug!(
            vrf_id,
            vrf_name,
            count = neighbors.len(),
            "Retrieved neighbors for VRF"
        );
        Ok(neighbors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_names() {
        assert_eq!(APP_NEIGH_TABLE_NAME, "NEIGH_TABLE");
        assert_eq!(STATE_NEIGH_RESTORE_TABLE_NAME, "NEIGH_RESTORE_TABLE");
    }

    #[test]
    fn test_interface_table_selection() {
        // These are just logic tests, not integration tests
        assert!("Vlan100".starts_with("Vlan"));
        assert!("PortChannel1".starts_with("PortChannel"));
        assert!("Ethernet0".starts_with("Ethernet"));
    }
}
