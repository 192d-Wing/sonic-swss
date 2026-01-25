//! Redis integration tests for neighsyncd
//!
//! Tests Redis operations with actual Redis instance using testcontainers.
//! These tests are marked with #[ignore] and require Docker to run.
//!
//! Run with: cargo test --test redis_integration_tests -- --ignored

mod redis_helper;

use redis_helper::RedisTestEnv;
use std::time::Duration;

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_redis_connection() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    // Verify we can get a connection
    let mut conn = env
        .get_async_connection()
        .await
        .expect("Failed to get connection");

    // Test PING
    let pong: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .expect("Failed to ping");
    assert_eq!(pong, "PONG");
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_redis_connection_retry() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    // Multiple connections should work
    for _ in 0..5 {
        let _conn = env
            .get_async_connection()
            .await
            .expect("Failed to get connection");
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_neighbor_crud_operations() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    // Clean slate
    env.flush_all().await.expect("Failed to flush");

    // Create neighbor entry (using hash for neighbor attributes)
    let neighbor_key = "NEIGH_TABLE:Vlan100:2001:db8::1";
    env.hset(neighbor_key, "neigh", "00:11:22:33:44:55")
        .await
        .expect("Failed to create neighbor");
    env.hset(neighbor_key, "family", "IPv6")
        .await
        .expect("Failed to set family");

    // Read neighbor
    let mac = env
        .hget(neighbor_key, "neigh")
        .await
        .expect("Failed to get MAC");
    assert_eq!(mac, Some("00:11:22:33:44:55".to_string()));

    // Update neighbor
    env.hset(neighbor_key, "neigh", "aa:bb:cc:dd:ee:ff")
        .await
        .expect("Failed to update neighbor");
    let mac = env
        .hget(neighbor_key, "neigh")
        .await
        .expect("Failed to get MAC");
    assert_eq!(mac, Some("aa:bb:cc:dd:ee:ff".to_string()));

    // Delete neighbor
    env.del(neighbor_key)
        .await
        .expect("Failed to delete neighbor");
    let exists = env
        .exists(neighbor_key)
        .await
        .expect("Failed to check exists");
    assert!(!exists);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_neighbor_batch_operations() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Create multiple neighbors
    let neighbors = vec![
        ("NEIGH_TABLE:eth0:2001:db8::1", "00:11:22:33:44:55"),
        ("NEIGH_TABLE:eth0:2001:db8::2", "00:11:22:33:44:56"),
        ("NEIGH_TABLE:eth0:2001:db8::3", "00:11:22:33:44:57"),
        ("NEIGH_TABLE:eth1:2001:db8::4", "00:11:22:33:44:58"),
        ("NEIGH_TABLE:eth1:2001:db8::5", "00:11:22:33:44:59"),
    ];

    for (key, mac) in &neighbors {
        env.hset(key, "neigh", mac)
            .await
            .expect("Failed to create neighbor");
    }

    // Verify count
    let count = env.dbsize().await.expect("Failed to get dbsize");
    assert_eq!(count, 5);

    // Verify pattern matching
    let eth0_keys = env
        .keys("NEIGH_TABLE:eth0:*")
        .await
        .expect("Failed to get keys");
    assert_eq!(eth0_keys.len(), 3);

    let eth1_keys = env
        .keys("NEIGH_TABLE:eth1:*")
        .await
        .expect("Failed to get keys");
    assert_eq!(eth1_keys.len(), 2);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_concurrent_operations() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Spawn multiple concurrent tasks
    let mut handles = vec![];

    for i in 0..10 {
        let env_clone = RedisTestEnv::start()
            .await
            .expect("Failed to start Redis for task");

        let handle = tokio::spawn(async move {
            let key = format!("NEIGH_TABLE:eth0:2001:db8::{}", i);
            let mac = format!("00:11:22:33:44:{:02x}", i);

            env_clone
                .hset(&key, "neigh", &mac)
                .await
                .expect("Failed to set neighbor");

            // Verify immediately
            let read_mac = env_clone
                .hget(&key, "neigh")
                .await
                .expect("Failed to get neighbor");
            assert_eq!(read_mac, Some(mac));
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.expect("Task panicked");
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_neighbor_deletion_scenarios() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Create neighbor
    let key = "NEIGH_TABLE:eth0:2001:db8::1";
    env.hset(key, "neigh", "00:11:22:33:44:55")
        .await
        .expect("Failed to create");

    // Delete non-existent field (should not error)
    env.hdel(key, "nonexistent")
        .await
        .expect("Failed to delete field");

    // Delete entire key
    env.del(key).await.expect("Failed to delete key");

    // Delete already deleted key (idempotent)
    env.del(key).await.expect("Failed to delete key again");

    // Verify it's gone
    let exists = env.exists(key).await.expect("Failed to check exists");
    assert!(!exists);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_neighbor_interface_patterns() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Different interface types
    let interfaces = [
        "Ethernet0",
        "Ethernet1",
        "Vlan100",
        "Vlan200",
        "PortChannel0",
        "PortChannel1",
    ];

    for (idx, interface) in interfaces.iter().enumerate() {
        let key = format!("NEIGH_TABLE:{}:2001:db8::{}", interface, idx);
        let mac = format!("00:11:22:33:44:{:02x}", idx);
        env.hset(&key, "neigh", &mac)
            .await
            .expect("Failed to create neighbor");
    }

    // Query by interface type
    let ethernet_keys = env
        .keys("NEIGH_TABLE:Ethernet*")
        .await
        .expect("Failed to get keys");
    assert_eq!(ethernet_keys.len(), 2);

    let vlan_keys = env
        .keys("NEIGH_TABLE:Vlan*")
        .await
        .expect("Failed to get keys");
    assert_eq!(vlan_keys.len(), 2);

    let portchannel_keys = env
        .keys("NEIGH_TABLE:PortChannel*")
        .await
        .expect("Failed to get keys");
    assert_eq!(portchannel_keys.len(), 2);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_neighbor_attributes() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    let key = "NEIGH_TABLE:eth0:2001:db8::1";

    // Set multiple attributes
    env.hset(key, "neigh", "00:11:22:33:44:55")
        .await
        .expect("Failed to set MAC");
    env.hset(key, "family", "IPv6")
        .await
        .expect("Failed to set family");

    // Get all attributes
    let all = env.hgetall(key).await.expect("Failed to get all");
    assert_eq!(all.len(), 2);

    // Verify specific attributes
    let mac = env.hget(key, "neigh").await.expect("Failed to get MAC");
    assert_eq!(mac, Some("00:11:22:33:44:55".to_string()));

    let family = env.hget(key, "family").await.expect("Failed to get family");
    assert_eq!(family, Some("IPv6".to_string()));
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_error_handling_invalid_operations() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Getting non-existent key should return None, not error
    let result = env.get("nonexistent").await.expect("Failed to get");
    assert_eq!(result, None);

    // Getting non-existent hash field should return None
    let result = env
        .hget("nonexistent", "field")
        .await
        .expect("Failed to hget");
    assert_eq!(result, None);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_neighbor_update_scenarios() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    let key = "NEIGH_TABLE:eth0:2001:db8::1";

    // Initial state: Incomplete
    env.hset(key, "neigh", "00:00:00:00:00:00")
        .await
        .expect("Failed to set");
    env.hset(key, "state", "Incomplete")
        .await
        .expect("Failed to set state");

    // Update to Reachable
    env.hset(key, "neigh", "00:11:22:33:44:55")
        .await
        .expect("Failed to update MAC");
    env.hset(key, "state", "Reachable")
        .await
        .expect("Failed to update state");

    // Verify final state
    let mac = env.hget(key, "neigh").await.expect("Failed to get MAC");
    assert_eq!(mac, Some("00:11:22:33:44:55".to_string()));

    let state = env.hget(key, "state").await.expect("Failed to get state");
    assert_eq!(state, Some("Reachable".to_string()));
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_large_batch_operations() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Create 1000 neighbors
    let count = 1000;
    for i in 0..count {
        let key = format!("NEIGH_TABLE:eth0:2001:db8::{:x}", i);
        let mac = format!("00:11:22:33:{:02x}:{:02x}", (i >> 8) & 0xff, i & 0xff);
        env.hset(&key, "neigh", &mac)
            .await
            .expect("Failed to create neighbor");
    }

    // Verify count
    let dbsize = env.dbsize().await.expect("Failed to get dbsize");
    assert_eq!(dbsize, count);

    // Pattern match should work with large sets
    let all_keys = env.keys("NEIGH_TABLE:*").await.expect("Failed to get keys");
    assert_eq!(all_keys.len(), count);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_connection_timeout_handling() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    // Get connection
    let mut conn = env
        .get_async_connection()
        .await
        .expect("Failed to get connection");

    // Perform operation with reasonable timeout
    let cmd = redis::cmd("PING");
    tokio::select! {
        result = cmd.query_async::<String>(&mut conn) => {
            let pong = result.expect("Failed to ping");
            assert_eq!(pong, "PONG");
        }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            panic!("Operation timed out");
        }
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_ipv6_address_formats() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Various IPv6 address formats
    let addresses = vec![
        "2001:db8::1",           // Standard
        "fe80::1",               // Link-local
        "::1",                   // Loopback
        "2001:db8:0:0:0:0:0:1",  // Expanded
        "2001:db8::192.168.1.1", // IPv4-mapped
        "ff02::1",               // Multicast
    ];

    for addr in addresses {
        let key = format!("NEIGH_TABLE:eth0:{}", addr);
        env.hset(&key, "neigh", "00:11:22:33:44:55")
            .await
            .expect("Failed to create neighbor");

        // Verify retrieval
        let mac = env
            .hget(&key, "neigh")
            .await
            .expect("Failed to get neighbor");
        assert_eq!(mac, Some("00:11:22:33:44:55".to_string()));
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_dual_tor_scenarios() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Incomplete neighbor with zero MAC (dual-ToR)
    let key = "NEIGH_TABLE:Vlan1000:2001:db8::1";
    env.hset(key, "neigh", "00:00:00:00:00:00")
        .await
        .expect("Failed to set");
    env.hset(key, "state", "Incomplete")
        .await
        .expect("Failed to set state");

    // Verify zero MAC is stored
    let mac = env.hget(key, "neigh").await.expect("Failed to get MAC");
    assert_eq!(mac, Some("00:00:00:00:00:00".to_string()));
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_redis_persistence() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Create neighbor
    let key = "NEIGH_TABLE:eth0:2001:db8::1";
    env.hset(key, "neigh", "00:11:22:33:44:55")
        .await
        .expect("Failed to create");

    // Create new connection and verify data persists
    let mut new_conn = env
        .get_async_connection()
        .await
        .expect("Failed to get new connection");

    let mac: Option<String> = redis::cmd("HGET")
        .arg(key)
        .arg("neigh")
        .query_async(&mut new_conn)
        .await
        .expect("Failed to get MAC");

    assert_eq!(mac, Some("00:11:22:33:44:55".to_string()));
}
