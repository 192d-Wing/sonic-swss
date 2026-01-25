//! FFI exports for MuxOrch.

use super::orch::{MuxOrch, MuxOrchConfig};
use std::cell::RefCell;

thread_local! {
    static MUX_ORCH: RefCell<Option<Box<MuxOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_mux_orch() -> bool {
    MUX_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(MuxOrch::new(MuxOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_mux_orch() -> bool {
    MUX_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
