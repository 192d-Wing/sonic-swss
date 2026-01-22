//! FFI exports for ChassisOrch.

use std::cell::RefCell;
use super::orch::{ChassisOrch, ChassisOrchConfig};

thread_local! {
    static CHASSIS_ORCH: RefCell<Option<Box<ChassisOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_chassis_orch() -> bool {
    CHASSIS_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(ChassisOrch::new(ChassisOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_chassis_orch() -> bool {
    CHASSIS_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
