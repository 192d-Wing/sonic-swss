//! FFI exports for MlagOrch.
//!
//! These functions allow C++ code to interact with the Rust MlagOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};

use super::orch::{MlagOrch, MlagOrchConfig};

// Thread-local storage for the MlagOrch instance
thread_local! {
    static MLAG_ORCH: RefCell<Option<Box<MlagOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust MlagOrch instance for C++ access.
pub fn register_mlag_orch(orch: Box<MlagOrch>) {
    MLAG_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust MlagOrch instance.
pub fn unregister_mlag_orch() {
    MLAG_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the MlagOrch is registered.
#[no_mangle]
pub extern "C" fn rust_mlag_orch_is_registered() -> bool {
    MLAG_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns true if the MlagOrch is initialized.
#[no_mangle]
pub extern "C" fn rust_mlag_orch_is_initialized() -> bool {
    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_initialized())
            .unwrap_or(false)
    })
}

/// Returns true if the given interface is the ISL.
///
/// # Safety
///
/// - `if_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_mlag_orch_is_isl_interface(if_name: *const c_char) -> bool {
    if if_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(if_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_isl_interface(name_str))
            .unwrap_or(false)
    })
}

/// Returns true if the given interface is an MLAG member.
///
/// # Safety
///
/// - `if_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_mlag_orch_is_mlag_interface(if_name: *const c_char) -> bool {
    if if_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(if_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_mlag_interface(name_str))
            .unwrap_or(false)
    })
}

/// Returns the number of MLAG interfaces.
#[no_mangle]
pub extern "C" fn rust_mlag_orch_mlag_interface_count() -> usize {
    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.mlag_interface_count())
            .unwrap_or(0)
    })
}

/// Adds or updates the ISL interface.
///
/// # Safety
///
/// - `isl_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_mlag_orch_add_isl_interface(isl_name: *const c_char) -> bool {
    if isl_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(isl_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    MLAG_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.add_isl_interface(name_str).is_ok())
            .unwrap_or(false)
    })
}

/// Removes the ISL interface.
#[no_mangle]
pub extern "C" fn rust_mlag_orch_del_isl_interface() -> bool {
    MLAG_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.del_isl_interface().is_ok())
            .unwrap_or(false)
    })
}

/// Adds an MLAG member interface.
///
/// # Safety
///
/// - `if_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_mlag_orch_add_mlag_interface(if_name: *const c_char) -> bool {
    if if_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(if_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    MLAG_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.add_mlag_interface(name_str).is_ok())
            .unwrap_or(false)
    })
}

/// Removes an MLAG member interface.
///
/// # Safety
///
/// - `if_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_mlag_orch_del_mlag_interface(if_name: *const c_char) -> bool {
    if if_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(if_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    MLAG_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.del_mlag_interface(name_str).is_ok())
            .unwrap_or(false)
    })
}

/// Gets the number of ISL adds (statistic).
#[no_mangle]
pub extern "C" fn rust_mlag_orch_stats_isl_adds() -> u64 {
    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().isl_adds)
            .unwrap_or(0)
    })
}

/// Gets the number of ISL deletes (statistic).
#[no_mangle]
pub extern "C" fn rust_mlag_orch_stats_isl_deletes() -> u64 {
    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().isl_deletes)
            .unwrap_or(0)
    })
}

/// Gets the number of interface adds (statistic).
#[no_mangle]
pub extern "C" fn rust_mlag_orch_stats_intf_adds() -> u64 {
    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().intf_adds)
            .unwrap_or(0)
    })
}

/// Gets the number of interface deletes (statistic).
#[no_mangle]
pub extern "C" fn rust_mlag_orch_stats_intf_deletes() -> u64 {
    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().intf_deletes)
            .unwrap_or(0)
    })
}

/// Gets the number of notifications sent (statistic).
#[no_mangle]
pub extern "C" fn rust_mlag_orch_stats_notifications() -> u64 {
    MLAG_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().notifications)
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
        unregister_mlag_orch();
        assert!(!rust_mlag_orch_is_registered());

        // Register
        let orch = Box::new(MlagOrch::new(MlagOrchConfig::default()));
        register_mlag_orch(orch);
        assert!(rust_mlag_orch_is_registered());

        // Check initial state
        assert!(!rust_mlag_orch_is_initialized());
        assert_eq!(rust_mlag_orch_mlag_interface_count(), 0);

        // Unregister
        unregister_mlag_orch();
        assert!(!rust_mlag_orch_is_registered());
    }

    #[test]
    fn test_isl_operations() {
        unregister_mlag_orch();
        let orch = Box::new(MlagOrch::new(MlagOrchConfig::default()));
        register_mlag_orch(orch);

        let isl_name = CString::new("PortChannel100").unwrap();

        unsafe {
            // Add ISL
            assert!(rust_mlag_orch_add_isl_interface(isl_name.as_ptr()));
            assert!(rust_mlag_orch_is_isl_interface(isl_name.as_ptr()));

            // Delete ISL
            assert!(rust_mlag_orch_del_isl_interface());
            assert!(!rust_mlag_orch_is_isl_interface(isl_name.as_ptr()));
        }

        unregister_mlag_orch();
    }

    #[test]
    fn test_mlag_interface_operations() {
        unregister_mlag_orch();
        let orch = Box::new(MlagOrch::new(MlagOrchConfig::default()));
        register_mlag_orch(orch);

        let if_name = CString::new("Ethernet0").unwrap();

        unsafe {
            // Add interface
            assert!(rust_mlag_orch_add_mlag_interface(if_name.as_ptr()));
            assert!(rust_mlag_orch_is_mlag_interface(if_name.as_ptr()));
            assert_eq!(rust_mlag_orch_mlag_interface_count(), 1);

            // Delete interface
            assert!(rust_mlag_orch_del_mlag_interface(if_name.as_ptr()));
            assert!(!rust_mlag_orch_is_mlag_interface(if_name.as_ptr()));
            assert_eq!(rust_mlag_orch_mlag_interface_count(), 0);
        }

        unregister_mlag_orch();
    }

    #[test]
    fn test_null_safety() {
        unregister_mlag_orch();
        let orch = Box::new(MlagOrch::new(MlagOrchConfig::default()));
        register_mlag_orch(orch);

        unsafe {
            assert!(!rust_mlag_orch_is_isl_interface(std::ptr::null()));
            assert!(!rust_mlag_orch_is_mlag_interface(std::ptr::null()));
            assert!(!rust_mlag_orch_add_isl_interface(std::ptr::null()));
            assert!(!rust_mlag_orch_add_mlag_interface(std::ptr::null()));
            assert!(!rust_mlag_orch_del_mlag_interface(std::ptr::null()));
        }

        unregister_mlag_orch();
    }

    #[test]
    fn test_statistics() {
        unregister_mlag_orch();
        let orch = Box::new(MlagOrch::new(MlagOrchConfig::default()));
        register_mlag_orch(orch);

        let isl_name = CString::new("PortChannel100").unwrap();
        let if_name = CString::new("Ethernet0").unwrap();

        unsafe {
            rust_mlag_orch_add_isl_interface(isl_name.as_ptr());
            rust_mlag_orch_del_isl_interface();
            rust_mlag_orch_add_mlag_interface(if_name.as_ptr());
            rust_mlag_orch_del_mlag_interface(if_name.as_ptr());
        }

        assert_eq!(rust_mlag_orch_stats_isl_adds(), 1);
        assert_eq!(rust_mlag_orch_stats_isl_deletes(), 1);
        assert_eq!(rust_mlag_orch_stats_intf_adds(), 1);
        assert_eq!(rust_mlag_orch_stats_intf_deletes(), 1);
        assert_eq!(rust_mlag_orch_stats_notifications(), 4);

        unregister_mlag_orch();
    }
}
