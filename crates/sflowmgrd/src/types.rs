//! Type definitions for sflowmgrd

use serde::{Deserialize, Serialize};

/// Per-port sFlow configuration information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SflowPortInfo {
    /// Whether local rate configuration is present
    pub local_rate_cfg: bool,

    /// Whether local admin configuration is present
    pub local_admin_cfg: bool,

    /// Whether local direction configuration is present
    pub local_dir_cfg: bool,

    /// Configured port speed from CONFIG_DB
    pub speed: String,

    /// Operational port speed from STATE_DB
    pub oper_speed: String,

    /// Configured sampling rate (packets per sample)
    pub rate: String,

    /// Admin state ("up" or "down")
    pub admin: String,

    /// Sample direction ("rx", "tx", or "both")
    pub dir: String,
}

impl SflowPortInfo {
    /// Creates a new SflowPortInfo with default values
    pub fn new() -> Self {
        Self {
            local_rate_cfg: false,
            local_admin_cfg: false,
            local_dir_cfg: false,
            speed: crate::constants::ERROR_SPEED.to_string(),
            oper_speed: crate::constants::NA_SPEED.to_string(),
            rate: String::new(),
            admin: String::new(),
            dir: String::new(),
        }
    }

    /// Checks if this port has any local configuration
    pub fn has_local_config(&self) -> bool {
        self.local_rate_cfg || self.local_admin_cfg || self.local_dir_cfg
    }

    /// Clears all local configuration flags and values
    pub fn clear_local_config(&mut self) {
        self.local_rate_cfg = false;
        self.local_admin_cfg = false;
        self.local_dir_cfg = false;
        self.rate = String::new();
        self.admin = String::new();
        self.dir = String::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sflow_port_info_new() {
        let info = SflowPortInfo::new();
        assert!(!info.local_rate_cfg);
        assert!(!info.local_admin_cfg);
        assert!(!info.local_dir_cfg);
        assert_eq!(info.speed, "error");
        assert_eq!(info.oper_speed, "N/A");
        assert!(info.rate.is_empty());
        assert!(info.admin.is_empty());
        assert!(info.dir.is_empty());
    }

    #[test]
    fn test_has_local_config() {
        let mut info = SflowPortInfo::new();
        assert!(!info.has_local_config());

        info.local_rate_cfg = true;
        assert!(info.has_local_config());

        info.local_rate_cfg = false;
        info.local_admin_cfg = true;
        assert!(info.has_local_config());

        info.local_admin_cfg = false;
        info.local_dir_cfg = true;
        assert!(info.has_local_config());
    }

    #[test]
    fn test_clear_local_config() {
        let mut info = SflowPortInfo::new();
        info.local_rate_cfg = true;
        info.local_admin_cfg = true;
        info.local_dir_cfg = true;
        info.rate = "1000".to_string();
        info.admin = "up".to_string();
        info.dir = "rx".to_string();

        info.clear_local_config();

        assert!(!info.local_rate_cfg);
        assert!(!info.local_admin_cfg);
        assert!(!info.local_dir_cfg);
        assert!(info.rate.is_empty());
        assert!(info.admin.is_empty());
        assert!(info.dir.is_empty());
    }
}
