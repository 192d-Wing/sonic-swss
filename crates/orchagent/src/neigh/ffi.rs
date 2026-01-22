//! FFI exports for NeighOrch.

use std::cell::RefCell;
use super::orch::{NeighOrch, NeighOrchConfig};

thread_local! {
    static NEIGH_ORCH: RefCell<Option<Box<NeighOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_neigh_orch() -> bool {
    NEIGH_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(NeighOrch::new(NeighOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_neigh_orch() -> bool {
    NEIGH_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
