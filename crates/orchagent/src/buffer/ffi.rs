//! FFI exports for BufferOrch.

use std::cell::RefCell;
use super::orch::{BufferOrch, BufferOrchConfig};

thread_local! {
    static BUFFER_ORCH: RefCell<Option<Box<BufferOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_buffer_orch() -> bool {
    BUFFER_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(BufferOrch::new(BufferOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_buffer_orch() -> bool {
    BUFFER_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
