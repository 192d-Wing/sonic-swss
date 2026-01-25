//! fabricmgrd - Fabric Monitoring Configuration Manager Daemon
//!
//! Entry point for the fabricmgrd daemon.

use std::process::ExitCode;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use sonic_fabricmgrd::FabricMgr;

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

    info!("--- Starting fabricmgrd (Rust) ---");

    let _mgr = FabricMgr::new();

    // TODO: Implement event loop when swss-common bindings are ready
    // For now, this is a placeholder that demonstrates the daemon structure

    info!("fabricmgrd initialization complete (placeholder mode)");
    info!("Full implementation pending swss-common Consumer/Producer integration");

    ExitCode::SUCCESS
}
