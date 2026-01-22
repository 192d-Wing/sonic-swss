//! FFI exports for MplsRouteOrch.

use std::cell::RefCell;
use super::orch::{MplsRouteOrch, MplsRouteOrchConfig};

thread_local! {
    static MPLSROUTE_ORCH: RefCell<Option<Box<MplsRouteOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_mplsroute_orch() -> bool {
    MPLSROUTE_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(MplsRouteOrch::new(MplsRouteOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_mplsroute_orch() -> bool {
    MPLSROUTE_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
