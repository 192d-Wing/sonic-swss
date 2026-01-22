//! FFI exports for PfcWdOrch.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};
use super::orch::{PfcWdOrch, PfcWdOrchConfig};

thread_local! {
    static PFCWD_ORCH: RefCell<Option<Box<PfcWdOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_pfcwd_orch() -> bool {
    PFCWD_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(PfcWdOrch::new(PfcWdOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_pfcwd_orch() -> bool {
    PFCWD_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

#[no_mangle]
pub extern "C" fn pfcwd_orch_queue_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PFCWD_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.queue_exists(name_str))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn pfcwd_orch_queue_count() -> u32 {
    PFCWD_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.queue_count() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn pfcwd_orch_handle_storm_detected(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PFCWD_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            o.handle_storm_detected(name_str);
            true
        } else {
            false
        }
    })
}

#[no_mangle]
pub extern "C" fn pfcwd_orch_handle_storm_restored(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    PFCWD_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            o.handle_storm_restored(name_str);
            true
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        assert!(register_pfcwd_orch());
        assert!(!register_pfcwd_orch());
        assert!(unregister_pfcwd_orch());
        assert!(!unregister_pfcwd_orch());
    }

    #[test]
    fn test_queue_count() {
        register_pfcwd_orch();
        assert_eq!(pfcwd_orch_queue_count(), 0);
        unregister_pfcwd_orch();
    }

    #[test]
    fn test_null_safety() {
        register_pfcwd_orch();
        assert!(!pfcwd_orch_queue_exists(std::ptr::null()));
        assert!(!pfcwd_orch_handle_storm_detected(std::ptr::null()));
        assert!(!pfcwd_orch_handle_storm_restored(std::ptr::null()));
        unregister_pfcwd_orch();
    }
}
