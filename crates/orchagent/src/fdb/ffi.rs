//! FFI exports for FdbOrch.

use std::cell::RefCell;
use super::orch::{FdbOrch, FdbOrchConfig};

thread_local! {
    static FDB_ORCH: RefCell<Option<Box<FdbOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_fdb_orch() -> bool {
    FDB_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(FdbOrch::new(FdbOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_fdb_orch() -> bool {
    FDB_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
