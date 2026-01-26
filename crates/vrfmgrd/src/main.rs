//! vrfmgrd - VRF configuration manager daemon
//!
//! Manages Virtual Routing and Forwarding (VRF) instances for SONiC

use sonic_vrfmgrd::VrfMgr;
use std::process::ExitCode;
use tracing::info;

#[tokio::main]
async fn main() -> ExitCode {
    init_logging();

    info!("--- Starting vrfmgrd (Rust) ---");

    let _mgr = VrfMgr::new();
    info!("vrfmgrd initialization complete (placeholder mode)");

    // TODO: Set up DB connections and event loop
    // TODO: Process CONFIG_DB changes
    // TODO: Handle warm restart

    ExitCode::SUCCESS
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();
}
