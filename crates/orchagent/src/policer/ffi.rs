//! FFI exports for PolicerOrch.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use super::orch::{PolicerOrch, PolicerOrchConfig};

thread_local! {
    static POLICER_ORCH: RefCell<Option<Box<PolicerOrch>>> = const { RefCell::new(None) };
}

/// Registers the policer orch instance.
#[no_mangle]
pub extern "C" fn register_policer_orch() -> bool {
    POLICER_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(PolicerOrch::new(PolicerOrchConfig::default())));
        true
    })
}

/// Unregisters the policer orch instance.
#[no_mangle]
pub extern "C" fn unregister_policer_orch() -> bool {
    POLICER_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

/// Checks if a policer exists.
#[no_mangle]
pub extern "C" fn policer_orch_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    POLICER_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.policer_exists(name_str))
            .unwrap_or(false)
    })
}

/// Gets the SAI OID for a policer.
#[no_mangle]
pub extern "C" fn policer_orch_get_oid(name: *const c_char, oid: *mut u64) -> bool {
    if name.is_null() || oid.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    POLICER_ORCH.with(|orch| {
        if let Some(ref o) = *orch.borrow() {
            if let Some(policer_oid) = o.get_policer_oid(name_str) {
                unsafe {
                    *oid = policer_oid;
                }
                return true;
            }
        }
        false
    })
}

/// Increments the reference count for a policer.
#[no_mangle]
pub extern "C" fn policer_orch_increase_ref_count(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    POLICER_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            match o.increase_ref_count(name_str) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!("Failed to increase ref count for {}: {}", name_str, e);
                    false
                }
            }
        } else {
            false
        }
    })
}

/// Decrements the reference count for a policer.
#[no_mangle]
pub extern "C" fn policer_orch_decrease_ref_count(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    POLICER_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            match o.decrease_ref_count(name_str) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!("Failed to decrease ref count for {}: {}", name_str, e);
                    false
                }
            }
        } else {
            false
        }
    })
}

/// Gets the number of policers.
#[no_mangle]
pub extern "C" fn policer_orch_policer_count() -> u32 {
    POLICER_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.policer_count() as u32)
            .unwrap_or(0)
    })
}

/// Gets policer statistics.
#[no_mangle]
pub extern "C" fn policer_orch_get_stats(
    policers_created: *mut u64,
    policers_removed: *mut u64,
    policers_updated: *mut u64,
    storm_control_applied: *mut u64,
) -> bool {
    if policers_created.is_null()
        || policers_removed.is_null()
        || policers_updated.is_null()
        || storm_control_applied.is_null()
    {
        return false;
    }

    POLICER_ORCH.with(|orch| {
        if let Some(ref o) = *orch.borrow() {
            let stats = o.stats();
            unsafe {
                *policers_created = stats.policers_created;
                *policers_removed = stats.policers_removed;
                *policers_updated = stats.policers_updated;
                *storm_control_applied = stats.storm_control_applied;
            }
            true
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_register_unregister() {
        // Clean up any existing instance
        unregister_policer_orch();

        assert!(register_policer_orch());
        assert!(!register_policer_orch()); // Already registered

        assert!(unregister_policer_orch());
        assert!(!unregister_policer_orch()); // Already unregistered
    }

    #[test]
    fn test_null_safety() {
        unregister_policer_orch();
        register_policer_orch();

        // Null name
        assert!(!policer_orch_exists(std::ptr::null()));

        // Null oid pointer
        let name = CString::new("test").unwrap();
        assert!(!policer_orch_get_oid(name.as_ptr(), std::ptr::null_mut()));

        // Null ref count
        assert!(!policer_orch_increase_ref_count(std::ptr::null()));
        assert!(!policer_orch_decrease_ref_count(std::ptr::null()));

        unregister_policer_orch();
    }

    #[test]
    fn test_stats_null_safety() {
        unregister_policer_orch();
        register_policer_orch();

        let mut val = 0u64;

        // Null pointers should return false
        assert!(!policer_orch_get_stats(
            std::ptr::null_mut(),
            &mut val,
            &mut val,
            &mut val
        ));

        unregister_policer_orch();
    }

    #[test]
    fn test_policer_count() {
        unregister_policer_orch();
        register_policer_orch();

        assert_eq!(policer_orch_policer_count(), 0);

        unregister_policer_orch();
    }
}
