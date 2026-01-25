//! FFI exports for FgNhgOrch.

use super::orch::{FgNhgOrch, FgNhgOrchConfig};
use std::cell::RefCell;

thread_local! {
    static FG_NHG_ORCH: RefCell<Option<Box<FgNhgOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_fg_nhg_orch() -> bool {
    FG_NHG_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(FgNhgOrch::new(FgNhgOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_fg_nhg_orch() -> bool {
    FG_NHG_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
