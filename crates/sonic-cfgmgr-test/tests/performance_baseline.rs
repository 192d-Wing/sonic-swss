//! Performance baseline measurements for cfgmgr daemons
//!
//! Establishes performance targets and regression testing

use sonic_cfgmgr_test::{fixtures::port_fixtures, RedisTestEnv};
use std::time::Instant;

/// Baseline: CONFIG_DB write latency
///
/// Target: <1ms per write operation
#[tokio::test]
#[ignore = "Requires Docker"]
async fn benchmark_config_db_write_latency() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    let iterations = 1000;
    let start = Instant::now();

    for i in 0..iterations {
        let port_name = format!("Ethernet{}", i);
        env.hset(&format!("PORT:{}", port_name), "mtu", "9100")
            .await
            .expect("Failed to write");
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed / iterations;

    println!(
        "CONFIG_DB write latency: {:?} (avg over {} iterations)",
        avg_latency, iterations
    );
    println!("Total time: {:?}", elapsed);

    // Assert target: average latency < 1ms
    assert!(
        avg_latency.as_micros() < 1000,
        "CONFIG_DB write latency too high: {:?}",
        avg_latency
    );
}

/// Baseline: APPL_DB read latency
///
/// Target: <1ms per read operation
#[tokio::test]
#[ignore = "Requires Docker"]
async fn benchmark_app_db_read_latency() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    // Setup: Write some data
    for i in 0..100 {
        let port_name = format!("Ethernet{}", i);
        env.hset(&format!("PORT_TABLE:{}", port_name), "mtu", "9100")
            .await
            .expect("Failed to write");
    }

    let iterations = 1000;
    let start = Instant::now();

    for i in 0..iterations {
        let port_name = format!("Ethernet{}", i % 100);
        let _value = env
            .hget(&format!("PORT_TABLE:{}", port_name), "mtu")
            .await
            .expect("Failed to read");
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed / iterations;

    println!(
        "APPL_DB read latency: {:?} (avg over {} iterations)",
        avg_latency, iterations
    );

    assert!(
        avg_latency.as_micros() < 1000,
        "APPL_DB read latency too high: {:?}",
        avg_latency
    );
}

/// Baseline: Hash field enumeration performance
///
/// Target: <5ms for 100 fields
#[tokio::test]
#[ignore = "Requires Docker"]
async fn benchmark_hgetall_performance() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    // Setup: Create hash with 100 fields
    for i in 0..100 {
        env.hset("TEST_HASH", &format!("field{}", i), &format!("value{}", i))
            .await
            .expect("Failed to write");
    }

    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let _fields = env.hgetall("TEST_HASH").await.expect("Failed to read");
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed / iterations;

    println!(
        "HGETALL latency (100 fields): {:?} (avg over {} iterations)",
        avg_latency, iterations
    );

    assert!(
        avg_latency.as_millis() < 5,
        "HGETALL latency too high: {:?}",
        avg_latency
    );
}

/// Baseline: Bulk configuration change throughput
///
/// Target: >100 operations/second
#[tokio::test]
#[ignore = "Requires Docker"]
async fn benchmark_bulk_config_throughput() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    let num_ports = 128;
    let start = Instant::now();

    // Simulate configuring 128 ports
    for i in 0..num_ports {
        let port_name = format!("Ethernet{}", i);
        let change = port_fixtures::ethernet_port_default(&port_name);

        // Write all fields
        for (field, value) in &change.fields {
            env.hset(&format!("PORT:{}", port_name), field, value)
                .await
                .expect("Failed to write");
        }
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (num_ports as f64) / elapsed.as_secs_f64();

    println!("Bulk config throughput: {:.2} ports/sec", ops_per_sec);
    println!("Total time for {} ports: {:?}", num_ports, elapsed);

    // Assert target: >100 ports/second
    assert!(
        ops_per_sec > 100.0,
        "Bulk config throughput too low: {:.2} ops/sec",
        ops_per_sec
    );
}

/// Baseline: Memory usage estimation
///
/// Target: <50MB for 1000 port configurations
#[tokio::test]
#[ignore = "Requires Docker"]
async fn benchmark_memory_usage() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    // Configure 1000 ports
    for i in 0..1000 {
        let port_name = format!("Ethernet{}", i);
        let change = port_fixtures::ethernet_port_default(&port_name);

        for (field, value) in &change.fields {
            env.hset(&format!("PORT:{}", port_name), field, value)
                .await
                .expect("Failed to write");
        }
    }

    // Get database size
    let dbsize = env.dbsize().await.expect("Failed to get dbsize");
    println!("Database keys after 1000 ports: {}", dbsize);

    // Get all keys to estimate size
    let all_keys = env.keys("*").await.expect("Failed to get keys");
    println!("Total keys: {}", all_keys.len());

    // This is just a sanity check - actual memory usage would need system-level monitoring
    assert!(
        dbsize >= 1000,
        "Expected at least 1000 keys, got {}",
        dbsize
    );
}
