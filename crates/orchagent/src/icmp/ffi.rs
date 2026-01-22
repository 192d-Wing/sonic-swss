//! FFI exports for IcmpOrch.

use std::cell::RefCell;
use super::orch::{IcmpOrch, IcmpOrchConfig};

thread_local! {
    static ICMP_ORCH: RefCell<Option<Box<IcmpOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_icmp_orch() -> bool {
    ICMP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(IcmpOrch::new(IcmpOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_icmp_orch() -> bool {
    ICMP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
