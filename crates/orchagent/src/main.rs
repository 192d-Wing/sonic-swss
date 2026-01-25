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

    // Register all orchestration modules in priority order
    // Lower priority numbers execute first
    // NIST: CM-3 - Configuration Change Control (module registration audit logging)
    info!("Registering orchestration modules...");

    // Priority 0: Critical infrastructure modules (must initialize first)
    // PortsOrch handles physical port configuration - required before any interface operations
    info!("  Registering module: PortsOrch (priority 0)");

    // Priority 5: Interface management (depends on ports)
    info!("  Registering module: IntfsOrch (priority 5)");

    // Priority 10: Core network infrastructure
    info!("  Registering module: VRFOrch (priority 10)");
    info!("  Registering module: VlanOrch (priority 10)");
    info!("  Registering module: BridgeOrch (priority 10)");

    // Priority 15: Neighbor/ARP/NDP resolution
    info!("  Registering module: NeighOrch (priority 15)");

    // Priority 20: Routing (depends on neighbors and interfaces)
    info!("  Registering module: RouteOrch (priority 20)");
    info!("  Registering module: MplsRouteOrch (priority 20)");
    info!("  Registering module: NhgOrch (priority 20)");
    info!("  Registering module: FgNhgOrch (priority 20)");

    // Priority 25: Tunneling and virtual networking
    info!("  Registering module: VxlanOrch (priority 25)");
    info!("  Registering module: NvgreOrch (priority 25)");
    info!("  Registering module: TunnelDecapOrch (priority 25)");
    info!("  Registering module: Srv6Orch (priority 25)");
    info!("  Registering module: VnetOrch (priority 25)");

    // Priority 30: Access control and security
    info!("  Registering module: AclOrch (priority 30)");
    info!("  Registering module: MacsecOrch (priority 30)");
    info!("  Registering module: NatOrch (priority 30)");

    // Priority 35: Port properties and configuration
    info!("  Registering module: QosOrch (priority 35)");
    info!("  Registering module: BufferOrch (priority 35)");
    info!("  Registering module: PolicerOrch (priority 35)");
    info!("  Registering module: PbhOrch (priority 35)");

    // Priority 40: Traffic management and monitoring
    info!("  Registering module: MirrorOrch (priority 40)");
    info!("  Registering module: SflowOrch (priority 40)");
    info!("  Registering module: DtelOrch (priority 40)");
    info!("  Registering module: PfcwdOrch (priority 40)");

    // Priority 45: High availability and resilience
    info!("  Registering module: MlagOrch (priority 45)");
    info!("  Registering module: MuxOrch (priority 45)");
    info!("  Registering module: StpOrch (priority 45)");

    // Priority 50: System and chassis management
    info!("  Registering module: SwitchOrch (priority 50)");
    info!("  Registering module: ChassisOrch (priority 50)");
    info!("  Registering module: FabricPortsOrch (priority 50)");

    // Priority 55: Monitoring and statistics
    info!("  Registering module: FlexCounterOrch (priority 55)");
    info!("  Registering module: DebugCounterOrch (priority 55)");
    info!("  Registering module: WatermarkOrch (priority 55)");
    info!("  Registering module: CounterCheckOrch (priority 55)");
    info!("  Registering module: CrmOrch (priority 55)");

    // Priority 60: Isolation and grouping
    info!("  Registering module: IsolationGroupOrch (priority 60)");

    // Priority 65: Network timing and synchronization
    info!("  Registering module: TwampOrch (priority 65)");
    info!("  Registering module: BfdOrch (priority 65)");

    // Priority 70: Specialized protocols
    info!("  Registering module: IcmpOrch (priority 70)");
    info!("  Registering module: CoppOrch (priority 70)");

    // Priority 75: Database and IPC
    info!("  Registering module: ZmqOrch (priority 75)");

    // Priority 80: FDB management
    info!("  Registering module: FdbOrch (priority 80)");

    // Note: Actual module instantiation will be added in subsequent phases
    // when we integrate with Redis, SAI, and SWSS-common libraries
    info!("All orchestration modules registered (in simulation mode)");

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
