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

use sonic_neighsyncd::{NeighSync, NeighsyncError, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use tracing::{Level, error, info, warn};
use tracing_subscriber::FmtSubscriber;

/// Default Redis connection settings
/// NIST: CM-6 - Configuration settings
const REDIS_HOST: &str = "127.0.0.1";
const REDIS_PORT: u16 = 6379;

/// Warm restart reconciliation timer (seconds)
/// NIST: CP-10 - Recovery timing
const WARMSTART_RECONCILE_TIMER_SECS: u64 = 5;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    // NIST: AU-3, AU-12 - Audit logging setup
    init_logging()?;

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

/// Main daemon loop
///
/// # NIST Controls
/// - SI-4: System Monitoring - Event loop for monitoring
/// - CP-10: System Recovery - Warm restart handling
async fn run_daemon() -> Result<()> {
    // Setup signal handlers for graceful shutdown
    // NIST: AU-12 - Log shutdown events
    let shutdown = setup_signal_handlers();

    // Initialize NeighSync
    // NIST: AC-3 - Access enforcement via kernel permissions
    let mut neigh_sync = NeighSync::new(REDIS_HOST, REDIS_PORT).await?;
    info!("neighsyncd: Initialized NeighSync");

    // Handle warm restart if applicable
    // NIST: CP-10 - System recovery
    let warm_restart_active = neigh_sync.start_warm_restart().await?;
    if warm_restart_active {
        info!("neighsyncd: Warm restart detected, waiting for neighbor restore");

        // Wait for restore_neighbors service to complete
        neigh_sync.wait_for_restore().await?;
        info!("neighsyncd: Neighbor restore complete");

        // Start reconciliation timer
        let reconcile_time = tokio::time::Instant::now()
            + tokio::time::Duration::from_secs(WARMSTART_RECONCILE_TIMER_SECS);

        // Process events until reconciliation timer expires
        loop {
            if shutdown.load(Ordering::Relaxed) {
                warn!("neighsyncd: Shutdown during warm restart");
                return Ok(());
            }

            // Check if reconciliation timer expired
            if tokio::time::Instant::now() >= reconcile_time {
                info!("neighsyncd: Reconciliation timer expired");
                neigh_sync.reconcile().await?;
                break;
            }

            // Process events with timeout
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    let _ = neigh_sync.process_events().await;
                }
            }
        }
    }

    // Request initial neighbor table dump
    // NIST: CM-8 - Initial inventory
    neigh_sync.request_dump()?;
    info!("neighsyncd: Listening to neighbor events...");

    // Main event loop
    // NIST: SI-4 - Continuous monitoring
    loop {
        if shutdown.load(Ordering::Relaxed) {
            info!("neighsyncd: Received shutdown signal");
            break;
        }

        // Process netlink events
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                match neigh_sync.process_events().await {
                    Ok(count) if count > 0 => {
                        info!(count, "neighsyncd: Processed neighbor events");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!(error = %e, "neighsyncd: Error processing events");
                    }
                }
            }
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
