//! CoPP Manager Daemon - Control Plane Policing configuration manager
//!
//! coppmgrd manages CoPP (Control Plane Policing) configuration to protect the
//! switch CPU from being overwhelmed by control plane traffic.
//!
//! Key features:
//! - Parse JSON init file with default CoPP policies
//! - Merge init config with user CONFIG_DB configuration
//! - Manage trap groups (policer settings) and trap IDs
//! - Integrate with FEATURE table to enable/disable traps
//! - Write to APPL_DB and STATE_DB

pub mod config_merge;
pub mod copp_mgr;
pub mod json_parser;
pub mod tables;
pub mod types;

pub use copp_mgr::CoppMgr;
pub use json_parser::parse_copp_init_file;
pub use types::{CoppCfg, CoppTrapConf};
