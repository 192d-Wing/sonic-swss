//! CoPP Manager Daemon Entry Point

use sonic_coppmgrd::{parse_copp_init_file, CoppMgr};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("Starting coppmgrd");

    // Get CoPP init file path from args or use default
    let copp_init_file = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/etc/sonic/copp_cfg.json".to_string());

    info!("Loading CoPP init file: {}", copp_init_file);

    // Parse CoPP init file
    let (trap_init_cfg, group_init_cfg) = match parse_copp_init_file(&copp_init_file) {
        Ok((trap_cfg, group_cfg)) => {
            info!(
                "Successfully loaded {} trap entries, {} group entries",
                trap_cfg.len(),
                group_cfg.len()
            );
            (trap_cfg, group_cfg)
        }
        Err(e) => {
            error!("Failed to parse CoPP init file: {}", e);
            error!("Continuing with empty init configuration");
            (Default::default(), Default::default())
        }
    };

    // Create manager instance
    let mut _mgr = CoppMgr::new(trap_init_cfg, group_init_cfg, copp_init_file);

    // TODO: Set up database connections
    // TODO: Register consumers for CONFIG_DB tables
    // TODO: Read existing CONFIG_DB entries and merge with init config
    // TODO: Initialize APPL_DB with merged config
    // TODO: Enter event loop

    info!("coppmgrd initialized successfully");

    // For now, just sleep to prevent exit
    tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
}
