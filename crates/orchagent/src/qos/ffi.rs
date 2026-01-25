//! FFI exports for QosOrch.

use super::orch::{QosOrch, QosOrchConfig};
use std::cell::RefCell;

thread_local! {
    static QOS_ORCH: RefCell<Option<Box<QosOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_qos_orch() -> bool {
    QOS_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(QosOrch::new(QosOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_qos_orch() -> bool {
    QOS_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
