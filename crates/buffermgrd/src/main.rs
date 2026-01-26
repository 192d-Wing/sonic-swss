//! Buffer Manager Daemon Entry Point

use sonic_buffermgrd::{parse_pg_lookup_file, BufferMgr};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("Starting buffermgrd");

    // Parse PG lookup file path from args or use default
    let pg_lookup_file = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/usr/share/sonic/hwsku/pg_profile_lookup.ini".to_string());

    info!("Loading PG profile lookup file: {}", pg_lookup_file);

    // Parse PG lookup file
    let pg_profile_lookup = match parse_pg_lookup_file(&pg_lookup_file) {
        Ok(lookup) => {
            info!("Successfully loaded {} speed entries", lookup.len());
            lookup
        }
        Err(e) => {
            error!("Failed to parse PG lookup file: {}", e);
            error!("Continuing with empty lookup table");
            Default::default()
        }
    };

    // Create manager instance
    let mut _mgr = BufferMgr::new(pg_profile_lookup);

    // TODO: Set up database connections
    // TODO: Register consumers for CONFIG_DB tables
    // TODO: Enter event loop

    info!("buffermgrd initialized successfully");

    // For now, just sleep to prevent exit
    tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
}
