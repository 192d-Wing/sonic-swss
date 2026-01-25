//! SONiC Orchagent entry point.
//!
//! This is the main entry point for the Rust implementation of SONiC orchagent.
//! It initializes all necessary components and starts the main event loop.

use clap::Parser;
use log::{error, info, warn};
use sonic_orchagent::daemon::{OrchDaemon, OrchDaemonConfig};
use std::process::ExitCode;
use std::sync::Arc;
use tokio::sync::Mutex;

/// SONiC Switch Orchestration Agent
#[derive(Parser, Debug)]
#[command(name = "orchagent")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Switch MAC address
    #[arg(short = 'm', long)]
    mac_address: Option<String>,

    /// Batch size for consumer table operations
    #[arg(short = 'b', long, default_value = "128")]
    batch_size: usize,

    /// Enable recording mode for debugging
    #[arg(short = 'r', long)]
    record: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short = 'l', long, default_value = "info")]
    log_level: String,

    /// Heartbeat interval in milliseconds
    #[arg(long, default_value = "1000")]
    heartbeat_interval: u64,

    /// Enable warm boot mode
    #[arg(long)]
    warm_boot: bool,

    /// Redis server host
    #[arg(long, default_value = "127.0.0.1")]
    redis_host: String,

    /// Redis server port
    #[arg(long, default_value = "6379")]
    redis_port: u16,

    /// Redis database index for CONFIG_DB
    #[arg(long, default_value = "4")]
    config_db: u32,

    /// Redis database index for APPL_DB
    #[arg(long, default_value = "0")]
    appl_db: u32,

    /// Redis database index for STATE_DB
    #[arg(long, default_value = "6")]
    state_db: u32,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&args.log_level))
        .init();

    info!("====================================================================");
    info!("Starting SONiC orchagent (Rust implementation)");
    info!("====================================================================");
    info!("Batch size: {}", args.batch_size);
    info!("Heartbeat interval: {}ms", args.heartbeat_interval);
    if let Some(ref mac) = args.mac_address {
        info!("Switch MAC: {}", mac);
    }
    info!("Redis: {}:{}", args.redis_host, args.redis_port);
    info!("CONFIG_DB: {}", args.config_db);
    info!("APPL_DB: {}", args.appl_db);
    info!("STATE_DB: {}", args.state_db);
    if args.warm_boot {
        info!("Warm boot mode: ENABLED");
    }
    if args.record {
        info!("Recording mode: ENABLED");
    }

    // Initialize OrchDaemon with configuration
    let daemon_config = OrchDaemonConfig {
        heartbeat_interval_ms: args.heartbeat_interval,
        batch_size: args.batch_size,
        warm_boot: args.warm_boot,
    };

    let mut daemon = OrchDaemon::new(daemon_config);

    // TODO: Register all orchestration modules
    // - PortsOrch (priority 0 - must be first)
    // - IntfsOrch (priority 5)
    // - VRFOrch (priority 10)
    // - VlanOrch (priority 10)
    // - BridgeOrch (priority 10)
    // - NeighOrch (priority 15)
    // - RouteOrch (priority 20)
    // - ACLOrch (priority 30)
    // - MirrorOrch (priority 40)
    // - And all other modules with appropriate priorities

    // Initialize the daemon
    info!("Initializing orchagent daemon...");
    if !daemon.init().await {
        error!("Failed to initialize orchagent daemon");
        return ExitCode::FAILURE;
    }

    info!("Daemon initialization complete");
    info!("Starting event loop...");

    // Setup signal handling for graceful shutdown
    let daemon_arc = Arc::new(Mutex::new(daemon));
    let daemon_clone = Arc::clone(&daemon_arc);

    let shutdown_handle = tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                warn!("Received SIGINT, shutting down gracefully...");
                let mut daemon = daemon_clone.lock().await;
                daemon.stop();
            }
            Err(err) => {
                error!("Failed to listen for ctrl-c: {}", err);
            }
        }
    });

    // Run the main event loop
    {
        let mut daemon = daemon_arc.lock().await;
        daemon.run().await;
    }

    shutdown_handle.abort();

    info!("====================================================================");
    info!("SONiC orchagent shutdown complete");
    info!("====================================================================");

    ExitCode::SUCCESS
}
