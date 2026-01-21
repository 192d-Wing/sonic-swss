//! FFI exports for RouteOrch.
//!
//! These functions allow C++ code to interact with the Rust RouteOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use sonic_sai::types::RawSaiObjectId;

use super::nhg::NextHopGroupKey;
use super::orch::RouteOrch;

// Thread-local storage for the RouteOrch instance
thread_local! {
    static ROUTE_ORCH: RefCell<Option<Box<RouteOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust RouteOrch instance for C++ access.
///
/// Called during orchagent startup to make the Rust RouteOrch
/// available to C++ code.
pub fn register_route_orch(orch: Box<RouteOrch>) {
    ROUTE_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust RouteOrch instance.
pub fn unregister_route_orch() {
    ROUTE_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the RouteOrch is registered.
#[no_mangle]
pub extern "C" fn rust_route_orch_is_registered() -> bool {
    ROUTE_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns the current count of next-hop groups.
#[no_mangle]
pub extern "C" fn rust_route_orch_nhg_count() -> usize {
    ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.nhg_count())
            .unwrap_or(0)
    })
}

/// Returns the maximum number of next-hop groups.
#[no_mangle]
pub extern "C" fn rust_route_orch_max_nhg_count() -> usize {
    ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.max_nhg_count())
            .unwrap_or(0)
    })
}

/// Checks if a next-hop group exists.
///
/// # Safety
///
/// - `nhg_key_str` must be a valid null-terminated C string in the format
///   "ip1@alias1,ip2@alias2,..."
#[no_mangle]
pub unsafe extern "C" fn rust_route_orch_has_nhg(nhg_key_str: *const c_char) -> bool {
    if nhg_key_str.is_null() {
        return false;
    }

    let key_str = match CStr::from_ptr(nhg_key_str).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let key = match key_str.parse::<NextHopGroupKey>() {
        Ok(k) => k,
        Err(_) => return false,
    };

    ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.has_nhg(&key))
            .unwrap_or(false)
    })
}

/// Returns true if the next-hop group's ref count is zero.
///
/// # Safety
///
/// - `nhg_key_str` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_route_orch_is_nhg_ref_count_zero(
    nhg_key_str: *const c_char,
) -> bool {
    if nhg_key_str.is_null() {
        return true;
    }

    let key_str = match CStr::from_ptr(nhg_key_str).to_str() {
        Ok(s) => s,
        Err(_) => return true,
    };

    let key = match key_str.parse::<NextHopGroupKey>() {
        Ok(k) => k,
        Err(_) => return true,
    };

    ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_nhg_ref_count_zero(&key))
            .unwrap_or(true)
    })
}

/// Checks if a route exists.
///
/// # Safety
///
/// - `prefix_str` must be a valid null-terminated C string in CIDR format (e.g., "10.0.0.0/24")
#[no_mangle]
pub unsafe extern "C" fn rust_route_orch_has_route(
    vrf_id: RawSaiObjectId,
    prefix_str: *const c_char,
) -> bool {
    if prefix_str.is_null() {
        return false;
    }

    let prefix_cstr = match CStr::from_ptr(prefix_str).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let prefix = match prefix_cstr.parse() {
        Ok(p) => p,
        Err(_) => return false,
    };

    ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.has_route(vrf_id, &prefix))
            .unwrap_or(false)
    })
}

/// Gets the SAI object ID for a next-hop group.
///
/// Returns 0 if the NHG doesn't exist.
///
/// # Safety
///
/// - `nhg_key_str` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_route_orch_get_nhg_id(
    nhg_key_str: *const c_char,
) -> RawSaiObjectId {
    if nhg_key_str.is_null() {
        return 0;
    }

    let key_str = match CStr::from_ptr(nhg_key_str).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let key = match key_str.parse::<NextHopGroupKey>() {
        Ok(k) => k,
        Err(_) => return 0,
    };

    ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_nhg(&key))
            .map(|entry| entry.sai_id())
            .unwrap_or(0)
    })
}

/// Gets the reference count for a next-hop group.
///
/// Returns 0 if the NHG doesn't exist.
///
/// # Safety
///
/// - `nhg_key_str` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_route_orch_get_nhg_ref_count(
    nhg_key_str: *const c_char,
) -> u32 {
    if nhg_key_str.is_null() {
        return 0;
    }

    let key_str = match CStr::from_ptr(nhg_key_str).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let key = match key_str.parse::<NextHopGroupKey>() {
        Ok(k) => k,
        Err(_) => return 0,
    };

    ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_nhg(&key))
            .map(|entry| entry.ref_count())
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::orch::RouteOrchConfig;
    use std::ffi::CString;

    #[test]
    fn test_register_unregister() {
        // Start clean
        unregister_route_orch();
        assert!(!rust_route_orch_is_registered());

        // Register
        let orch = Box::new(RouteOrch::new(RouteOrchConfig::default()));
        register_route_orch(orch);
        assert!(rust_route_orch_is_registered());

        // Check initial state
        assert_eq!(rust_route_orch_nhg_count(), 0);
        assert_eq!(rust_route_orch_max_nhg_count(), 1024);

        // Unregister
        unregister_route_orch();
        assert!(!rust_route_orch_is_registered());
    }

    #[test]
    fn test_has_nhg_null_safety() {
        let result = unsafe { rust_route_orch_has_nhg(std::ptr::null()) };
        assert!(!result);
    }

    #[test]
    fn test_has_route_null_safety() {
        let result = unsafe { rust_route_orch_has_route(0, std::ptr::null()) };
        assert!(!result);
    }

    #[test]
    fn test_get_nhg_id_not_found() {
        unregister_route_orch();

        let orch = Box::new(RouteOrch::new(RouteOrchConfig::default()));
        register_route_orch(orch);

        let key_cstr = CString::new("192.168.1.1@Ethernet0").unwrap();
        let id = unsafe { rust_route_orch_get_nhg_id(key_cstr.as_ptr()) };
        assert_eq!(id, 0); // Not found

        unregister_route_orch();
    }

    #[test]
    fn test_ref_count_zero_for_nonexistent() {
        unregister_route_orch();

        let orch = Box::new(RouteOrch::new(RouteOrchConfig::default()));
        register_route_orch(orch);

        let key_cstr = CString::new("192.168.1.1@Ethernet0").unwrap();
        let is_zero = unsafe { rust_route_orch_is_nhg_ref_count_zero(key_cstr.as_ptr()) };
        assert!(is_zero); // Returns true for non-existent NHG

        unregister_route_orch();
    }
}
