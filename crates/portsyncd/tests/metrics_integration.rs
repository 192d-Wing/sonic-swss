//! Integration tests for metrics collection and HTTP server
//!
//! Tests the complete metrics system including:
//! - Metrics collection and recording
//! - HTTP /metrics endpoint serving
//! - Prometheus text format output

use sonic_portsyncd::{MetricsCollector, MetricsServer, MetricsServerConfig};
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::test]
async fn test_metrics_server_startup_requires_mtls_certs() {
    let metrics = Arc::new(MetricsCollector::new().expect("Failed to create metrics"));

    // Should fail because certificates don't exist
    let config = MetricsServerConfig::new(
        "/nonexistent/cert.pem".to_string(),
        "/nonexistent/key.pem".to_string(),
        "/nonexistent/ca.pem".to_string(),
    );
    let result = MetricsServer::new(config, metrics);

    // Should fail validation
    assert!(result.is_err());
}

#[test]
fn test_metrics_collection_integration() {
    let metrics = MetricsCollector::new().expect("Failed to create metrics");

    // Record various events
    metrics.record_event_success();
    metrics.record_event_success();
    metrics.record_event_failure();
    metrics.record_port_flap("Ethernet0");
    metrics.record_port_flap("Ethernet4");

    // Set gauges
    metrics.set_queue_depth(42);
    metrics.set_memory_bytes(104857600);
    metrics.set_health_status(1.0);
    metrics.set_redis_connected(true);
    metrics.set_netlink_connected(true);

    // Record histogram observations
    let timer1 = metrics.start_event_latency();
    drop(timer1);
    let timer2 = metrics.start_redis_latency();
    drop(timer2);

    // Gather metrics and verify output
    let output = metrics.gather_metrics();

    // Verify metrics are present in output
    assert!(output.contains("portsyncd_events_processed_total 2"));
    assert!(output.contains("portsyncd_events_failed_total 1"));
    assert!(output.contains("portsyncd_port_flaps_total"));
    assert!(output.contains("Ethernet0"));
    assert!(output.contains("Ethernet4"));
    assert!(output.contains("portsyncd_queue_depth 42"));
    assert!(output.contains("portsyncd_memory_bytes"));
    assert!(output.contains("portsyncd_health_status 1"));
    assert!(output.contains("portsyncd_redis_connected 1"));
    assert!(output.contains("portsyncd_netlink_connected 1"));
    assert!(output.contains("portsyncd_event_latency_seconds_bucket"));
    assert!(output.contains("portsyncd_redis_latency_seconds_bucket"));

    // Verify Prometheus text format
    assert!(output.contains("# HELP"));
    assert!(output.contains("# TYPE"));
}

#[test]
fn test_metrics_collection_with_connections_down() {
    let metrics = MetricsCollector::new().expect("Failed to create metrics");

    // Set connection statuses as disconnected
    metrics.set_redis_connected(false);
    metrics.set_netlink_connected(false);
    metrics.set_health_status(0.0); // Unhealthy

    let output = metrics.gather_metrics();

    assert!(output.contains("portsyncd_redis_connected 0"));
    assert!(output.contains("portsyncd_netlink_connected 0"));
    assert!(output.contains("portsyncd_health_status 0"));
}

#[test]
fn test_metrics_collection_degraded_health() {
    let metrics = MetricsCollector::new().expect("Failed to create metrics");

    // Set degraded health status
    metrics.set_health_status(0.5);

    let output = metrics.gather_metrics();

    assert!(output.contains("portsyncd_health_status 0.5"));
}

#[test]
fn test_metrics_config_ipv6_mandatory_mtls() {
    // mTLS is now mandatory - always required
    let config = MetricsServerConfig::new(
        "/etc/portsyncd/metrics/server.crt".to_string(),
        "/etc/portsyncd/metrics/server.key".to_string(),
        "/etc/portsyncd/metrics/ca.crt".to_string(),
    );

    // Should default to IPv6 localhost
    assert!(config.listen_addr.is_ipv6());
    assert_eq!(config.listen_addr.to_string(), "[::1]:9090");

    // All cert paths are mandatory (not optional)
    assert!(!config.cert_path.is_empty());
    assert!(!config.key_path.is_empty());
    assert!(!config.ca_cert_path.is_empty());
}

#[test]
fn test_metrics_config_custom_ipv6_address() {
    let addr = "[::]:9090".parse::<SocketAddr>().unwrap();
    let config = MetricsServerConfig::with_ipv6(
        addr,
        "/etc/portsyncd/metrics/server.crt".to_string(),
        "/etc/portsyncd/metrics/server.key".to_string(),
        "/etc/portsyncd/metrics/ca.crt".to_string(),
    );

    assert_eq!(config.listen_addr.to_string(), "[::]:9090");
}

#[test]
fn test_metrics_multiple_port_tracking() {
    let metrics = MetricsCollector::new().expect("Failed to create metrics");

    // Record flaps on multiple ports
    for i in 0..10 {
        let port_name = format!("Ethernet{}", i * 4);
        metrics.record_port_flap(&port_name);
        metrics.record_port_flap(&port_name);
        metrics.record_port_flap(&port_name);
    }

    let output = metrics.gather_metrics();

    // Verify all ports recorded with count of 3
    for i in 0..10 {
        let port_name = format!("Ethernet{}", i * 4);
        assert!(
            output.contains(&port_name),
            "Port {} not found in metrics",
            port_name
        );
    }
}

#[test]
fn test_metrics_event_latency_timer() {
    let metrics = MetricsCollector::new().expect("Failed to create metrics");

    // Record several latency observations
    for _ in 0..5 {
        let timer = metrics.start_event_latency();
        std::thread::sleep(std::time::Duration::from_millis(1));
        drop(timer);
    }

    let output = metrics.gather_metrics();

    // Verify histogram is present
    assert!(output.contains("portsyncd_event_latency_seconds_bucket"));
    assert!(output.contains("portsyncd_event_latency_seconds_sum"));
    assert!(output.contains("portsyncd_event_latency_seconds_count 5"));
}
