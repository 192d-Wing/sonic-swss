//! FFI exports for NatOrch.

use std::cell::RefCell;
use super::orch::{NatOrch, NatOrchConfig};

thread_local! {
    static NAT_ORCH: RefCell<Option<Box<NatOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_nat_orch() -> bool {
    NAT_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(NatOrch::new(NatOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_nat_orch() -> bool {
    NAT_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
