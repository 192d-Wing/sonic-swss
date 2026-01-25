//! sflowmgrd - sFlow Sampling Configuration Manager Daemon
//!
//! Entry point for the sflowmgrd daemon.

use std::process::ExitCode;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

use sonic_sflowmgrd::SflowMgr;

/// Initializes tracing/logging subsystem
fn init_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

#[tokio::main]
async fn main() -> ExitCode {
    init_logging();

    info!("--- Starting sflowmgrd (Rust) ---");

    let _mgr = SflowMgr::new();

    // TODO: Implement event loop when swss-common bindings are ready
    // For now, this is a placeholder that demonstrates the daemon structure

    info!("sflowmgrd initialization complete (placeholder mode)");
    info!("Full implementation pending swss-common Consumer/Producer integration");

    // In production, this would run the event loop:
    // match run_event_loop(mgr).await {
    //     Ok(()) => ExitCode::SUCCESS,
    //     Err(e) => {
    //         error!("sflowmgrd failed: {}", e);
    //         ExitCode::FAILURE
    //     }
    // }

    ExitCode::SUCCESS
}
