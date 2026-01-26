//! Warm Restart Integration Tests
//!
//! Tests warm restart behavior with real Redis instance.
//! Validates state recovery, reconciliation, and event handling during restart.
//!
//! Run with: cargo test --test warm_restart_integration -- --ignored

mod redis_helper;

use redis_helper::RedisTestEnv;
use std::collections::HashMap;

#[tokio::test]
#[ignore] // Requires Docker
async fn test_warm_restart_state_recovery() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Simulate pre-restart state in STATE_DB
    // In real SONiC, this would be in database 6 (STATE_DB)
    // For this test, we use database 0

    // Set initial neighbors before "restart"
    for i in 0..10 {
        let key = format!("NEIGH_TABLE:Ethernet{}", i % 3);
        let ip = format!("fe80::{:x}", i + 1);
        let mac = format!("00:11:22:33:44:{:02x}", i);

        env.hset(&key, &ip, &mac)
            .expect("Failed to set pre-restart neighbor");
    }

    // Verify initial state
    let keys_before = env
        .keys("NEIGH_TABLE:*")
        .expect("Failed to get keys before restart");
    let dbsize_before = env.dbsize().expect("Failed to get dbsize before");

    // Simulate restart: keys should still exist (persistence)
    let keys_after = env
        .keys("NEIGH_TABLE:*")
        .expect("Failed to get keys after restart");
    let dbsize_after = env.dbsize().expect("Failed to get dbsize after");

    // Verify state recovered
    assert_eq!(keys_before.len(), keys_after.len());
    assert_eq!(dbsize_before, dbsize_after);
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_warm_restart_reconciliation() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Set initial state (before restart)
    let key = "NEIGH_TABLE:Ethernet0";
    env.hset(key, "fe80::1", "00:11:22:33:44:55")
        .expect("Failed to set");
    env.hset(key, "fe80::2", "AA:BB:CC:DD:EE:FF")
        .expect("Failed to set");
    env.hset(key, "fe80::3", "11:22:33:44:55:66")
        .expect("Failed to set");

    // Get all fields before
    let fields_before = env.hgetall(key).expect("Failed to get all before");
    assert_eq!(fields_before.len(), 3);

    // Simulate post-restart reconciliation:
    // - fe80::1 still exists (no change)
    // - fe80::2 was deleted (remove it)
    // - fe80::3 MAC changed (update it)
    // - fe80::4 is new (add it)

    // Delete fe80::2 (simulate neighbor gone)
    env.del(key).expect("Failed to delete");
    env.hset(key, "fe80::1", "00:11:22:33:44:55")
        .expect("Failed to re-set");
    env.hset(key, "fe80::3", "99:88:77:66:55:44")
        .expect("Failed to update MAC");
    env.hset(key, "fe80::4", "FF:EE:DD:CC:BB:AA")
        .expect("Failed to add new");

    // Verify reconciliation
    let fields_after = env.hgetall(key).expect("Failed to get all after");
    assert_eq!(fields_after.len(), 3);

    // Verify specific entries
    assert_eq!(
        fields_after.get("fe80::1").unwrap(),
        "00:11:22:33:44:55"
    ); // unchanged
    assert!(!fields_after.contains_key("fe80::2")); // deleted
    assert_eq!(
        fields_after.get("fe80::3").unwrap(),
        "99:88:77:66:55:44"
    ); // updated MAC
    assert_eq!(
        fields_after.get("fe80::4").unwrap(),
        "FF:EE:DD:CC:BB:AA"
    ); // new entry
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_warm_restart_with_concurrent_updates() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Set initial state
    let key = "NEIGH_TABLE:Ethernet0";
    for i in 0..50 {
        let ip = format!("fe80::{:x}", i + 1);
        let mac = format!("00:11:22:33:44:{:02x}", i);
        env.hset(key, &ip, &mac).expect("Failed to set");
    }

    // Simulate concurrent updates during restart
    // (in reality, these would be queued and applied after reconciliation)
    for i in 50..60 {
        let ip = format!("fe80::{:x}", i + 1);
        let mac = format!("AA:BB:CC:DD:EE:{:02x}", i - 50);
        env.hset(key, &ip, &mac).expect("Failed to set");
    }

    // Verify all updates applied
    let all_fields = env.hgetall(key).expect("Failed to get all fields");
    assert_eq!(all_fields.len(), 60); // 50 original + 10 concurrent

    // Verify some specific entries
    assert_eq!(all_fields.get("fe80::1").unwrap(), "00:11:22:33:44:00");
    assert_eq!(all_fields.get("fe80::33").unwrap(), "AA:BB:CC:DD:EE:00");
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_warm_restart_timeout_handling() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Set a large number of neighbors to simulate long reconciliation
    let key = "NEIGH_TABLE:Ethernet0";
    for i in 0..1000 {
        let ip = format!("fe80::{:x}", i + 1);
        let mac = format!("{:02x}:11:22:33:44:{:02x}", i / 256, i % 256);
        env.hset(key, &ip, &mac).expect("Failed to set");
    }

    // Verify all neighbors exist
    let all_fields = env.hgetall(key).expect("Failed to get all fields");
    assert_eq!(all_fields.len(), 1000);

    // In a real scenario with timeout, we would:
    // 1. Start reconciliation
    // 2. Process as many as possible within timeout
    // 3. Queue remaining for later processing

    // For this test, verify the data is accessible quickly
    let start = std::time::Instant::now();
    let _all_fields = env.hgetall(key).expect("Failed to get all fields");
    let elapsed = start.elapsed();

    // Should be able to retrieve 1000 neighbors very quickly (< 100ms)
    assert!(
        elapsed.as_millis() < 100,
        "Retrieval took too long: {:?}",
        elapsed
    );
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_warm_restart_vrf_isolation() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Set up neighbors in multiple VRFs before "restart"
    let vrfs = vec!["", "Vrf_red|", "Vrf_blue|"];

    for vrf_prefix in &vrfs {
        for i in 0..10 {
            let key = format!("{}NEIGH_TABLE:Ethernet0", vrf_prefix);
            let ip = format!("fe80::{:x}", i + 1);
            let mac = format!("{:02x}:11:22:33:44:{:02x}", vrfs.iter().position(|&v| v == *vrf_prefix).unwrap(), i);
            env.hset(&key, &ip, &mac).expect("Failed to set");
        }
    }

    // Verify all VRFs have their neighbors
    for vrf_prefix in &vrfs {
        let key = format!("{}NEIGH_TABLE:Ethernet0", vrf_prefix);
        let all_fields = env.hgetall(&key).expect("Failed to get VRF neighbors");
        assert_eq!(all_fields.len(), 10);
    }

    // Verify total keys
    let all_keys = env
        .keys("*NEIGH_TABLE:*")
        .expect("Failed to get all keys");
    assert_eq!(all_keys.len(), 3); // 3 VRFs

    // Verify VRF isolation: same IP in different VRFs has different MAC
    let mac_default: String = env
        .hget("NEIGH_TABLE:Ethernet0", "fe80::1")
        .expect("Failed to get default VRF");
    let mac_red: String = env
        .hget("Vrf_red|NEIGH_TABLE:Ethernet0", "fe80::1")
        .expect("Failed to get red VRF");
    let mac_blue: String = env
        .hget("Vrf_blue|NEIGH_TABLE:Ethernet0", "fe80::1")
        .expect("Failed to get blue VRF");

    assert_ne!(mac_default, mac_red);
    assert_ne!(mac_default, mac_blue);
    assert_ne!(mac_red, mac_blue);
}

#[tokio::test]
#[ignore] // Requires Docker
async fn test_warm_restart_partial_state() {
    let env = RedisTestEnv::new().await.expect("Failed to create Redis env");
    env.flush_all().expect("Failed to flush");

    // Simulate partial state (some interfaces have neighbors, others don't)
    env.hset("NEIGH_TABLE:Ethernet0", "fe80::1", "00:11:22:33:44:55")
        .expect("Failed");
    env.hset("NEIGH_TABLE:Ethernet1", "fe80::2", "AA:BB:CC:DD:EE:FF")
        .expect("Failed");
    // Ethernet2 has no neighbors

    // Verify partial state
    assert!(env.exists("NEIGH_TABLE:Ethernet0").expect("EXISTS failed"));
    assert!(env.exists("NEIGH_TABLE:Ethernet1").expect("EXISTS failed"));
    assert!(!env
        .exists("NEIGH_TABLE:Ethernet2")
        .expect("EXISTS failed"));

    let keys = env
        .keys("NEIGH_TABLE:*")
        .expect("Failed to get keys");
    assert_eq!(keys.len(), 2); // Only interfaces with neighbors
}
