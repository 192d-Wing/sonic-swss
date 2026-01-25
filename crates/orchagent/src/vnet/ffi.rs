//! FFI exports for VnetOrch.

use super::orch::{VnetOrch, VnetOrchConfig};
use std::cell::RefCell;

thread_local! {
    static VNET_ORCH: RefCell<Option<Box<VnetOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_vnet_orch() -> bool {
    VNET_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(VnetOrch::new(VnetOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_vnet_orch() -> bool {
    VNET_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
