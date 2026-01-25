//! FFI exports for PortsOrch.
//!
//! These functions allow C++ code to interact with the Rust PortsOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::ptr;

use sonic_sai::types::RawSaiObjectId;

use super::orch::{PortsOrch, PortsOrchConfig};

// Thread-local storage for the PortsOrch instance
thread_local! {
    static PORTS_ORCH: RefCell<Option<Box<PortsOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust PortsOrch instance for C++ access.
///
/// Called during orchagent startup to make the Rust PortsOrch
/// available to C++ code.
pub fn register_ports_orch(orch: Box<PortsOrch>) {
    PORTS_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust PortsOrch instance.
pub fn unregister_ports_orch() {
    PORTS_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the PortsOrch is registered.
#[no_mangle]
pub extern "C" fn rust_ports_orch_is_registered() -> bool {
    PORTS_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns the current count of ports.
#[no_mangle]
pub extern "C" fn rust_ports_orch_port_count() -> usize {
    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.port_count())
            .unwrap_or(0)
    })
}

/// Returns the current count of LAGs.
#[no_mangle]
pub extern "C" fn rust_ports_orch_lag_count() -> usize {
    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.lag_count())
            .unwrap_or(0)
    })
}

/// Returns the current count of VLANs.
#[no_mangle]
pub extern "C" fn rust_ports_orch_vlan_count() -> usize {
    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.vlan_count())
            .unwrap_or(0)
    })
}

/// Checks if a port exists.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_has_port(alias: *const c_char) -> bool {
    if alias.is_null() {
        return false;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.has_port(alias_str))
            .unwrap_or(false)
    })
}

/// Gets the SAI object ID for a port.
///
/// Returns 0 if the port doesn't exist.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_port_id(alias: *const c_char) -> RawSaiObjectId {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port(alias_str))
            .map(|port| port.sai_id())
            .unwrap_or(0)
    })
}

/// Gets the speed of a port in Mbps.
///
/// Returns 0 if the port doesn't exist.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_port_speed(alias: *const c_char) -> u32 {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port(alias_str))
            .map(|port| port.speed)
            .unwrap_or(0)
    })
}

/// Gets the MTU of a port.
///
/// Returns 0 if the port doesn't exist.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_port_mtu(alias: *const c_char) -> u32 {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port(alias_str))
            .map(|port| port.mtu)
            .unwrap_or(0)
    })
}

/// Returns true if the port is admin up.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_is_port_admin_up(alias: *const c_char) -> bool {
    if alias.is_null() {
        return false;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port(alias_str))
            .map(|port| port.is_admin_up())
            .unwrap_or(false)
    })
}

/// Returns true if the port is operationally up.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_is_port_oper_up(alias: *const c_char) -> bool {
    if alias.is_null() {
        return false;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port(alias_str))
            .map(|port| port.is_oper_up())
            .unwrap_or(false)
    })
}

/// Checks if a LAG exists.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_has_lag(alias: *const c_char) -> bool {
    if alias.is_null() {
        return false;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.has_lag(alias_str))
            .unwrap_or(false)
    })
}

/// Gets the SAI object ID for a LAG.
///
/// Returns 0 if the LAG doesn't exist.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_lag_id(alias: *const c_char) -> RawSaiObjectId {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_lag(alias_str))
            .map(|lag| lag.sai_id())
            .unwrap_or(0)
    })
}

/// Checks if a VLAN exists.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_has_vlan(alias: *const c_char) -> bool {
    if alias.is_null() {
        return false;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.has_vlan(alias_str))
            .unwrap_or(false)
    })
}

/// Gets the SAI object ID for a VLAN.
///
/// Returns 0 if the VLAN doesn't exist.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_vlan_id(alias: *const c_char) -> RawSaiObjectId {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_vlan(alias_str))
            .map(|vlan| vlan.sai_id())
            .unwrap_or(0)
    })
}

/// Gets the port alias by SAI object ID.
///
/// Returns null if the port doesn't exist.
/// The returned string must be freed by the caller using `rust_free_string`.
///
/// # Safety
///
/// - Caller must free the returned string using `rust_free_string`
#[no_mangle]
pub extern "C" fn rust_ports_orch_get_port_alias_by_oid(oid: RawSaiObjectId) -> *mut c_char {
    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port_by_oid(oid))
            .and_then(|port| CString::new(port.alias).ok())
            .map(|s| s.into_raw())
            .unwrap_or(ptr::null_mut())
    })
}

/// Frees a string allocated by Rust FFI functions.
///
/// # Safety
///
/// - `s` must have been allocated by a Rust FFI function that returns `*mut c_char`
/// - `s` must not be used after calling this function
#[no_mangle]
pub unsafe extern "C" fn rust_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Returns true if PortsOrch is initialized.
#[no_mangle]
pub extern "C" fn rust_ports_orch_is_initialized() -> bool {
    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_initialized())
            .unwrap_or(false)
    })
}

/// Gets the CPU port ID.
#[no_mangle]
pub extern "C" fn rust_ports_orch_get_cpu_port_id() -> RawSaiObjectId {
    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.cpu_port_id())
            .unwrap_or(0)
    })
}

/// Gets the default VLAN ID.
#[no_mangle]
pub extern "C" fn rust_ports_orch_get_default_vlan_id() -> u16 {
    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.default_vlan_id())
            .unwrap_or(1)
    })
}

/// Gets the LAG alias for a member port.
///
/// Returns null if the port is not a LAG member.
/// The returned string must be freed by the caller using `rust_free_string`.
///
/// # Safety
///
/// - `member_alias` must be a valid null-terminated C string
/// - Caller must free the returned string using `rust_free_string`
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_lag_for_member(
    member_alias: *const c_char,
) -> *mut c_char {
    if member_alias.is_null() {
        return ptr::null_mut();
    }

    let alias_str = match CStr::from_ptr(member_alias).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_lag_for_member(alias_str))
            .and_then(|lag_alias| CString::new(lag_alias).ok())
            .map(|s| s.into_raw())
            .unwrap_or(ptr::null_mut())
    })
}

/// Gets the number of lanes for a port.
///
/// Returns 0 if the port doesn't exist.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_port_lane_count(alias: *const c_char) -> usize {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port(alias_str))
            .map(|port| port.lane_count())
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_register_unregister() {
        // Start clean
        unregister_ports_orch();
        assert!(!rust_ports_orch_is_registered());

        // Register
        let orch = Box::new(PortsOrch::new(PortsOrchConfig::default()));
        register_ports_orch(orch);
        assert!(rust_ports_orch_is_registered());

        // Check initial state
        assert_eq!(rust_ports_orch_port_count(), 0);
        assert_eq!(rust_ports_orch_lag_count(), 0);
        assert_eq!(rust_ports_orch_vlan_count(), 0);

        // Unregister
        unregister_ports_orch();
        assert!(!rust_ports_orch_is_registered());
    }

    #[test]
    fn test_has_port_null_safety() {
        let result = unsafe { rust_ports_orch_has_port(ptr::null()) };
        assert!(!result);
    }

    #[test]
    fn test_get_port_id_not_found() {
        unregister_ports_orch();

        let orch = Box::new(PortsOrch::new(PortsOrchConfig::default()));
        register_ports_orch(orch);

        let alias_cstr = CString::new("Ethernet0").unwrap();
        let id = unsafe { rust_ports_orch_get_port_id(alias_cstr.as_ptr()) };
        assert_eq!(id, 0); // Not found

        unregister_ports_orch();
    }

    #[test]
    fn test_port_operations() {
        unregister_ports_orch();

        let mut orch = PortsOrch::new(PortsOrchConfig::default());
        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0, 1, 2, 3])
            .unwrap();

        register_ports_orch(Box::new(orch));

        let alias_cstr = CString::new("Ethernet0").unwrap();

        assert!(unsafe { rust_ports_orch_has_port(alias_cstr.as_ptr()) });
        assert_eq!(
            unsafe { rust_ports_orch_get_port_id(alias_cstr.as_ptr()) },
            0x1234
        );
        assert_eq!(
            unsafe { rust_ports_orch_get_port_lane_count(alias_cstr.as_ptr()) },
            4
        );

        unregister_ports_orch();
    }

    #[test]
    fn test_get_port_alias_by_oid() {
        unregister_ports_orch();

        let mut orch = PortsOrch::new(PortsOrchConfig::default());
        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1234, vec![0])
            .unwrap();

        register_ports_orch(Box::new(orch));

        let alias_ptr = rust_ports_orch_get_port_alias_by_oid(0x1234);
        assert!(!alias_ptr.is_null());

        let alias = unsafe { CStr::from_ptr(alias_ptr).to_str().unwrap() };
        assert_eq!(alias, "Ethernet0");

        // Free the string
        unsafe { rust_free_string(alias_ptr) };

        // Non-existent OID returns null
        let null_ptr = rust_ports_orch_get_port_alias_by_oid(0x9999);
        assert!(null_ptr.is_null());

        unregister_ports_orch();
    }

    #[test]
    fn test_lag_operations() {
        unregister_ports_orch();

        let mut orch = PortsOrch::new(PortsOrchConfig::default());
        orch.add_port_from_hardware("Ethernet0".to_string(), 0x1000, vec![0])
            .unwrap();
        orch.create_lag("PortChannel0001", 0x2000).unwrap();
        orch.add_lag_member("PortChannel0001", "Ethernet0").unwrap();

        register_ports_orch(Box::new(orch));

        let lag_cstr = CString::new("PortChannel0001").unwrap();
        assert!(unsafe { rust_ports_orch_has_lag(lag_cstr.as_ptr()) });
        assert_eq!(
            unsafe { rust_ports_orch_get_lag_id(lag_cstr.as_ptr()) },
            0x2000
        );

        let member_cstr = CString::new("Ethernet0").unwrap();
        let lag_alias_ptr = unsafe { rust_ports_orch_get_lag_for_member(member_cstr.as_ptr()) };
        assert!(!lag_alias_ptr.is_null());

        let lag_alias = unsafe { CStr::from_ptr(lag_alias_ptr).to_str().unwrap() };
        assert_eq!(lag_alias, "PortChannel0001");

        unsafe { rust_free_string(lag_alias_ptr) };

        unregister_ports_orch();
    }

    #[test]
    fn test_vlan_operations() {
        unregister_ports_orch();

        let mut orch = PortsOrch::new(PortsOrchConfig::default());
        orch.create_vlan("Vlan100", 100, 0x3000).unwrap();

        register_ports_orch(Box::new(orch));

        let vlan_cstr = CString::new("Vlan100").unwrap();
        assert!(unsafe { rust_ports_orch_has_vlan(vlan_cstr.as_ptr()) });
        assert_eq!(
            unsafe { rust_ports_orch_get_vlan_id(vlan_cstr.as_ptr()) },
            0x3000
        );

        unregister_ports_orch();
    }
}
