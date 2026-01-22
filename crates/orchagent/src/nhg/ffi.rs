//! FFI exports for NhgOrch.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};
use super::orch::{NhgOrch, NhgOrchConfig};

thread_local! {
    static NHG_ORCH: RefCell<Option<Box<NhgOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_nhg_orch() -> bool {
    NHG_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(NhgOrch::new(NhgOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_nhg_orch() -> bool {
    NHG_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

#[no_mangle]
pub extern "C" fn nhg_orch_nhg_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    NHG_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.nhg_exists(name_str))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn nhg_orch_nhg_count() -> u32 {
    NHG_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.nhg_count() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn nhg_orch_nexthop_count() -> u32 {
    NHG_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.nexthop_count() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn nhg_orch_increment_ref(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    NHG_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .and_then(|o| o.increment_nhg_ref(name_str).ok())
            .is_some()
    })
}

#[no_mangle]
pub extern "C" fn nhg_orch_decrement_ref(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    NHG_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .and_then(|o| o.decrement_nhg_ref(name_str).ok())
            .is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        assert!(register_nhg_orch());
        assert!(!register_nhg_orch());
        assert!(unregister_nhg_orch());
        assert!(!unregister_nhg_orch());
    }

    #[test]
    fn test_nhg_count() {
        register_nhg_orch();
        assert_eq!(nhg_orch_nhg_count(), 0);
        assert_eq!(nhg_orch_nexthop_count(), 0);
        unregister_nhg_orch();
    }

    #[test]
    fn test_null_safety() {
        register_nhg_orch();
        assert!(!nhg_orch_nhg_exists(std::ptr::null()));
        assert!(!nhg_orch_increment_ref(std::ptr::null()));
        assert!(!nhg_orch_decrement_ref(std::ptr::null()));
        unregister_nhg_orch();
    }
}
