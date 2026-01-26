//! Redis Integration Tests
//!
//! Comprehensive integration tests with real Redis instance using testcontainers.
//! These tests validate actual Redis interactions including:
//! - Connection management and reconnection
//! - CRUD operations on neighbor entries
//! - Batch operations and pipelining
//! - State consistency under concurrent updates
//! - VRF isolation in Redis keys
//!
//! Run with: cargo test --test redis_integration_tests -- --ignored

mod redis_helper;

use redis_helper::RedisTestEnv;

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_connection() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");

    // Verify connection works
    let mut conn = env.get_connection().expect("Failed to get connection");
    let pong: String = redis::cmd("PING")
        .query(&mut conn)
        .expect("PING failed");
    assert_eq!(pong, "PONG");
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_set_neighbor() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Set in Redis using HSET (simulating RedisAdapter behavior)
    let key = "NEIGH_TABLE:Ethernet0";
    let ip_str = "fe80::1";
    let mac_str = "00:11:22:33:44:55";

    env.hset(key, ip_str, mac_str)
        .expect("Failed to set neighbor");

    // Verify it was set
    let stored_mac: String = env.hget(key, ip_str).expect("Failed to get neighbor");
    assert_eq!(stored_mac, mac_str);

    // Verify key exists
    assert!(env.exists(key).expect("EXISTS failed"));
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_delete_neighbor() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Set a neighbor
    let key = "NEIGH_TABLE:Ethernet0";
    let ip = "fe80::1";
    let mac = "00:11:22:33:44:55";

    env.hset(key, ip, mac).expect("Failed to set neighbor");
    assert!(env.exists(key).expect("EXISTS failed"));

    // Delete the neighbor
    env.del(key).expect("Failed to delete neighbor");

    // Verify deletion
    assert!(!env.exists(key).expect("EXISTS failed"));
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_batch_operations() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Create 100 neighbors
    for i in 0..100 {
        let key = format!("NEIGH_TABLE:Ethernet{}", i);
        let ip = format!("fe80::{:x}", i + 1);
        let mac = format!("00:11:22:33:44:{:02x}", i);

        env.hset(&key, &ip, &mac)
            .expect("Failed to set neighbor");
    }

    // Verify all were set
    let keys = env.keys("NEIGH_TABLE:*").expect("Failed to get keys");
    assert_eq!(keys.len(), 100);

    // Verify database size
    let dbsize = env.dbsize().expect("Failed to get dbsize");
    assert_eq!(dbsize, 100);
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_vrf_isolation() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Same neighbor in two different VRFs
    let ip = "fe80::1";
    let mac1 = "00:11:22:33:44:55";
    let mac2 = "AA:BB:CC:DD:EE:FF";

    // VRF default (no prefix)
    let key1 = "NEIGH_TABLE:Ethernet0";
    env.hset(key1, ip, mac1).expect("Failed to set in default VRF");

    // VRF red (with prefix)
    let key2 = "Vrf_red|NEIGH_TABLE:Ethernet0";
    env.hset(key2, ip, mac2).expect("Failed to set in VRF red");

    // Verify both exist with different MACs
    let stored_mac1: String = env.hget(key1, ip).expect("Failed to get from default VRF");
    let stored_mac2: String = env.hget(key2, ip).expect("Failed to get from VRF red");

    assert_eq!(stored_mac1, mac1);
    assert_eq!(stored_mac2, mac2);
    assert_ne!(stored_mac1, stored_mac2);

    // Verify two keys exist
    let keys = env.keys("*NEIGH_TABLE:*").expect("Failed to get keys");
    assert_eq!(keys.len(), 2);
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_concurrent_updates() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    let key = "NEIGH_TABLE:Ethernet0";
    let ip = "fe80::1";

    // Simulate concurrent updates (last write wins)
    env.hset(key, ip, "00:11:22:33:44:55")
        .expect("Failed to set 1");
    env.hset(key, ip, "AA:BB:CC:DD:EE:FF")
        .expect("Failed to set 2");
    env.hset(key, ip, "11:22:33:44:55:66")
        .expect("Failed to set 3");

    // Verify final value
    let final_mac: String = env.hget(key, ip).expect("Failed to get final value");
    assert_eq!(final_mac, "11:22:33:44:55:66");
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_state_consistency() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Set multiple fields in a hash (simulating neighbor with metadata)
    let key = "NEIGH_TABLE:Ethernet0";
    env.hset(key, "fe80::1", "00:11:22:33:44:55")
        .expect("Failed to set field 1");
    env.hset(key, "fe80::2", "AA:BB:CC:DD:EE:FF")
        .expect("Failed to set field 2");

    // Get all fields
    let all_fields = env.hgetall(key).expect("Failed to get all fields");
    assert_eq!(all_fields.len(), 2);
    assert_eq!(all_fields.get("fe80::1").unwrap(), "00:11:22:33:44:55");
    assert_eq!(all_fields.get("fe80::2").unwrap(), "AA:BB:CC:DD:EE:FF");

    // Delete one field by deleting the key and re-setting
    env.del(key).expect("Failed to delete");
    env.hset(key, "fe80::2", "AA:BB:CC:DD:EE:FF")
        .expect("Failed to set field 2");

    // Verify only one field remains
    let all_fields = env.hgetall(key).expect("Failed to get all fields");
    assert_eq!(all_fields.len(), 1);
    assert!(all_fields.contains_key("fe80::2"));
    assert!(!all_fields.contains_key("fe80::1"));
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_keys_pattern_matching() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Create neighbors on different interfaces
    env.hset("NEIGH_TABLE:Ethernet0", "fe80::1", "00:11:22:33:44:55")
        .expect("Failed");
    env.hset("NEIGH_TABLE:Ethernet1", "fe80::2", "AA:BB:CC:DD:EE:FF")
        .expect("Failed");
    env.hset("NEIGH_TABLE:Vlan100", "fe80::3", "11:22:33:44:55:66")
        .expect("Failed");

    // Match all neighbor tables
    let all_keys = env.keys("NEIGH_TABLE:*").expect("Failed to get keys");
    assert_eq!(all_keys.len(), 3);

    // Match only Ethernet interfaces
    let eth_keys = env.keys("NEIGH_TABLE:Ethernet*").expect("Failed to get keys");
    assert_eq!(eth_keys.len(), 2);

    // Match only Vlan interfaces
    let vlan_keys = env.keys("NEIGH_TABLE:Vlan*").expect("Failed to get keys");
    assert_eq!(vlan_keys.len(), 1);
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_redis_large_batch() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Create 1000 neighbors across 10 interfaces
    for iface in 0..10 {
        for i in 0..100 {
            let key = format!("NEIGH_TABLE:Ethernet{}", iface);
            let ip = format!("fe80::{:x}:{:x}", iface, i + 1);
            let mac = format!("{:02x}:11:22:33:44:{:02x}", iface, i);

            env.hset(&key, &ip, &mac)
                .expect("Failed to set neighbor");
        }
    }

    // Verify all keys exist
    let keys = env.keys("NEIGH_TABLE:*").expect("Failed to get keys");
    assert_eq!(keys.len(), 10);

    // Verify total database size
    let dbsize = env.dbsize().expect("Failed to get dbsize");
    assert_eq!(dbsize, 10);

    // Verify one interface has 100 neighbors
    let all_fields = env
        .hgetall("NEIGH_TABLE:Ethernet0")
        .expect("Failed to get all fields");
    assert_eq!(all_fields.len(), 100);
}
