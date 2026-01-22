//! FFI exports for Srv6Orch.

use std::cell::RefCell;
use super::orch::{Srv6Orch, Srv6OrchConfig};

thread_local! {
    static SRV6_ORCH: RefCell<Option<Box<Srv6Orch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_srv6_orch() -> bool {
    SRV6_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(Srv6Orch::new(Srv6OrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_srv6_orch() -> bool {
    SRV6_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
