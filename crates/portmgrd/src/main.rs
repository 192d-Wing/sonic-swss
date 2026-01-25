//! portmgrd daemon entry point.
//!
//! This is the main entry point for the port configuration manager daemon.
//! It initializes logging, database connections, and runs the event loop.

use std::process::ExitCode;

use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

use sonic_cfgmgr_common::{CfgMgr, Orch};
use sonic_portmgrd::PortMgr;

/// Select timeout in milliseconds (matches C++ SELECT_TIMEOUT).
const SELECT_TIMEOUT_MS: u64 = 1000;

/// Initialize tracing/logging.
fn init_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

/// Main event loop (placeholder for real Redis integration).
async fn run_event_loop(mut mgr: PortMgr) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting event loop with {}ms timeout", SELECT_TIMEOUT_MS);

    // In the real implementation, this would:
    // 1. Create Redis connections to CONFIG_DB, APPL_DB, STATE_DB
    // 2. Subscribe to CONFIG_DB tables (PORT, SEND_TO_INGRESS_PORT)
    // 3. Subscribe to STATE_DB (PORT_TABLE) for port readiness
    // 4. Run Select loop with timeout

    // For now, just demonstrate the structure
    // Simulate select timeout
    tokio::time::sleep(tokio::time::Duration::from_millis(SELECT_TIMEOUT_MS)).await;

    // Process any pending tasks
    mgr.do_task().await;

    // In real implementation, would check for shutdown signal
    // For now, just run once for testing
    info!(
        "Event loop iteration complete, {} pending tasks",
        mgr.pending_count()
    );

    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    init_logging();

    info!("--- Starting portmgrd (Rust) ---");

    // Check for warm restart (would read from command line or environment)
    let warm_restart = std::env::var("WARM_RESTART").is_ok();

    // Create the port manager
    let mgr = PortMgr::new().with_warm_restart(warm_restart);

    if warm_restart {
        info!("Warm restart mode enabled");
    }

    info!(
        "Subscribing to CONFIG_DB tables: {:?}",
        mgr.config_table_names()
    );
    info!(
        "Subscribing to STATE_DB tables: {:?}",
        mgr.state_table_names()
    );

    // Run the event loop
    match run_event_loop(mgr).await {
        Ok(()) => {
            info!("portmgrd exiting normally");
            ExitCode::SUCCESS
        }
        Err(e) => {
            error!("portmgrd error: {}", e);
            ExitCode::FAILURE
        }
    }
}
