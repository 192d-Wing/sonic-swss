//! # fabricmgrd - Fabric Monitoring Configuration Manager
//!
//! This module implements the fabric monitoring configuration manager daemon for SONiC.
//! It translates fabric monitoring configuration from CONFIG_DB into APPL_DB entries.
//!
//! ## Responsibilities
//! - Fabric monitoring threshold configuration
//! - Fabric port configuration (alias, lanes, isolation status)
//! - Pure CONFIG_DB → APPL_DB pass-through (no shell commands)
//!
//! ## Configuration Sources
//! - `FABRIC_MONITOR_DATA` table: Global fabric monitoring thresholds
//! - `FABRIC_PORT` table: Per-fabric-port configuration
//!
//! ## Key Features
//! - No shell commands (pure database operations)
//! - Simple field-by-field pass-through
//! - Separate handling for monitor data vs port data

mod fabric_mgr;
mod tables;

pub use fabric_mgr::FabricMgr;
pub use tables::*;
