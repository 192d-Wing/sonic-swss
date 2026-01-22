//! FFI exports for CoppOrch.

use std::cell::RefCell;
use super::orch::{CoppOrch, CoppOrchConfig};

thread_local! {
    static COPP_ORCH: RefCell<Option<Box<CoppOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_copp_orch() -> bool {
    COPP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(CoppOrch::new(CoppOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_copp_orch() -> bool {
    COPP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
