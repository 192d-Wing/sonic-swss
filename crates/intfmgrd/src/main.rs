//! Interface Manager Daemon Entry Point

use sonic_intfmgrd::{IntfMgr, SwitchType};
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("Starting intfmgrd");

    // Detect switch type from environment or CONFIG_DB
    // TODO: Read from CONFIG_DB DEVICE_METADATA table
    let switch_type = std::env::var("SWITCH_TYPE")
        .map(|s| SwitchType::from_str(&s))
        .unwrap_or(SwitchType::Normal);

    info!("Detected switch type: {:?}", switch_type);

    // Create manager instance
    let mut _mgr = IntfMgr::new(switch_type);

    // TODO: Set up database connections
    // TODO: Register consumers for CONFIG_DB tables:
    //       - INTERFACE
    //       - VLAN_INTERFACE
    //       - LAG_INTERFACE
    //       - LOOPBACK_INTERFACE
    // TODO: Register STATE_DB consumers for port/LAG state
    // TODO: Handle warm restart replay
    // TODO: Enter event loop

    info!("intfmgrd initialized successfully");

    // For now, just sleep to prevent exit
    tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
}
