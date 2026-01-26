//! Tunnel Manager Daemon - IP-in-IP tunnel configuration manager
//!
//! tunnelmgrd manages IP-in-IP tunnel interfaces in SONiC, handling:
//! - Tunnel interface creation and lifecycle
//! - Peer endpoint discovery from PEER_SWITCH table
//! - Route management through tunnel devices
//! - APPL_DB synchronization for orchagent
//! - Warm restart support

pub mod commands;
pub mod tables;
pub mod tunnel_mgr;
pub mod types;

pub use tunnel_mgr::TunnelMgr;
pub use types::TunnelInfo;
