//! Configuration manager trait and common abstractions.
//!
//! This module provides the base trait for all cfgmgr daemon managers,
//! extending the `Orch` trait from `sonic-orch-common` with cfgmgr-specific
//! functionality.

use async_trait::async_trait;
use sonic_orch_common::Orch;

/// Database identifiers used by cfgmgr daemons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DbId {
    /// Configuration database (CONFIG_DB) - source of configuration.
    ConfigDb,
    /// Application database (APPL_DB) - destination for processed config.
    ApplDb,
    /// State database (STATE_DB) - operational state tracking.
    StateDb,
}

impl DbId {
    /// Returns the database name as used in Redis/SONiC.
    pub fn name(&self) -> &'static str {
        match self {
            DbId::ConfigDb => "CONFIG_DB",
            DbId::ApplDb => "APPL_DB",
            DbId::StateDb => "STATE_DB",
        }
    }

    /// Returns the database ID number.
    pub fn id(&self) -> i32 {
        match self {
            DbId::ConfigDb => 4,
            DbId::ApplDb => 0,
            DbId::StateDb => 6,
        }
    }
}

/// Default values for port configuration.
pub mod defaults {
    /// Default admin status for ports.
    pub const DEFAULT_ADMIN_STATUS: &str = "down";

    /// Default MTU for ports and interfaces.
    pub const DEFAULT_MTU: &str = "9100";

    /// Default select timeout in milliseconds.
    pub const SELECT_TIMEOUT_MS: u64 = 1000;
}

/// Warm restart states matching the C++ WarmStart enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmRestartState {
    /// Warm restart is disabled.
    Disabled,
    /// System is initializing.
    Initialized,
    /// Restoring state from previous run.
    Restoring,
    /// State has been restored, waiting for reconciliation.
    Restored,
    /// Replaying configuration.
    Replayed,
    /// Reconciliation complete.
    Reconciled,
}

impl WarmRestartState {
    /// Returns the state name as a string for STATE_DB.
    pub fn as_str(&self) -> &'static str {
        match self {
            WarmRestartState::Disabled => "disabled",
            WarmRestartState::Initialized => "initialized",
            WarmRestartState::Restoring => "restoring",
            WarmRestartState::Restored => "restored",
            WarmRestartState::Replayed => "replayed",
            WarmRestartState::Reconciled => "reconciled",
        }
    }
}

/// Base trait for configuration manager daemons.
///
/// This trait extends `Orch` with functionality specific to cfgmgr daemons,
/// which bridge configuration from CONFIG_DB to APPL_DB and execute
/// shell commands to configure the Linux network stack.
///
/// # Example
///
/// ```ignore
/// use sonic_cfgmgr_common::{CfgMgr, WarmRestartState};
/// use sonic_orch_common::Orch;
///
/// struct MyMgr {
///     // ... state
/// }
///
/// #[async_trait]
/// impl Orch for MyMgr {
///     fn name(&self) -> &str { "MyMgr" }
///     async fn do_task(&mut self) { /* ... */ }
/// }
///
/// #[async_trait]
/// impl CfgMgr for MyMgr {
///     fn daemon_name(&self) -> &str { "mymgrd" }
///
///     fn is_warm_restart(&self) -> bool {
///         false
///     }
///
///     async fn set_warm_restart_state(&mut self, state: WarmRestartState) {
///         // ... update STATE_DB
///     }
/// }
/// ```
#[async_trait]
pub trait CfgMgr: Orch {
    /// Returns the daemon name (e.g., "portmgrd", "vlanmgrd").
    ///
    /// This is used for logging and warm restart state tracking.
    fn daemon_name(&self) -> &str;

    /// Returns true if warm restart mode is enabled for this daemon.
    fn is_warm_restart(&self) -> bool;

    /// Returns the current warm restart state.
    fn warm_restart_state(&self) -> WarmRestartState {
        WarmRestartState::Disabled
    }

    /// Sets the warm restart state in STATE_DB.
    ///
    /// This should update the WARM_RESTART_TABLE in STATE_DB with
    /// the new state for this daemon.
    async fn set_warm_restart_state(&mut self, state: WarmRestartState);

    /// Checks if warm restart replay is done.
    ///
    /// Returns true if all pending replay items have been processed.
    fn is_replay_done(&self) -> bool {
        true
    }

    /// Called during warm restart to build the replay list.
    ///
    /// Implementations should populate their internal replay list
    /// from CONFIG_DB tables.
    async fn build_replay_list(&mut self) {
        // Default: no-op
    }

    /// Returns the subscribed CONFIG_DB table names.
    fn config_table_names(&self) -> &[&str];

    /// Returns the subscribed STATE_DB table names.
    fn state_table_names(&self) -> &[&str] {
        &[]
    }

    /// Called when a port becomes ready in STATE_DB.
    ///
    /// Managers can override this to handle deferred configuration
    /// for ports that weren't ready during initial processing.
    async fn on_port_ready(&mut self, _port_alias: &str) {
        // Default: no-op
    }
}

/// Key-value tuple representing a field and its value.
pub type FieldValue = (String, String);

/// Collection of field-value pairs for a table entry.
pub type FieldValues = Vec<FieldValue>;

/// Helper trait for working with field-value collections.
pub trait FieldValuesExt {
    /// Gets the value for a field, if present.
    fn get_field(&self, field: &str) -> Option<&str>;

    /// Gets the value for a field, returning the default if not present.
    fn get_field_or<'a>(&'a self, field: &str, default: &'a str) -> &'a str;

    /// Checks if a field exists.
    fn has_field(&self, field: &str) -> bool;
}

impl FieldValuesExt for FieldValues {
    fn get_field(&self, field: &str) -> Option<&str> {
        self.iter()
            .find(|(f, _)| f == field)
            .map(|(_, v)| v.as_str())
    }

    fn get_field_or<'a>(&'a self, field: &str, default: &'a str) -> &'a str {
        self.get_field(field).unwrap_or(default)
    }

    fn has_field(&self, field: &str) -> bool {
        self.iter().any(|(f, _)| f == field)
    }
}

/// Builds a FieldValues collection from key-value pairs.
#[macro_export]
macro_rules! field_values {
    ($($field:expr => $value:expr),* $(,)?) => {
        vec![
            $(($field.to_string(), $value.to_string()),)*
        ]
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_id() {
        assert_eq!(DbId::ConfigDb.name(), "CONFIG_DB");
        assert_eq!(DbId::ConfigDb.id(), 4);
        assert_eq!(DbId::ApplDb.name(), "APPL_DB");
        assert_eq!(DbId::ApplDb.id(), 0);
        assert_eq!(DbId::StateDb.name(), "STATE_DB");
        assert_eq!(DbId::StateDb.id(), 6);
    }

    #[test]
    fn test_warm_restart_state() {
        assert_eq!(WarmRestartState::Disabled.as_str(), "disabled");
        assert_eq!(WarmRestartState::Reconciled.as_str(), "reconciled");
    }

    #[test]
    fn test_field_values_ext() {
        let fvs: FieldValues = vec![
            ("mtu".to_string(), "9100".to_string()),
            ("admin_status".to_string(), "up".to_string()),
        ];

        assert_eq!(fvs.get_field("mtu"), Some("9100"));
        assert_eq!(fvs.get_field("admin_status"), Some("up"));
        assert_eq!(fvs.get_field("nonexistent"), None);

        assert_eq!(fvs.get_field_or("mtu", "1500"), "9100");
        assert_eq!(fvs.get_field_or("nonexistent", "default"), "default");

        assert!(fvs.has_field("mtu"));
        assert!(!fvs.has_field("nonexistent"));
    }

    #[test]
    fn test_field_values_macro() {
        let fvs = field_values! {
            "mtu" => "9100",
            "admin_status" => "up",
        };

        assert_eq!(fvs.len(), 2);
        assert_eq!(fvs.get_field("mtu"), Some("9100"));
    }
}
