//! FFI exports for SwitchOrch.

use std::cell::RefCell;
use super::orch::{SwitchOrch, SwitchOrchConfig};

thread_local! {
    static SWITCH_ORCH: RefCell<Option<Box<SwitchOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_switch_orch() -> bool {
    SWITCH_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(SwitchOrch::new(SwitchOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_switch_orch() -> bool {
    SWITCH_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
