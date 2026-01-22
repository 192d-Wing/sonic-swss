//! FFI exports for VxlanOrch.

use std::cell::RefCell;
use super::orch::{VxlanOrch, VxlanOrchConfig};

thread_local! {
    static VXLAN_ORCH: RefCell<Option<Box<VxlanOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_vxlan_orch() -> bool {
    VXLAN_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(VxlanOrch::new(VxlanOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_vxlan_orch() -> bool {
    VXLAN_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
