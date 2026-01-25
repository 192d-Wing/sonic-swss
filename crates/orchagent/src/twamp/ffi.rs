//! FFI exports for TwampOrch.

use super::orch::{TwampOrch, TwampOrchConfig};
use std::cell::RefCell;
use std::ffi::{c_char, CStr};

thread_local! {
    static TWAMP_ORCH: RefCell<Option<Box<TwampOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_twamp_orch() -> bool {
    TWAMP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(TwampOrch::new(TwampOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_twamp_orch() -> bool {
    TWAMP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

#[no_mangle]
pub extern "C" fn twamp_orch_session_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    TWAMP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.session_exists(name_str))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn twamp_orch_session_count() -> u32 {
    TWAMP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.session_count() as u32)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        assert!(register_twamp_orch());
        assert!(!register_twamp_orch());
        assert!(unregister_twamp_orch());
        assert!(!unregister_twamp_orch());
    }

    #[test]
    fn test_session_count() {
        register_twamp_orch();
        assert_eq!(twamp_orch_session_count(), 0);
        unregister_twamp_orch();
    }
}
