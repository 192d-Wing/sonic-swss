//! FFI exports for VRFOrch.
//!
//! These functions allow C++ code to interact with the Rust VRFOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use sonic_sai::types::RawSaiObjectId;

use super::orch::{VrfOrch, VrfOrchConfig};
use super::types::Vni;

// Thread-local storage for the VRFOrch instance
thread_local! {
    static VRF_ORCH: RefCell<Option<Box<VrfOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust VRFOrch instance for C++ access.
pub fn register_vrf_orch(orch: Box<VrfOrch>) {
    VRF_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust VRFOrch instance.
pub fn unregister_vrf_orch() {
    VRF_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the VRFOrch is registered.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_is_registered() -> bool {
    VRF_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns the number of VRFs.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_vrf_count() -> usize {
    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.vrf_count())
            .unwrap_or(0)
    })
}

/// Checks if a VRF exists.
///
/// # Safety
///
/// - `vrf_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_vrf_orch_vrf_exists(vrf_name: *const c_char) -> bool {
    if vrf_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(vrf_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.vrf_exists(name_str))
            .unwrap_or(false)
    })
}

/// Gets the VRF ID for a name.
///
/// Returns the global VRF ID if the name is empty or not found.
///
/// # Safety
///
/// - `vrf_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_vrf_orch_get_vrf_id(vrf_name: *const c_char) -> RawSaiObjectId {
    if vrf_name.is_null() {
        return VRF_ORCH.with(|cell| {
            cell.borrow()
                .as_ref()
                .map(|orch| orch.config().global_vrf_id)
                .unwrap_or(0)
        });
    }

    let name_str = match CStr::from_ptr(vrf_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            return VRF_ORCH.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .map(|orch| orch.config().global_vrf_id)
                    .unwrap_or(0)
            })
        }
    };

    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.get_vrf_id(name_str))
            .unwrap_or(0)
    })
}

/// Increases the reference count for a VRF.
///
/// Returns the new ref count, or -1 on error.
///
/// # Safety
///
/// - `vrf_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_vrf_orch_increase_ref_count(vrf_name: *const c_char) -> i32 {
    if vrf_name.is_null() {
        return -1;
    }

    let name_str = match CStr::from_ptr(vrf_name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    VRF_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .and_then(|orch| orch.increase_vrf_ref_count(name_str).ok())
            .unwrap_or(-1)
    })
}

/// Increases the reference count for a VRF by ID.
///
/// Does nothing for the global VRF.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_increase_ref_count_by_id(vrf_id: RawSaiObjectId) -> i32 {
    VRF_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .and_then(|orch| orch.increase_vrf_ref_count_by_id(vrf_id).ok())
            .unwrap_or(-1)
    })
}

/// Decreases the reference count for a VRF.
///
/// Returns the new ref count, or -1 on error.
///
/// # Safety
///
/// - `vrf_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_vrf_orch_decrease_ref_count(vrf_name: *const c_char) -> i32 {
    if vrf_name.is_null() {
        return -1;
    }

    let name_str = match CStr::from_ptr(vrf_name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    VRF_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .and_then(|orch| orch.decrease_vrf_ref_count(name_str).ok())
            .unwrap_or(-1)
    })
}

/// Decreases the reference count for a VRF by ID.
///
/// Does nothing for the global VRF.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_decrease_ref_count_by_id(vrf_id: RawSaiObjectId) -> i32 {
    VRF_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .and_then(|orch| orch.decrease_vrf_ref_count_by_id(vrf_id).ok())
            .unwrap_or(-1)
    })
}

/// Gets the reference count for a VRF.
///
/// Returns -1 if not found.
///
/// # Safety
///
/// - `vrf_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_vrf_orch_get_ref_count(vrf_name: *const c_char) -> i32 {
    if vrf_name.is_null() {
        return -1;
    }

    let name_str = match CStr::from_ptr(vrf_name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.get_vrf_ref_count(name_str))
            .unwrap_or(-1)
    })
}

/// Gets the VNI mapped to a VRF.
///
/// Returns 0 if not mapped.
///
/// # Safety
///
/// - `vrf_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_vrf_orch_get_mapped_vni(vrf_name: *const c_char) -> Vni {
    if vrf_name.is_null() {
        return 0;
    }

    let name_str = match CStr::from_ptr(vrf_name).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.get_vrf_mapped_vni(name_str))
            .unwrap_or(0)
    })
}

/// Gets the VLAN ID for an L3 VNI.
///
/// Returns -1 if not found.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_get_l3_vni_vlan(vni: Vni) -> i32 {
    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_l3_vni_vlan(vni))
            .map(|v| v as i32)
            .unwrap_or(-1)
    })
}

/// Returns true if the VNI is an L3 VNI.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_is_l3_vni(vni: Vni) -> bool {
    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_l3_vni(vni))
            .unwrap_or(false)
    })
}

/// Updates the L3 VNI VLAN mapping.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_update_l3_vni_vlan(vni: Vni, vlan_id: u16) -> i32 {
    VRF_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .and_then(|orch| orch.update_l3_vni_vlan(vni, vlan_id).ok())
            .map(|_| 0)
            .unwrap_or(-1)
    })
}

/// Gets the global VRF ID.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_get_global_vrf_id() -> RawSaiObjectId {
    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.config().global_vrf_id)
            .unwrap_or(0)
    })
}

/// Returns true if the VRFOrch is initialized.
#[no_mangle]
pub extern "C" fn rust_vrf_orch_is_initialized() -> bool {
    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_initialized())
            .unwrap_or(false)
    })
}

/// Gets the number of VRFs created (statistic).
#[no_mangle]
pub extern "C" fn rust_vrf_orch_stats_vrfs_created() -> u64 {
    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().vrfs_created)
            .unwrap_or(0)
    })
}

/// Gets the number of VRFs removed (statistic).
#[no_mangle]
pub extern "C" fn rust_vrf_orch_stats_vrfs_removed() -> u64 {
    VRF_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().vrfs_removed)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_register_unregister() {
        // Start clean
        unregister_vrf_orch();
        assert!(!rust_vrf_orch_is_registered());

        // Register
        let orch = Box::new(VrfOrch::new(VrfOrchConfig::default()));
        register_vrf_orch(orch);
        assert!(rust_vrf_orch_is_registered());

        // Check initial state
        assert_eq!(rust_vrf_orch_vrf_count(), 0);

        // Unregister
        unregister_vrf_orch();
        assert!(!rust_vrf_orch_is_registered());
    }

    #[test]
    fn test_vrf_exists_null_safety() {
        let result = unsafe { rust_vrf_orch_vrf_exists(ptr::null()) };
        assert!(!result);
    }

    #[test]
    fn test_get_vrf_id_null() {
        unregister_vrf_orch();

        let orch = Box::new(VrfOrch::new(VrfOrchConfig::new(0x1000)));
        register_vrf_orch(orch);

        // Null returns global VRF ID
        let id = unsafe { rust_vrf_orch_get_vrf_id(ptr::null()) };
        assert_eq!(id, 0x1000);

        unregister_vrf_orch();
    }

    #[test]
    fn test_global_vrf_id() {
        unregister_vrf_orch();

        let orch = Box::new(VrfOrch::new(VrfOrchConfig::new(0x5000)));
        register_vrf_orch(orch);

        assert_eq!(rust_vrf_orch_get_global_vrf_id(), 0x5000);

        unregister_vrf_orch();
    }

    #[test]
    fn test_ref_count_operations() {
        unregister_vrf_orch();

        let mut orch = VrfOrch::new(VrfOrchConfig::default());
        use super::super::types::VrfConfig;
        orch.add_vrf(&VrfConfig::new("Vrf1")).unwrap();
        register_vrf_orch(Box::new(orch));

        let vrf_name = CString::new("Vrf1").unwrap();

        // Initial ref count
        assert_eq!(unsafe { rust_vrf_orch_get_ref_count(vrf_name.as_ptr()) }, 0);

        // Increase
        assert_eq!(
            unsafe { rust_vrf_orch_increase_ref_count(vrf_name.as_ptr()) },
            1
        );
        assert_eq!(unsafe { rust_vrf_orch_get_ref_count(vrf_name.as_ptr()) }, 1);

        // Decrease
        assert_eq!(
            unsafe { rust_vrf_orch_decrease_ref_count(vrf_name.as_ptr()) },
            0
        );
        assert_eq!(unsafe { rust_vrf_orch_get_ref_count(vrf_name.as_ptr()) }, 0);

        unregister_vrf_orch();
    }

    #[test]
    fn test_l3_vni_operations() {
        unregister_vrf_orch();

        let orch = Box::new(VrfOrch::new(VrfOrchConfig::default()));
        register_vrf_orch(orch);

        // No L3 VNI configured
        assert!(!rust_vrf_orch_is_l3_vni(10000));
        assert_eq!(rust_vrf_orch_get_l3_vni_vlan(10000), -1);

        unregister_vrf_orch();
    }
}
