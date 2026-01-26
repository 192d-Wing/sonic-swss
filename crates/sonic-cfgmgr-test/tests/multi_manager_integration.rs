//! Multi-manager integration tests
//!
//! Tests interactions between multiple configuration managers

use sonic_cfgmgr_test::{
    fixtures::{port_fixtures, sflow_fixtures, ConfigChange, ConfigOp},
    AppDbVerifier, RedisTestEnv,
};

/// Test portmgrd + sflowmgrd interaction
///
/// Scenario:
/// 1. Configure port Ethernet0
/// 2. Enable sFlow on Ethernet0
/// 3. Verify both PORT and SFLOW tables updated in APPL_DB
#[tokio::test]
#[ignore = "Requires Docker and actual manager implementations"]
async fn test_port_and_sflow_interaction() {
    // Start Redis
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");
    let verifier = AppDbVerifier::new(&env);

    // 1. Configure port
    let port_change = port_fixtures::ethernet_port_default("Ethernet0");
    simulate_config_db_change(&env, &port_change).await;

    // 2. Enable sFlow on port
    let sflow_change = sflow_fixtures::sflow_interface("Ethernet0", "4000");
    simulate_config_db_change(&env, &sflow_change).await;

    // Give managers time to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 3. Verify PORT_TABLE
    verifier
        .assert_field_value("PORT_TABLE:Ethernet0", "mtu", "9100")
        .await
        .expect("Port MTU not set");

    // 4. Verify SFLOW_SESSION_TABLE
    verifier
        .assert_field_value("SFLOW_SESSION_TABLE:Ethernet0", "sample_rate", "4000")
        .await
        .expect("sFlow sample rate not set");
}

/// Test cascading configuration changes
///
/// Scenario:
/// 1. Configure multiple ports
/// 2. Enable sFlow globally
/// 3. Override sample rate on one port
/// 4. Verify correct APPL_DB state
#[tokio::test]
#[ignore = "Requires Docker and actual manager implementations"]
async fn test_cascading_config_changes() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");
    let verifier = AppDbVerifier::new(&env);

    // Configure 3 ports
    for i in 0..3 {
        let port_name = format!("Ethernet{}", i * 4);
        let change = port_fixtures::ethernet_port_default(&port_name);
        simulate_config_db_change(&env, &change).await;
    }

    // Enable sFlow globally
    let global_sflow = sflow_fixtures::sflow_global();
    simulate_config_db_change(&env, &global_sflow).await;

    // All interfaces sFlow
    let all_sflow = sflow_fixtures::sflow_all_interfaces("8000");
    simulate_config_db_change(&env, &all_sflow).await;

    // Override one port
    let override_sflow = sflow_fixtures::sflow_interface("Ethernet0", "16000");
    simulate_config_db_change(&env, &override_sflow).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify Ethernet0 has override
    verifier
        .assert_field_value("SFLOW_SESSION_TABLE:Ethernet0", "sample_rate", "16000")
        .await
        .expect("Override sample rate not applied");

    // Verify Ethernet4 has global setting
    verifier
        .assert_field_value("SFLOW_SESSION_TABLE:Ethernet4", "sample_rate", "8000")
        .await
        .expect("Global sample rate not applied");
}

/// Helper to simulate CONFIG_DB change
async fn simulate_config_db_change(env: &RedisTestEnv, change: &ConfigChange) {
    let key = change.config_db_key();

    match change.op {
        ConfigOp::Set => {
            // Write all fields as hash
            for (field, value) in &change.fields {
                env.hset(&key, field, value)
                    .await
                    .expect("Failed to set field");
            }
        }
        ConfigOp::Del => {
            // Delete the key
            env.del(&key).await.expect("Failed to delete key");
        }
    }
}

#[tokio::test]
#[ignore = "Requires Docker"]
async fn test_simulate_config_db_change() {
    let env = RedisTestEnv::start().await.expect("Failed to start Redis");

    let change = port_fixtures::ethernet_port_custom_mtu("Ethernet0", "1500");
    simulate_config_db_change(&env, &change).await;

    let value = env
        .hget("PORT:Ethernet0", "mtu")
        .await
        .expect("Failed to get");
    assert_eq!(value, Some("1500".to_string()));
}
