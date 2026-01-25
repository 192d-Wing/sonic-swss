//! Port configuration manager daemon for SONiC.
//!
//! This crate implements the `portmgrd` daemon, which manages port
//! configuration in the Linux network stack based on CONFIG_DB entries.
//!
//! # Responsibilities
//!
//! - Set port MTU via `ip link set dev <port> mtu <mtu>`
//! - Set port admin status via `ip link set dev <port> up|down`
//! - Propagate configuration to APPL_DB for orchagent
//! - Handle SendToIngress port configuration
//!
//! # Tables
//!
//! | Database | Table | Purpose |
//! |----------|-------|---------|
//! | CONFIG_DB | PORT | Port configuration source |
//! | CONFIG_DB | SEND_TO_INGRESS_PORT | Special ingress port config |
//! | CONFIG_DB | PORTCHANNEL_MEMBER | LAG member detection |
//! | STATE_DB | PORT_TABLE | Port readiness state |
//! | APPL_DB | PORT_TABLE | Published config for orchagent |
//! | APPL_DB | SEND_TO_INGRESS_PORT_TABLE | Published ingress config |
//!
//! # Example
//!
//! ```ignore
//! use sonic_portmgrd::PortMgr;
//!
//! let mgr = PortMgr::new(config_db, app_db, state_db).await?;
//! mgr.run().await?;
//! ```

mod port_mgr;
mod tables;

pub use port_mgr::PortMgr;
pub use tables::*;
