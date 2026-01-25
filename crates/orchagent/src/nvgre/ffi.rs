//! FFI exports for NvgreOrch.

use super::orch::{NvgreOrch, NvgreOrchConfig};
use std::cell::RefCell;
use std::ffi::{c_char, CStr};

thread_local! {
    static NVGRE_ORCH: RefCell<Option<Box<NvgreOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_nvgre_orch() -> bool {
    NVGRE_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(NvgreOrch::new(NvgreOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_nvgre_orch() -> bool {
    NVGRE_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

#[no_mangle]
pub extern "C" fn nvgre_orch_tunnel_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    NVGRE_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.tunnel_exists(name_str))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn nvgre_orch_tunnel_count() -> u32 {
    NVGRE_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.tunnel_count() as u32)
            .unwrap_or(0)
    })
}
