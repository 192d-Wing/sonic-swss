//! FFI exports for MacsecOrch.

use super::orch::{MacsecOrch, MacsecOrchConfig};
use std::cell::RefCell;

thread_local! {
    static MACSEC_ORCH: RefCell<Option<Box<MacsecOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_macsec_orch() -> bool {
    MACSEC_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(MacsecOrch::new(MacsecOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_macsec_orch() -> bool {
    MACSEC_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
