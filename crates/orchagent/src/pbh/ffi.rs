//! FFI exports for PbhOrch.

use std::cell::RefCell;
use super::orch::{PbhOrch, PbhOrchConfig};

thread_local! {
    static PBH_ORCH: RefCell<Option<Box<PbhOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_pbh_orch() -> bool {
    PBH_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(PbhOrch::new(PbhOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_pbh_orch() -> bool {
    PBH_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
