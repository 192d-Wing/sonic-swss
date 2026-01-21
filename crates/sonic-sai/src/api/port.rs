//! Safe wrapper for SAI port API.
//!
//! This module provides type-safe access to SAI port configuration and
//! management functions.

use crate::error::{SaiError, SaiResult};
use crate::types::{PortOid, SwitchOid};

/// Port speed in Mbps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortSpeed(u32);

impl PortSpeed {
    /// 1 Gigabit Ethernet
    pub const GE_1: Self = PortSpeed(1_000);
    /// 10 Gigabit Ethernet
    pub const GE_10: Self = PortSpeed(10_000);
    /// 25 Gigabit Ethernet
    pub const GE_25: Self = PortSpeed(25_000);
    /// 40 Gigabit Ethernet
    pub const GE_40: Self = PortSpeed(40_000);
    /// 50 Gigabit Ethernet
    pub const GE_50: Self = PortSpeed(50_000);
    /// 100 Gigabit Ethernet
    pub const GE_100: Self = PortSpeed(100_000);
    /// 200 Gigabit Ethernet
    pub const GE_200: Self = PortSpeed(200_000);
    /// 400 Gigabit Ethernet
    pub const GE_400: Self = PortSpeed(400_000);
    /// 800 Gigabit Ethernet
    pub const GE_800: Self = PortSpeed(800_000);

    /// Creates a new port speed from Mbps.
    pub const fn from_mbps(mbps: u32) -> Self {
        PortSpeed(mbps)
    }

    /// Returns the speed in Mbps.
    pub const fn as_mbps(&self) -> u32 {
        self.0
    }

    /// Returns the speed in Gbps.
    pub const fn as_gbps(&self) -> u32 {
        self.0 / 1_000
    }
}

/// Forward Error Correction (FEC) mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FecMode {
    /// No FEC
    #[default]
    None,
    /// Reed-Solomon FEC (RS)
    Rs,
    /// Fire Code FEC (FC)
    Fc,
    /// Auto-negotiate FEC
    Auto,
}

/// Port operational status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PortOperStatus {
    /// Port is operationally down
    #[default]
    Down,
    /// Port is operationally up
    Up,
    /// Port status is unknown
    Unknown,
    /// Port is in testing mode
    Testing,
    /// Port link is training
    NotPresent,
    /// Port is in lower layer down state
    LowerLayerDown,
}

/// Port configuration for creation.
#[derive(Debug, Clone)]
pub struct PortConfig {
    /// Hardware lane list for this port
    pub lanes: Vec<u32>,
    /// Port speed in Mbps
    pub speed: PortSpeed,
    /// Administrative state (up/down)
    pub admin_state: bool,
    /// FEC mode
    pub fec_mode: FecMode,
    /// Auto-negotiation enable
    pub auto_neg: bool,
    /// MTU size
    pub mtu: Option<u32>,
}

impl Default for PortConfig {
    fn default() -> Self {
        Self {
            lanes: vec![],
            speed: PortSpeed::GE_100,
            admin_state: false,
            fec_mode: FecMode::default(),
            auto_neg: false,
            mtu: None,
        }
    }
}

/// Safe wrapper for SAI port API.
///
/// This struct will hold the raw SAI port API pointer when FFI is enabled.
/// For now, it provides the interface definition.
pub struct PortApi {
    switch_id: SwitchOid,
    // When FFI is enabled:
    // api: *const sai_port_api_t,
}

impl PortApi {
    /// Creates a new PortApi instance.
    ///
    /// In production, this will query the SAI API table.
    pub fn new(switch_id: SwitchOid) -> Self {
        Self { switch_id }
    }

    /// Returns the switch ID this API is associated with.
    pub fn switch_id(&self) -> SwitchOid {
        self.switch_id
    }

    /// Creates a new port with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Port configuration including lanes, speed, etc.
    ///
    /// # Returns
    ///
    /// The OID of the newly created port on success.
    ///
    /// # Errors
    ///
    /// Returns an error if port creation fails (e.g., invalid lanes, table full).
    pub fn create_port(&self, config: &PortConfig) -> SaiResult<PortOid> {
        // Validate configuration
        if config.lanes.is_empty() {
            return Err(SaiError::invalid_parameter("lanes cannot be empty"));
        }

        if config.speed.as_mbps() == 0 {
            return Err(SaiError::invalid_parameter("speed cannot be zero"));
        }

        // TODO: When FFI is enabled, call sai_port_api->create_port()
        // For now, return a placeholder error
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Removes a port.
    ///
    /// # Errors
    ///
    /// Returns an error if the port is in use or doesn't exist.
    pub fn remove_port(&self, port: PortOid) -> SaiResult<()> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->remove_port()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Sets the administrative state of a port.
    ///
    /// # Arguments
    ///
    /// * `port` - The port to configure
    /// * `up` - true to bring the port up, false to bring it down
    pub fn set_admin_state(&self, port: PortOid, up: bool) -> SaiResult<()> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->set_port_attribute()
        let _ = up; // Suppress unused warning
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Gets the administrative state of a port.
    pub fn get_admin_state(&self, port: PortOid) -> SaiResult<bool> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->get_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Gets the operational status of a port.
    pub fn get_oper_status(&self, port: PortOid) -> SaiResult<PortOperStatus> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->get_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Sets the port speed.
    ///
    /// # Arguments
    ///
    /// * `port` - The port to configure
    /// * `speed` - The desired speed
    ///
    /// # Errors
    ///
    /// Returns an error if the speed is not supported by the port.
    pub fn set_speed(&self, port: PortOid, speed: PortSpeed) -> SaiResult<()> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // Validate speed is reasonable
        if speed.as_mbps() == 0 || speed.as_mbps() > 800_000 {
            return Err(SaiError::invalid_parameter(format!(
                "invalid speed: {} Mbps",
                speed.as_mbps()
            )));
        }

        // TODO: When FFI is enabled, call sai_port_api->set_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Gets the port speed.
    pub fn get_speed(&self, port: PortOid) -> SaiResult<PortSpeed> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->get_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Sets the FEC mode for a port.
    pub fn set_fec_mode(&self, port: PortOid, fec: FecMode) -> SaiResult<()> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->set_port_attribute()
        let _ = fec;
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Gets the FEC mode for a port.
    pub fn get_fec_mode(&self, port: PortOid) -> SaiResult<FecMode> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->get_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Sets the MTU for a port.
    ///
    /// # Arguments
    ///
    /// * `port` - The port to configure
    /// * `mtu` - The MTU size in bytes (typically 1500-9216)
    pub fn set_mtu(&self, port: PortOid, mtu: u32) -> SaiResult<()> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // Validate MTU is reasonable
        if mtu < 64 || mtu > 16383 {
            return Err(SaiError::invalid_parameter(format!(
                "invalid MTU: {} (must be 64-16383)",
                mtu
            )));
        }

        // TODO: When FFI is enabled, call sai_port_api->set_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Gets the MTU for a port.
    pub fn get_mtu(&self, port: PortOid) -> SaiResult<u32> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->get_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }

    /// Gets the hardware lane list for a port.
    pub fn get_lanes(&self, port: PortOid) -> SaiResult<Vec<u32>> {
        if port.is_null() {
            return Err(SaiError::invalid_parameter("port OID is null"));
        }

        // TODO: When FFI is enabled, call sai_port_api->get_port_attribute()
        Err(SaiError::not_supported("FFI not enabled"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_speed() {
        assert_eq!(PortSpeed::GE_100.as_mbps(), 100_000);
        assert_eq!(PortSpeed::GE_100.as_gbps(), 100);
        assert_eq!(PortSpeed::from_mbps(25_000), PortSpeed::GE_25);
    }

    #[test]
    fn test_port_config_default() {
        let config = PortConfig::default();
        assert!(config.lanes.is_empty());
        assert_eq!(config.speed, PortSpeed::GE_100);
        assert!(!config.admin_state);
    }

    #[test]
    fn test_port_api_null_validation() {
        let api = PortApi::new(SwitchOid::NULL);
        let null_port = PortOid::NULL;

        assert!(api.set_admin_state(null_port, true).is_err());
        assert!(api.get_admin_state(null_port).is_err());
        assert!(api.set_speed(null_port, PortSpeed::GE_100).is_err());
    }

    #[test]
    fn test_create_port_validation() {
        let api = PortApi::new(SwitchOid::NULL);

        // Empty lanes should fail
        let mut config = PortConfig::default();
        assert!(api.create_port(&config).is_err());

        // Zero speed should fail
        config.lanes = vec![0];
        config.speed = PortSpeed::from_mbps(0);
        assert!(api.create_port(&config).is_err());
    }
}
