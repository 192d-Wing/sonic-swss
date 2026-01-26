//! vlanmgrd - VLAN configuration manager daemon for SONiC
//!
//! Manages VLAN interfaces and VLAN memberships by translating CONFIG_DB
//! entries into Linux bridge/VLAN operations and APPL_DB updates.

mod bridge;
mod commands;
mod tables;
mod types;
mod vlan_mgr;

pub use bridge::*;
pub use commands::*;
pub use tables::*;
pub use types::*;
pub use vlan_mgr::VlanMgr;
