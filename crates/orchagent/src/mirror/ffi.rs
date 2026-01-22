//! FFI exports for MirrorOrch.

use std::cell::RefCell;
use super::orch::{MirrorOrch, MirrorOrchConfig};

thread_local! {
    static MIRROR_ORCH: RefCell<Option<Box<MirrorOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_mirror_orch() -> bool {
    MIRROR_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(MirrorOrch::new(MirrorOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_mirror_orch() -> bool {
    MIRROR_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
