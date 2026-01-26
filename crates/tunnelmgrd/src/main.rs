//! Tunnel Manager Daemon Entry Point

use sonic_tunnelmgrd::TunnelMgr;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("Starting tunnelmgrd");

    // Create manager instance
    let mut mgr = TunnelMgr::new();

    // Initialize peer IP
    if let Err(e) = mgr.init_peer_ip().await {
        error!("Failed to initialize peer IP: {}", e);
        std::process::exit(1);
    }

    // Initialize warm restart
    if let Err(e) = mgr.init_warm_restart().await {
        error!("Failed to initialize warm restart: {}", e);
        std::process::exit(1);
    }

    // Cleanup any existing tunnel interface
    if let Err(e) = mgr.cleanup_tunnel_interface().await {
        error!("Failed to cleanup tunnel interface: {}", e);
        // Continue anyway
    }

    // TODO: Set up database connections
    // TODO: Register consumers for CONFIG_DB and APPL_DB tables
    // TODO: Enter event loop

    info!("tunnelmgrd initialized successfully");

    // For now, just sleep to prevent exit
    tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
}
