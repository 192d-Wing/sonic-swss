//! FFI exports for IntfsOrch.

use super::orch::{IntfsOrch, IntfsOrchConfig};
use std::cell::RefCell;

thread_local! {
    static INTFS_ORCH: RefCell<Option<Box<IntfsOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_intfs_orch() -> bool {
    INTFS_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(IntfsOrch::new(IntfsOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_intfs_orch() -> bool {
    INTFS_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
