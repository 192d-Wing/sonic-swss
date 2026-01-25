//! FFI exports for TunnelDecapOrch.

use super::orch::{TunnelDecapOrch, TunnelDecapOrchConfig};
use std::cell::RefCell;
use std::ffi::{c_char, CStr};

thread_local! {
    static TUNNEL_DECAP_ORCH: RefCell<Option<Box<TunnelDecapOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_tunnel_decap_orch() -> bool {
    TUNNEL_DECAP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(TunnelDecapOrch::new(
            TunnelDecapOrchConfig::default(),
        )));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_tunnel_decap_orch() -> bool {
    TUNNEL_DECAP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

#[no_mangle]
pub extern "C" fn tunnel_decap_orch_tunnel_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    TUNNEL_DECAP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.tunnel_exists(name_str))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn tunnel_decap_orch_tunnel_count() -> u32 {
    TUNNEL_DECAP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.tunnel_count() as u32)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        assert!(register_tunnel_decap_orch());
        assert!(!register_tunnel_decap_orch());
        assert!(unregister_tunnel_decap_orch());
        assert!(!unregister_tunnel_decap_orch());
    }

    #[test]
    fn test_tunnel_count() {
        register_tunnel_decap_orch();
        assert_eq!(tunnel_decap_orch_tunnel_count(), 0);
        unregister_tunnel_decap_orch();
    }

    #[test]
    fn test_null_safety() {
        register_tunnel_decap_orch();
        assert!(!tunnel_decap_orch_tunnel_exists(std::ptr::null()));
        unregister_tunnel_decap_orch();
    }
}
