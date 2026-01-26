//! Neighbor Synchronization Daemon
//!
//! Main entry point for the neighsyncd daemon.
//! Listens for kernel netlink neighbor events and synchronizes to SONiC databases.
//!
//! # NIST 800-53 Rev 5 Control Mappings
//! - AU-3: Content of Audit Records - Structured logging
//! - AU-12: Audit Record Generation - Log daemon lifecycle
//! - CP-10: System Recovery - Warm restart support
//! - SC-7: Boundary Protection - Network neighbor awareness
//! - SI-4: System Monitoring - Real-time event processing
//!
//! # Performance
//! Uses AsyncNeighSync with epoll-based async netlink I/O for efficient
//! event processing without busy-waiting.

use sonic_neighsyncd::{
    AsyncNeighSync, HealthMonitor, MetricsCollector, NeighsyncError, Result,
    start_metrics_server_insecure,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

// NIST SP 800-53 Rev5 compliant audit logging
use sonic_audit::{
    init_global_auditor, AuditorConfig, Facility,
    backends::{SyslogBackend, MultiBackend, WriteStrategy},
};
#[cfg(feature = "redis")]
use sonic_audit::backends::RedisBackend;

/// Default Redis connection settings
/// NIST: CM-6 - Configuration settings
const REDIS_HOST: &str = "127.0.0.1";
const REDIS_PORT: u16 = 6379;

/// Warm restart reconciliation timer (seconds)
/// NIST: CP-10 - Recovery timing
const WARMSTART_RECONCILE_TIMER_SECS: u64 = 5;

/// Default metrics server port
/// NIST: SI-4 - System monitoring endpoint
const METRICS_PORT: u16 = 9091;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    // NIST: AU-3, AU-12 - Audit logging setup
    init_logging()?;

    // Initialize NIST-compliant audit framework
    // NIST: AU-2, AU-3, AU-4, AU-9, AU-12 - Comprehensive audit logging
    init_audit_framework().await?;

    info!("neighsyncd: Starting neighbor synchronization daemon");

    // Run daemon with signal handling
    match run_daemon().await {
        Ok(()) => {
            info!("neighsyncd: Daemon exiting normally");
            Ok(())
        }
        Err(e) => {
            error!(error = %e, "neighsyncd: Daemon exiting with error");
            Err(Box::new(e) as Box<dyn std::error::Error>)
        }
    }
}

/// Initialize structured logging
///
/// # NIST Controls
/// - AU-3: Content of Audit Records - Structured format
/// - AU-9: Protection of Audit Information - Log to system journal
fn init_logging() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| NeighsyncError::Config(format!("Failed to set logger: {}", e)))?;

    Ok(())
}

/// Initialize NIST SP 800-53 Rev5 compliant audit framework
///
/// # NIST Controls
/// - AU-2: Event Logging - Configurable audit event types
/// - AU-3: Content of Audit Records - Comprehensive structured records
/// - AU-4: Audit Storage Capacity - Multi-backend with Redis persistence
/// - AU-6: Audit Review and Analysis - Optional SIEM integration
/// - AU-9: Protection of Audit Information - Backend security
/// - AU-11: Audit Record Retention - Redis persistence
/// - AU-12: Audit Generation - Automated audit record generation
///
/// # Backend Configuration
/// - **Syslog**: Local Unix socket to /dev/log (Facility::Local0)
/// - **Redis**: Persistent storage in StateDB (database 6) with 10k entry limit
/// - **SIEM**: Optional remote aggregation (configure via environment)
///
/// Uses `WriteStrategy::BestEffort` to ensure audit failures don't block operations.
async fn init_audit_framework() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create multi-backend for redundancy (AU-4, AU-9)
    let mut multi = MultiBackend::new(WriteStrategy::BestEffort);

    // 1. Syslog backend - local system integration (AU-12)
    let syslog_backend = SyslogBackend::new(Facility::Local0, "neighsyncd")
        .map_err(|e| NeighsyncError::Config(format!("Failed to init syslog backend: {}", e)))?;
    multi.add_backend(Arc::new(syslog_backend));
    info!("neighsyncd: Initialized syslog audit backend (Facility::Local0)");

    // 2. Redis backend - persistent storage (AU-4, AU-11)
    #[cfg(feature = "redis")]
    {
        let mut redis_backend = RedisBackend::new(6, "AUDIT_LOG", 10000);
        match redis_backend.connect(REDIS_HOST, REDIS_PORT).await {
            Ok(()) => {
                multi.add_backend(Arc::new(redis_backend));
                info!("neighsyncd: Initialized Redis audit backend (StateDB, max 10k entries)");
            }
            Err(e) => {
                warn!(error = %e, "neighsyncd: Failed to connect Redis audit backend, continuing without persistence");
            }
        }
    }

    // 3. SIEM backend - optional remote aggregation (AU-6)
    // TODO: Configure via environment variable SIEM_SERVER
    // Example: export SIEM_SERVER=siem.example.com:514
    if let Ok(siem_addr) = std::env::var("SIEM_SERVER") {
        match siem_addr.parse() {
            Ok(addr) => {
                match sonic_audit::backends::SiemBackend::new_udp(addr, Facility::Local0) {
                    Ok(siem_backend) => {
                        multi.add_backend(Arc::new(siem_backend));
                        info!(siem_server = %siem_addr, "neighsyncd: Initialized SIEM audit backend (UDP)");
                    }
                    Err(e) => {
                        warn!(error = %e, "neighsyncd: Failed to init SIEM backend");
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, siem_addr, "neighsyncd: Invalid SIEM_SERVER address");
            }
        }
    }

    // Initialize global auditor (AU-2, AU-3)
    let config = AuditorConfig::new("neighsyncd")
        .with_facility(Facility::Local0)
        .with_min_severity(sonic_audit::Severity::Informational);

    init_global_auditor(config, Arc::new(multi))
        .map_err(|e| NeighsyncError::Config(format!("Failed to init global auditor: {}", e)))?;

    info!("neighsyncd: NIST SP 800-53 Rev5 audit framework initialized");

    // Log framework initialization as audit event
    sonic_audit::info_audit!(
        "neighsyncd",
        event = "audit_init",
        backends = "syslog,redis",
        "Audit framework initialized with multi-backend configuration"
    );

    Ok(())
}

/// Main daemon loop
///
/// # NIST Controls
/// - SI-4: System Monitoring - Event loop for monitoring
/// - CP-10: System Recovery - Warm restart handling
/// - AU-6: Audit Record Review - Metrics collection
///
/// # Performance
/// Uses AsyncNeighSync with epoll-based async I/O. The netlink socket
/// integrates with tokio's event loop, yielding when no data is available
/// instead of busy-waiting.
async fn run_daemon() -> Result<()> {
    // Initialize metrics collector
    // NIST: AU-6, SI-4 - Metrics collection for monitoring
    let metrics = MetricsCollector::new()
        .map_err(|e| NeighsyncError::Config(format!("Failed to create metrics: {}", e)))?;
    info!("neighsyncd: Initialized metrics collector");

    // Initialize health monitor
    // NIST: CP-10, SI-4 - Health tracking
    let mut health_monitor = HealthMonitor::new(metrics.clone());
    info!("neighsyncd: Initialized health monitor");

    // Spawn metrics server in background (insecure mode for now - TODO: Add mTLS support)
    // NIST: AU-6 - Metrics endpoint for analysis
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        info!(
            port = METRICS_PORT,
            "neighsyncd: Starting metrics server (HTTP mode)"
        );
        if let Err(e) = start_metrics_server_insecure(metrics_clone, Some(METRICS_PORT)).await {
            error!(error = %e, "neighsyncd: Metrics server failed");
        }
    });

    // Setup signal handlers for graceful shutdown
    // NIST: AU-12 - Log shutdown events
    let shutdown = setup_signal_handlers();

    // Initialize AsyncNeighSync with epoll integration
    // NIST: AC-3 - Access enforcement via kernel permissions
    let mut neigh_sync = AsyncNeighSync::new(REDIS_HOST, REDIS_PORT).await?;
    info!("neighsyncd: Initialized AsyncNeighSync with epoll integration");

    // Update connection status metrics
    metrics.set_netlink_connected(true);
    metrics.set_redis_connected(true);

    // Handle warm restart if applicable
    // NIST: CP-10 - System recovery
    let warm_restart_active = neigh_sync.start_warm_restart().await?;
    if warm_restart_active {
        info!("neighsyncd: Warm restart detected, waiting for neighbor restore");

        // Wait for restore_neighbors service to complete
        neigh_sync.wait_for_restore().await?;
        info!("neighsyncd: Neighbor restore complete");

        // Start reconciliation timer
        let reconcile_deadline = tokio::time::Instant::now()
            + tokio::time::Duration::from_secs(WARMSTART_RECONCILE_TIMER_SECS);

        // Process events until reconciliation timer expires
        loop {
            if shutdown.load(Ordering::Relaxed) {
                warn!("neighsyncd: Shutdown during warm restart");
                return Ok(());
            }

            // Check if reconciliation timer expired
            if tokio::time::Instant::now() >= reconcile_deadline {
                info!("neighsyncd: Reconciliation timer expired");
                neigh_sync.reconcile().await?;
                break;
            }

            // Process events with timeout (async - yields when no data)
            tokio::select! {
                biased;
                result = neigh_sync.process_events_batched() => {
                    match result {
                        Ok(count) if count > 0 => {
                            // Record successful event processing
                            health_monitor.record_success();
                            metrics.set_pending_neighbors(0);
                        }
                        Err(e) => {
                            warn!(error = %e, "neighsyncd: Error processing events during warm restart");
                            health_monitor.record_failure();
                            metrics.record_event_failed();
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep_until(reconcile_deadline) => {
                    // Timer expired, will be handled in next iteration
                }
            }

            // Update health status periodically
            health_monitor.update_health();
        }
    }

    // Request initial neighbor table dump
    // NIST: CM-8 - Initial inventory
    neigh_sync.request_dump()?;
    info!("neighsyncd: Listening to neighbor events (async epoll mode)...");

    // Main event loop - true async, no polling!
    // NIST: SI-4 - Continuous monitoring
    loop {
        tokio::select! {
            biased;
            // Check shutdown first
            _ = tokio::signal::ctrl_c() => {
                info!("neighsyncd: Received SIGINT");
                break;
            }
            // Process netlink events (async - waits via epoll)
            result = neigh_sync.process_events_batched() => {
                let start = std::time::Instant::now();
                match result {
                    Ok(count) if count > 0 => {
                        info!(count, "neighsyncd: Processed neighbor events");

                        // Record metrics
                        health_monitor.record_success();
                        metrics.set_pending_neighbors(0);

                        // Record latency
                        let latency_secs = start.elapsed().as_secs_f64();
                        metrics.observe_event_latency(latency_secs);
                    }
                    Ok(_) => {
                        // No events, still update health
                        health_monitor.update_health();
                    }
                    Err(e) => {
                        warn!(error = %e, "neighsyncd: Error processing events");

                        // Record failure metrics
                        health_monitor.record_failure();
                        metrics.record_event_failed();
                        metrics.record_netlink_error();

                        // Brief pause on error to avoid tight loop
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }

        // Update health status periodically
        health_monitor.update_health();

        // Check shutdown flag (set by signal handler)
        if shutdown.load(Ordering::Relaxed) {
            info!("neighsyncd: Received shutdown signal");
            break;
        }
    }

    info!("neighsyncd: Graceful shutdown complete");
    Ok(())
}

/// Setup signal handlers for graceful shutdown
///
/// # NIST Controls
/// - AU-12: Audit Record Generation - Log shutdown signals
fn setup_signal_handlers() -> Arc<AtomicBool> {
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_flag_clone = shutdown_flag.clone();

    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            info!("neighsyncd: Received SIGINT/SIGTERM");
            shutdown_flag_clone.store(true, Ordering::Relaxed);
        }
    });

    shutdown_flag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::Relaxed));
        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_constants() {
        assert_eq!(REDIS_HOST, "127.0.0.1");
        assert_eq!(REDIS_PORT, 6379);
        assert_eq!(WARMSTART_RECONCILE_TIMER_SECS, 5);
    }
}
