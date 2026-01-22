//! FFI exports for DtelOrch.

use std::cell::RefCell;
use super::orch::{DtelOrch, DtelOrchConfig};

thread_local! {
    static DTEL_ORCH: RefCell<Option<Box<DtelOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_dtel_orch() -> bool {
    DTEL_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(DtelOrch::new(DtelOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_dtel_orch() -> bool {
    DTEL_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
