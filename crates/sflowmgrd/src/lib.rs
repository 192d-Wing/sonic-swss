//! # sflowmgrd - sFlow Sampling Configuration Manager
//!
//! This module implements the sFlow configuration manager daemon for SONiC.
//! It translates sFlow configuration from CONFIG_DB into APPL_DB entries
//! and manages the hsflowd service lifecycle.
//!
//! ## Responsibilities
//! - Global sFlow enable/disable configuration
//! - Per-port sampling rate configuration
//! - Per-port admin state management
//! - Sample direction control (rx/tx/both)
//! - hsflowd service lifecycle management
//!
//! ## Configuration Sources
//! - `SFLOW` table: Global configuration
//! - `SFLOW_SESSION` table: Per-interface or "all" configuration
//! - `PORT` table: Port speed information
//! - `PORT_TABLE` (STATE_DB): Operational speed updates
//!
//! ## Key Features
//! - No shell commands for configuration (pure DB pass-through)
//! - Service control via systemd (hsflowd start/stop/restart)
//! - Default sampling rate equals port speed
//! - Local per-port configuration overrides global configuration

mod sflow_mgr;
mod tables;
mod types;

pub use sflow_mgr::SflowMgr;
pub use tables::*;
pub use types::*;
