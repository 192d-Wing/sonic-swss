//! FFI bindings to call C++ code from Rust.
//!
//! These bindings allow Rust Orch modules to access functionality
//! that still lives in C++ during the migration period.

use sonic_sai::PortOid;
#[cfg(feature = "cpp-link")]
use std::ffi::CString;
use std::ffi::{c_char, CStr};
use thiserror::Error;

/// Error type for FFI operations.
#[derive(Debug, Clone, Error)]
pub enum FfiError {
    #[error("Null pointer received from C++")]
    NullPointer,

    #[error("Invalid UTF-8 string from C++")]
    InvalidUtf8,

    #[error("C++ function returned error: {message}")]
    CppError { message: String },

    #[error("Object not found: {name}")]
    NotFound { name: String },
}

/// Result type for FFI operations.
pub type FfiResult<T> = Result<T, FfiError>;

// =============================================================================
// C++ function declarations (extern "C")
// =============================================================================

// These would be linked against the C++ orchagent library.
// For now, they are placeholder declarations that will be implemented
// when the build system integrates the Rust and C++ code.

#[cfg(feature = "cpp-link")]
extern "C" {
    // PortsOrch functions
    fn gPortsOrch_getPort(alias: *const c_char, port_out: *mut CppPort) -> bool;
    fn gPortsOrch_allPortsReady() -> bool;
    fn gPortsOrch_getPortCount() -> u32;

    // NeighOrch functions
    fn gNeighOrch_hasNextHop(key: *const c_char) -> bool;
    fn gNeighOrch_getNextHopId(key: *const c_char) -> u64;

    // RouteOrch functions
    fn gRouteOrch_getNextHopGroupId(key: *const c_char) -> u64;

    // IntfsOrch functions
    fn gIntfsOrch_isIntfExists(intf_name: *const c_char) -> bool;
    fn gIntfsOrch_getRouterIntfId(intf_name: *const c_char) -> u64;
}

// =============================================================================
// C++ type representations
// =============================================================================

/// C-compatible representation of a Port (matches C++ Port class layout).
///
/// This is used to receive port data from C++ gPortsOrch.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CppPort {
    /// Port alias (e.g., "Ethernet0")
    pub alias: [c_char; 64],
    /// SAI port OID
    pub port_id: u64,
    /// Port index
    pub index: u32,
    /// Port speed in Mbps
    pub speed: u32,
    /// MTU
    pub mtu: u32,
    /// Admin state (0 = down, 1 = up)
    pub admin_state: u8,
    /// Oper state (0 = down, 1 = up)
    pub oper_state: u8,
    /// Port type
    pub port_type: u8,
    /// Padding for alignment
    _padding: u8,
}

impl Default for CppPort {
    fn default() -> Self {
        Self {
            alias: [0; 64],
            port_id: 0,
            index: 0,
            speed: 0,
            mtu: 0,
            admin_state: 0,
            oper_state: 0,
            port_type: 0,
            _padding: 0,
        }
    }
}

impl CppPort {
    /// Returns the alias as a Rust string.
    pub fn alias_str(&self) -> FfiResult<&str> {
        // Safety: alias is a fixed-size array that should be null-terminated
        let c_str = unsafe { CStr::from_ptr(self.alias.as_ptr()) };
        c_str.to_str().map_err(|_| FfiError::InvalidUtf8)
    }

    /// Returns the port OID.
    pub fn port_oid(&self) -> Option<PortOid> {
        PortOid::from_raw(self.port_id)
    }
}

// =============================================================================
// Safe Rust wrappers for C++ functions
// =============================================================================

/// Wrapper for accessing C++ gPortsOrch.
pub struct CppPortsOrch;

impl CppPortsOrch {
    /// Gets a port by alias from C++ gPortsOrch.
    ///
    /// # Safety
    ///
    /// This function is safe to call as long as:
    /// - gPortsOrch is initialized
    /// - The caller is on the orchagent main thread
    #[cfg(feature = "cpp-link")]
    pub fn get_port(alias: &str) -> FfiResult<CppPort> {
        let c_alias = CString::new(alias).map_err(|_| FfiError::InvalidUtf8)?;
        let mut port = CppPort::default();

        // Safety: gPortsOrch_getPort is thread-safe and port_out is valid
        let found = unsafe { gPortsOrch_getPort(c_alias.as_ptr(), &mut port) };

        if found {
            Ok(port)
        } else {
            Err(FfiError::NotFound {
                name: alias.to_string(),
            })
        }
    }

    /// Stub implementation when C++ linking is not enabled.
    #[cfg(not(feature = "cpp-link"))]
    pub fn get_port(alias: &str) -> FfiResult<CppPort> {
        log::warn!("CppPortsOrch::get_port called without cpp-link feature");
        Err(FfiError::NotFound {
            name: alias.to_string(),
        })
    }

    /// Returns true if all ports are ready.
    #[cfg(feature = "cpp-link")]
    pub fn all_ports_ready() -> bool {
        unsafe { gPortsOrch_allPortsReady() }
    }

    #[cfg(not(feature = "cpp-link"))]
    pub fn all_ports_ready() -> bool {
        log::warn!("CppPortsOrch::all_ports_ready called without cpp-link feature");
        false
    }

    /// Returns the number of ports.
    #[cfg(feature = "cpp-link")]
    pub fn port_count() -> u32 {
        unsafe { gPortsOrch_getPortCount() }
    }

    #[cfg(not(feature = "cpp-link"))]
    pub fn port_count() -> u32 {
        log::warn!("CppPortsOrch::port_count called without cpp-link feature");
        0
    }
}

/// Wrapper for accessing C++ gNeighOrch.
pub struct CppNeighOrch;

impl CppNeighOrch {
    /// Checks if a next-hop exists.
    #[cfg(feature = "cpp-link")]
    pub fn has_next_hop(key: &str) -> FfiResult<bool> {
        let c_key = CString::new(key).map_err(|_| FfiError::InvalidUtf8)?;
        Ok(unsafe { gNeighOrch_hasNextHop(c_key.as_ptr()) })
    }

    #[cfg(not(feature = "cpp-link"))]
    pub fn has_next_hop(_key: &str) -> FfiResult<bool> {
        log::warn!("CppNeighOrch::has_next_hop called without cpp-link feature");
        Ok(false)
    }

    /// Gets the SAI next-hop ID for a neighbor.
    #[cfg(feature = "cpp-link")]
    pub fn get_next_hop_id(key: &str) -> FfiResult<u64> {
        let c_key = CString::new(key).map_err(|_| FfiError::InvalidUtf8)?;
        let id = unsafe { gNeighOrch_getNextHopId(c_key.as_ptr()) };
        if id == 0 {
            Err(FfiError::NotFound {
                name: key.to_string(),
            })
        } else {
            Ok(id)
        }
    }

    #[cfg(not(feature = "cpp-link"))]
    pub fn get_next_hop_id(key: &str) -> FfiResult<u64> {
        log::warn!("CppNeighOrch::get_next_hop_id called without cpp-link feature");
        Err(FfiError::NotFound {
            name: key.to_string(),
        })
    }
}

/// Wrapper for accessing C++ gIntfsOrch.
pub struct CppIntfsOrch;

impl CppIntfsOrch {
    /// Checks if an interface exists.
    #[cfg(feature = "cpp-link")]
    pub fn is_intf_exists(intf_name: &str) -> FfiResult<bool> {
        let c_name = CString::new(intf_name).map_err(|_| FfiError::InvalidUtf8)?;
        Ok(unsafe { gIntfsOrch_isIntfExists(c_name.as_ptr()) })
    }

    #[cfg(not(feature = "cpp-link"))]
    pub fn is_intf_exists(_intf_name: &str) -> FfiResult<bool> {
        log::warn!("CppIntfsOrch::is_intf_exists called without cpp-link feature");
        Ok(false)
    }

    /// Gets the router interface ID for an interface.
    #[cfg(feature = "cpp-link")]
    pub fn get_router_intf_id(intf_name: &str) -> FfiResult<u64> {
        let c_name = CString::new(intf_name).map_err(|_| FfiError::InvalidUtf8)?;
        let id = unsafe { gIntfsOrch_getRouterIntfId(c_name.as_ptr()) };
        if id == 0 {
            Err(FfiError::NotFound {
                name: intf_name.to_string(),
            })
        } else {
            Ok(id)
        }
    }

    #[cfg(not(feature = "cpp-link"))]
    pub fn get_router_intf_id(intf_name: &str) -> FfiResult<u64> {
        log::warn!("CppIntfsOrch::get_router_intf_id called without cpp-link feature");
        Err(FfiError::NotFound {
            name: intf_name.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpp_port_default() {
        let port = CppPort::default();
        assert_eq!(port.port_id, 0);
        assert_eq!(port.speed, 0);
    }

    #[test]
    fn test_cpp_ports_orch_stub() {
        // Without cpp-link feature, these should return errors/defaults
        assert!(CppPortsOrch::get_port("Ethernet0").is_err());
        assert!(!CppPortsOrch::all_ports_ready());
        assert_eq!(CppPortsOrch::port_count(), 0);
    }
}
