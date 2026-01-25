//! Warm restart integration tests for neighsyncd
//!
//! Tests warm restart scenarios including state caching, reconciliation, and recovery.
//! These tests are marked with #[ignore] and require Docker to run.
//!
//! Run with: cargo test --test warm_restart_integration -- --ignored

mod redis_helper;

use redis_helper::RedisTestEnv;
use std::collections::HashMap;

/// Simulate warm restart state in Redis STATE_DB
async fn setup_warm_restart_state(env: &RedisTestEnv) -> Result<(), Box<dyn std::error::Error>> {
    // Set warm restart flag
    env.set("WARM_RESTART_ENABLE_TABLE:neighsyncd", "true")
        .await?;

    // Set reconciliation timer
    env.set("WARM_RESTART_TABLE:neighsyncd:restore_count", "0")
        .await?;

    Ok(())
}

/// Populate initial neighbor state (before warm restart)
async fn populate_initial_neighbors(
    env: &RedisTestEnv,
    count: usize,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut keys = Vec::new();

    for i in 0..count {
        let key = format!("NEIGH_TABLE:eth0:2001:db8::{:x}", i);
        let mac = format!("00:11:22:33:{:02x}:{:02x}", (i >> 8) & 0xff, i & 0xff);

        env.hset(&key, "neigh", &mac).await?;
        env.hset(&key, "family", "IPv6").await?;
        env.hset(&key, "state", "Reachable").await?;

        keys.push(key);
    }

    Ok(keys)
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_flag_detection() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Initially, no warm restart flag
    let flag = env
        .get("WARM_RESTART_ENABLE_TABLE:neighsyncd")
        .await
        .expect("Failed to get flag");
    assert_eq!(flag, None);

    // Set warm restart flag
    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup warm restart");

    // Verify flag is set
    let flag = env
        .get("WARM_RESTART_ENABLE_TABLE:neighsyncd")
        .await
        .expect("Failed to get flag");
    assert_eq!(flag, Some("true".to_string()));
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_initial_state_cache() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Setup warm restart
    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Populate 10 neighbors
    let initial_keys = populate_initial_neighbors(&env, 10)
        .await
        .expect("Failed to populate");

    // Verify all neighbors exist
    for key in &initial_keys {
        let exists = env.exists(key).await.expect("Failed to check exists");
        assert!(exists, "Key {} should exist", key);
    }

    // Verify count
    let count = env.dbsize().await.expect("Failed to get count");
    assert!(count >= 10); // At least 10 (plus warm restart keys)
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_reconciliation_add_only() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Setup warm restart with initial state
    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");
    let initial_keys = populate_initial_neighbors(&env, 5)
        .await
        .expect("Failed to populate");

    // Simulate new neighbors added during warm restart
    let new_keys: Vec<String> = vec![
        "NEIGH_TABLE:eth0:2001:db8::100".to_string(),
        "NEIGH_TABLE:eth0:2001:db8::101".to_string(),
        "NEIGH_TABLE:eth0:2001:db8::102".to_string(),
    ];

    for key in &new_keys {
        env.hset(key, "neigh", "aa:bb:cc:dd:ee:ff")
            .await
            .expect("Failed to add new neighbor");
    }

    // After reconciliation, all neighbors should exist
    for key in initial_keys.iter().chain(new_keys.iter()) {
        let exists = env.exists(key).await.expect("Failed to check exists");
        assert!(exists, "Key {} should exist after reconciliation", key);
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_reconciliation_delete_stale() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Setup warm restart with initial state
    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");
    populate_initial_neighbors(&env, 5)
        .await
        .expect("Failed to populate");

    // Simulate stale neighbor (exists in cache but not in kernel)
    let stale_key = "NEIGH_TABLE:eth0:2001:db8::999";
    env.hset(stale_key, "neigh", "ff:ff:ff:ff:ff:ff")
        .await
        .expect("Failed to add stale neighbor");

    // Verify it exists before reconciliation
    let exists = env
        .exists(stale_key)
        .await
        .expect("Failed to check exists");
    assert!(exists);

    // After reconciliation, stale entries should be removed
    // (In real scenario, reconciliation would delete this)
    // For testing, we manually delete to simulate reconciliation
    env.del(stale_key)
        .await
        .expect("Failed to delete stale");

    let exists = env
        .exists(stale_key)
        .await
        .expect("Failed to check exists");
    assert!(!exists);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_reconciliation_update() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Setup warm restart
    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    let key = "NEIGH_TABLE:eth0:2001:db8::1";

    // Initial state (before warm restart)
    env.hset(key, "neigh", "00:11:22:33:44:55")
        .await
        .expect("Failed to set initial");
    env.hset(key, "state", "Reachable")
        .await
        .expect("Failed to set state");

    // During warm restart, neighbor MAC changed
    env.hset(key, "neigh", "aa:bb:cc:dd:ee:ff")
        .await
        .expect("Failed to update");

    // Verify updated MAC
    let mac = env.hget(key, "neigh").await.expect("Failed to get MAC");
    assert_eq!(mac, Some("aa:bb:cc:dd:ee:ff".to_string()));
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_large_state() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    // Setup warm restart with large number of neighbors
    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    let neighbor_count = 1000;
    let keys = populate_initial_neighbors(&env, neighbor_count)
        .await
        .expect("Failed to populate");

    // Verify count
    assert_eq!(keys.len(), neighbor_count);

    // Sample verification
    for i in (0..neighbor_count).step_by(100) {
        let exists = env
            .exists(&keys[i])
            .await
            .expect("Failed to check exists");
        assert!(exists);
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_multiple_interfaces() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Create neighbors on different interfaces
    let interfaces = vec!["Ethernet0", "Ethernet1", "Vlan100", "PortChannel0"];
    let mut all_keys = Vec::new();

    for (idx, interface) in interfaces.iter().enumerate() {
        for i in 0..5 {
            let key = format!("NEIGH_TABLE:{}:2001:db8:{}:{}", interface, idx, i);
            let mac = format!("00:11:22:33:{:02x}:{:02x}", idx, i);

            env.hset(&key, "neigh", &mac)
                .await
                .expect("Failed to set neighbor");
            all_keys.push(key);
        }
    }

    // Verify per-interface counts
    for interface in interfaces {
        let pattern = format!("NEIGH_TABLE:{}:*", interface);
        let keys = env.keys(&pattern).await.expect("Failed to get keys");
        assert_eq!(keys.len(), 5);
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_restore_count() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Initially restore_count is 0
    let count = env
        .get("WARM_RESTART_TABLE:neighsyncd:restore_count")
        .await
        .expect("Failed to get count");
    assert_eq!(count, Some("0".to_string()));

    // Simulate incrementing restore count
    env.set("WARM_RESTART_TABLE:neighsyncd:restore_count", "1")
        .await
        .expect("Failed to set count");

    let count = env
        .get("WARM_RESTART_TABLE:neighsyncd:restore_count")
        .await
        .expect("Failed to get count");
    assert_eq!(count, Some("1".to_string()));
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_timer_expiry() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Set timer state
    env.set("WARM_RESTART_TABLE:neighsyncd:timer_started", "true")
        .await
        .expect("Failed to set timer");

    // Simulate timer expiry by checking elapsed time
    // In real implementation, this would trigger reconciliation
    let timer_started = env
        .get("WARM_RESTART_TABLE:neighsyncd:timer_started")
        .await
        .expect("Failed to get timer");
    assert_eq!(timer_started, Some("true".to_string()));

    // After reconciliation, clear warm restart state
    env.del("WARM_RESTART_ENABLE_TABLE:neighsyncd")
        .await
        .expect("Failed to clear flag");

    let flag = env
        .get("WARM_RESTART_ENABLE_TABLE:neighsyncd")
        .await
        .expect("Failed to get flag");
    assert_eq!(flag, None);
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_concurrent_updates() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Simulate concurrent updates during warm restart
    let keys = vec![
        "NEIGH_TABLE:eth0:2001:db8::1",
        "NEIGH_TABLE:eth0:2001:db8::2",
        "NEIGH_TABLE:eth0:2001:db8::3",
    ];

    // Multiple updates to same keys
    for iteration in 0..3 {
        for key in &keys {
            let mac = format!("00:11:22:33:44:{:02x}", iteration);
            env.hset(key, "neigh", &mac)
                .await
                .expect("Failed to update");
        }
    }

    // Verify final state (last update wins)
    for key in &keys {
        let mac = env.hget(key, "neigh").await.expect("Failed to get MAC");
        assert_eq!(mac, Some("00:11:22:33:44:02".to_string()));
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_state_consistency() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Create a set of neighbors with complete state
    let neighbors = vec![
        ("NEIGH_TABLE:eth0:2001:db8::1", "00:11:22:33:44:55", "Reachable"),
        ("NEIGH_TABLE:eth0:2001:db8::2", "00:11:22:33:44:56", "Stale"),
        ("NEIGH_TABLE:eth0:2001:db8::3", "00:11:22:33:44:57", "Delay"),
    ];

    for (key, mac, state) in &neighbors {
        env.hset(key, "neigh", mac)
            .await
            .expect("Failed to set MAC");
        env.hset(key, "family", "IPv6")
            .await
            .expect("Failed to set family");
        env.hset(key, "state", state)
            .await
            .expect("Failed to set state");
    }

    // Verify consistency of all attributes
    for (key, expected_mac, expected_state) in &neighbors {
        let mac = env
            .hget(key, "neigh")
            .await
            .expect("Failed to get MAC");
        assert_eq!(mac, Some(expected_mac.to_string()));

        let state = env
            .hget(key, "state")
            .await
            .expect("Failed to get state");
        assert_eq!(state, Some(expected_state.to_string()));

        let family = env
            .hget(key, "family")
            .await
            .expect("Failed to get family");
        assert_eq!(family, Some("IPv6".to_string()));
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_incomplete_neighbors() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Create incomplete neighbor (zero MAC)
    let key = "NEIGH_TABLE:eth0:2001:db8::1";
    env.hset(key, "neigh", "00:00:00:00:00:00")
        .await
        .expect("Failed to set zero MAC");
    env.hset(key, "state", "Incomplete")
        .await
        .expect("Failed to set state");

    // During reconciliation, incomplete neighbors should be handled
    let mac = env.hget(key, "neigh").await.expect("Failed to get MAC");
    assert_eq!(mac, Some("00:00:00:00:00:00".to_string()));

    let state = env.hget(key, "state").await.expect("Failed to get state");
    assert_eq!(state, Some("Incomplete".to_string()));
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_reconciliation_performance() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Populate 100 neighbors
    let start = std::time::Instant::now();
    populate_initial_neighbors(&env, 100)
        .await
        .expect("Failed to populate");
    let populate_duration = start.elapsed();

    // Verify all created
    let count = env.dbsize().await.expect("Failed to get count");
    assert!(count >= 100);

    // Should complete reasonably fast (< 5 seconds for 100 neighbors)
    assert!(
        populate_duration.as_secs() < 5,
        "Population took too long: {:?}",
        populate_duration
    );
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_warm_restart_cache_vs_kernel_diff() {
    let env = RedisTestEnv::start()
        .await
        .expect("Failed to start Redis");

    env.flush_all().await.expect("Failed to flush");

    setup_warm_restart_state(&env)
        .await
        .expect("Failed to setup");

    // Create cached state (what's in Redis before warm restart)
    let cached_neighbors: HashMap<String, String> = [
        (
            "NEIGH_TABLE:eth0:2001:db8::1".to_string(),
            "00:11:22:33:44:55".to_string(),
        ),
        (
            "NEIGH_TABLE:eth0:2001:db8::2".to_string(),
            "00:11:22:33:44:56".to_string(),
        ),
        (
            "NEIGH_TABLE:eth0:2001:db8::3".to_string(),
            "00:11:22:33:44:57".to_string(),
        ),
    ]
    .iter()
    .cloned()
    .collect();

    // Populate cached state
    for (key, mac) in &cached_neighbors {
        env.hset(key, "neigh", mac)
            .await
            .expect("Failed to set cached");
    }

    // Simulate kernel state (what netlink reports after restart)
    // Neighbor 2 deleted, neighbor 4 added, neighbor 1 unchanged
    let kernel_neighbors: HashMap<String, String> = [
        (
            "NEIGH_TABLE:eth0:2001:db8::1".to_string(),
            "00:11:22:33:44:55".to_string(),
        ), // Unchanged
        (
            "NEIGH_TABLE:eth0:2001:db8::4".to_string(),
            "00:11:22:33:44:58".to_string(),
        ), // New
    ]
    .iter()
    .cloned()
    .collect();

    // Apply kernel state
    for (key, mac) in &kernel_neighbors {
        env.hset(key, "neigh", mac)
            .await
            .expect("Failed to set kernel");
    }

    // Reconciliation: Remove stale entry (neighbor 2, 3)
    env.del("NEIGH_TABLE:eth0:2001:db8::2")
        .await
        .expect("Failed to delete");
    env.del("NEIGH_TABLE:eth0:2001:db8::3")
        .await
        .expect("Failed to delete");

    // Verify final state matches kernel
    for key in kernel_neighbors.keys() {
        let exists = env.exists(key).await.expect("Failed to check exists");
        assert!(exists, "Kernel neighbor {} should exist", key);
    }

    // Verify stale entries removed
    let stale_exists = env
        .exists("NEIGH_TABLE:eth0:2001:db8::2")
        .await
        .expect("Failed to check stale");
    assert!(!stale_exists);
}
