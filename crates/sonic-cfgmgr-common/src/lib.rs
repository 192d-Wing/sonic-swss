//! Common infrastructure for SONiC configuration manager daemons.
//!
//! This crate provides shared functionality for all cfgmgr daemons
//! (portmgrd, vlanmgrd, intfmgrd, etc.) in the Rust rewrite:
//!
//! - [`shell`]: Safe shell command execution with proper quoting
//! - [`CfgMgr`]: Base trait extending `Orch` for config managers
//! - [`error`]: Error types for cfgmgr operations
//!
//! # Architecture
//!
//! Configuration managers follow this pattern:
//!
//! 1. Subscribe to CONFIG_DB tables for configuration changes
//! 2. Monitor STATE_DB to track port/interface readiness
//! 3. Execute shell commands to configure the Linux network stack
//! 4. Write processed configuration to APPL_DB for orchagent
//!
//! # Example
//!
//! ```ignore
//! use sonic_cfgmgr_common::{
//!     CfgMgr, WarmRestartState,
//!     shell::{self, IP_CMD, shellquote},
//!     error::CfgMgrResult,
//! };
//!
//! async fn set_mtu(alias: &str, mtu: &str) -> CfgMgrResult<()> {
//!     let cmd = format!("{} link set dev {} mtu {}",
//!         IP_CMD, shellquote(alias), shellquote(mtu));
//!     shell::exec_or_throw(&cmd).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Migration from C++
//!
//! This crate provides Rust equivalents for the C++ cfgmgr infrastructure:
//!
//! | C++ | Rust |
//! |-----|------|
//! | `shellcmd.h` | [`shell`] module |
//! | `Orch` base class | [`CfgMgr`] trait + `sonic_orch_common::Orch` |
//! | `swss::exec()` | [`shell::exec()`] |
//! | `shellquote()` | [`shell::shellquote()`] |
//! | `EXEC_WITH_ERROR_THROW` | [`shell::exec_or_throw()`] |
//! | `WarmStart` class | [`WarmRestartState`] enum |

pub mod error;
pub mod manager;
pub mod shell;

// Re-export commonly used items at crate root
pub use error::{CfgMgrError, CfgMgrResult};
pub use manager::{
    defaults, CfgMgr, DbId, FieldValue, FieldValues, FieldValuesExt, WarmRestartState,
};

// Re-export the Orch trait for convenience
pub use sonic_orch_common::Orch;
