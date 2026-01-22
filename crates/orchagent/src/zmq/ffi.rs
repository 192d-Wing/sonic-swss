//! FFI exports for ZmqOrch.

use std::cell::RefCell;
use super::orch::{ZmqOrch, ZmqOrchConfig};

thread_local! {
    static ZMQ_ORCH: RefCell<Option<Box<ZmqOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_zmq_orch() -> bool {
    ZMQ_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(ZmqOrch::new(ZmqOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_zmq_orch() -> bool {
    ZMQ_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
