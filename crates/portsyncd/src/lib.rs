//! Port Synchronization Daemon
//!
//! Synchronizes kernel port/interface status with SONiC databases via netlink events.
//! Listens for RTM_NEWLINK and RTM_DELLINK messages and updates STATE_DB with port status.
//!
//! NIST 800-53 Rev5 [SC-7]: Boundary Protection - Port status synchronization
//! NIST 800-53 Rev5 [SI-4]: System Monitoring - Real-time port state monitoring

pub mod config;
pub mod port_sync;
pub mod error;

pub use config::*;
pub use port_sync::*;
pub use error::*;
