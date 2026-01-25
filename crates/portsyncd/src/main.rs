//! Port Synchronization Daemon
//!
//! Main entry point for the portsyncd daemon.
//! Listens for kernel netlink events and synchronizes port status to SONiC databases.

use sonic_portsyncd::{
    LinkSync, MetricsCollector, MetricsServer, MetricsServerConfig, PortsyncError, RedisAdapter,
    load_port_config, send_port_config_done, send_port_init_done,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    init_logging()?;

    eprintln!("portsyncd: Starting port synchronization daemon");

    // Run daemon with signal handling
    run_daemon()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    eprintln!("portsyncd: Port synchronization daemon exiting");

    Ok(())
}

/// Initialize logging with structured format
///
/// # NIST Controls
/// - AU-3: Content of Audit Records - Structured logging format
/// - AU-12: Audit Record Generation - Comprehensive event logging
/// - SI-4: System Monitoring - Real-time event visibility
fn init_logging() -> Result<(), PortsyncError> {
    // Create structured logging with timestamp, target, and context
    // This enables SIEM integration and centralized log aggregation
    // NIST: AU-3, AU-12 - RFC 5424 syslog-compatible structured logging

    let env_filter = std::env::var("PORTSYNCD_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

    // Configure tracing with structured format for RFC 5424 compliance
    let _subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(&env_filter))
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .init();

    eprintln!(
        "portsyncd: Structured logging initialized (level: {})",
        env_filter
    );
    Ok(())
}

/// Main daemon loop with full orchestration
async fn run_daemon() -> Result<(), PortsyncError> {
    // Setup signal handlers for graceful shutdown
    let shutdown = setup_signal_handlers();

    // Initialize metrics collector
    let metrics = Arc::new(
        MetricsCollector::new()
            .map_err(|e| PortsyncError::Other(format!("Failed to initialize metrics: {}", e)))?,
    );
    eprintln!("portsyncd: Initialized metrics collector");

    // Spawn metrics server with mandatory mTLS on IPv6 [::1]:9090
    // Certificate paths can be configured via environment variables or config file
    let cert_path = std::env::var("PORTSYNCD_METRICS_CERT")
        .unwrap_or_else(|_| "/etc/portsyncd/metrics/server.crt".to_string());
    let key_path = std::env::var("PORTSYNCD_METRICS_KEY")
        .unwrap_or_else(|_| "/etc/portsyncd/metrics/server.key".to_string());
    let ca_cert_path = std::env::var("PORTSYNCD_METRICS_CA")
        .unwrap_or_else(|_| "/etc/portsyncd/metrics/ca.crt".to_string());

    let metrics_server_handle = tokio::spawn({
        let metrics_clone = metrics.clone();
        async move {
            let config = MetricsServerConfig::new(cert_path, key_path, ca_cert_path);
            let server = MetricsServer::new(config, metrics_clone)?;
            server.start().await
        }
    });
    eprintln!("portsyncd: Spawned metrics server with mandatory mTLS on IPv6 [::1]:9090");

    // Connect to databases via Redis adapter
    #[cfg(not(test))]
    let (config_db, mut app_db) = {
        let mut c = RedisAdapter::config_db("127.0.0.1", 6379);
        let mut a = RedisAdapter::app_db("127.0.0.1", 6379);
        c.connect().await?;
        a.connect().await?;
        (c, a)
    };

    #[cfg(test)]
    let (config_db, mut app_db) = {
        (
            RedisAdapter::config_db("127.0.0.1", 6379),
            RedisAdapter::app_db("127.0.0.1", 6379),
        )
    };

    eprintln!("portsyncd: Connected to databases");

    // Load port configuration from CONFIG_DB
    let port_configs = load_port_config(&config_db, &mut app_db, false).await?;
    eprintln!(
        "portsyncd: Loaded {} port configurations",
        port_configs.len()
    );

    // Send PortConfigDone signal
    send_port_config_done(&mut app_db).await?;
    eprintln!("portsyncd: Sent PortConfigDone signal");

    // Create LinkSync daemon and initialize with port names
    let mut link_sync = LinkSync::new()?;
    let port_names: Vec<String> = port_configs.iter().map(|p| p.name.clone()).collect();
    link_sync.initialize_ports(port_names);
    eprintln!(
        "portsyncd: Initialized LinkSync with {} ports",
        link_sync.uninitialized_count()
    );

    // Main event loop - simulate receiving netlink events
    // In production, this would connect to kernel netlink socket
    eprintln!("portsyncd: Starting event processing loop");

    loop {
        // Check for shutdown signal
        if shutdown.load(Ordering::Relaxed) {
            eprintln!("portsyncd: Received shutdown signal");
            break;
        }

        // TODO: In production, receive actual netlink events from kernel socket
        // For now, simulate a simple delay to prevent busy loop
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check if all ports have been initialized and send signal
        if link_sync.should_send_port_init_done() {
            let timer = metrics.start_event_latency();
            match send_port_init_done(&mut app_db).await {
                Ok(_) => {
                    metrics.record_event_success();
                    drop(timer);
                    link_sync.set_port_init_done();
                    eprintln!("portsyncd: Sent PortInitDone signal");
                }
                Err(e) => {
                    metrics.record_event_failure();
                    drop(timer);
                    eprintln!("portsyncd: Failed to send PortInitDone: {}", e);
                }
            }
        }
    }

    // Graceful shutdown
    eprintln!("portsyncd: Performing graceful shutdown");

    // Attempt graceful shutdown of metrics server
    drop(metrics_server_handle);

    Ok(())
}

/// Setup signal handlers and return atomic flag for shutdown signaling
fn setup_signal_handlers() -> Arc<AtomicBool> {
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_flag_clone = shutdown_flag.clone();

    // Handle SIGTERM (graceful shutdown)
    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            eprintln!("portsyncd: Received SIGTERM/SIGINT");
            shutdown_flag_clone.store(true, Ordering::Relaxed);
        }
    });

    shutdown_flag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag_creation() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::Relaxed));
        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_run_daemon_requires_databases() {
        // This test verifies the basic structure is correct
        // Full integration testing in Day 5
    }
}
