//! FFI exports for BfdOrch.
//!
//! These functions allow C++ code to interact with the Rust BfdOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use sonic_sai::types::RawSaiObjectId;

use super::orch::{BfdOrch, BfdOrchConfig};
use super::types::BfdSessionState;

// Thread-local storage for the BfdOrch instance
thread_local! {
    static BFD_ORCH: RefCell<Option<Box<BfdOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust BfdOrch instance for C++ access.
pub fn register_bfd_orch(orch: Box<BfdOrch>) {
    BFD_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust BfdOrch instance.
pub fn unregister_bfd_orch() {
    BFD_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the BfdOrch is registered.
#[no_mangle]
pub extern "C" fn rust_bfd_orch_is_registered() -> bool {
    BFD_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns true if the BfdOrch is initialized.
#[no_mangle]
pub extern "C" fn rust_bfd_orch_is_initialized() -> bool {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_initialized())
            .unwrap_or(false)
    })
}

/// Returns the number of active BFD sessions.
#[no_mangle]
pub extern "C" fn rust_bfd_orch_session_count() -> usize {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.session_count())
            .unwrap_or(0)
    })
}

/// Checks if a BFD session exists.
///
/// # Safety
///
/// - `key` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_bfd_orch_has_session(key: *const c_char) -> bool {
    if key.is_null() {
        return false;
    }

    let key_str = match CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.get_session(key_str).is_some())
            .unwrap_or(false)
    })
}

/// Gets the state of a BFD session.
///
/// # Safety
///
/// - `key` must be a valid null-terminated C string
///
/// Returns the SAI state value, or -1 if not found.
#[no_mangle]
pub unsafe extern "C" fn rust_bfd_orch_get_session_state(key: *const c_char) -> i32 {
    if key.is_null() {
        return -1;
    }

    let key_str = match CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_session(key_str))
            .map(|info| info.state.sai_value())
            .unwrap_or(-1)
    })
}

/// Gets the SAI OID of a BFD session.
///
/// # Safety
///
/// - `key` must be a valid null-terminated C string
///
/// Returns 0 if not found.
#[no_mangle]
pub unsafe extern "C" fn rust_bfd_orch_get_session_oid(key: *const c_char) -> RawSaiObjectId {
    if key.is_null() {
        return 0;
    }

    let key_str = match CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_session(key_str))
            .map(|info| info.sai_oid)
            .unwrap_or(0)
    })
}

/// Handles a BFD state change notification.
///
/// Returns true on success.
#[no_mangle]
pub extern "C" fn rust_bfd_orch_handle_state_change(sai_oid: RawSaiObjectId, state: i32) -> bool {
    let new_state = match BfdSessionState::from_sai_value(state) {
        Some(s) => s,
        None => return false,
    };

    BFD_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.handle_state_change(sai_oid, new_state).is_ok())
            .unwrap_or(false)
    })
}

/// Handles TSA state change.
///
/// Returns true on success.
#[no_mangle]
pub extern "C" fn rust_bfd_orch_handle_tsa_change(tsa_enabled: bool) -> bool {
    BFD_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.handle_tsa_state_change(tsa_enabled).is_ok())
            .unwrap_or(false)
    })
}

/// Gets the number of sessions created (statistic).
#[no_mangle]
pub extern "C" fn rust_bfd_orch_stats_sessions_created() -> u64 {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().sessions_created)
            .unwrap_or(0)
    })
}

/// Gets the number of sessions removed (statistic).
#[no_mangle]
pub extern "C" fn rust_bfd_orch_stats_sessions_removed() -> u64 {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().sessions_removed)
            .unwrap_or(0)
    })
}

/// Gets the number of state changes (statistic).
#[no_mangle]
pub extern "C" fn rust_bfd_orch_stats_state_changes() -> u64 {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().state_changes)
            .unwrap_or(0)
    })
}

/// Gets the number of creation retries (statistic).
#[no_mangle]
pub extern "C" fn rust_bfd_orch_stats_creation_retries() -> u64 {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().creation_retries)
            .unwrap_or(0)
    })
}

/// Gets the number of TSA shutdowns (statistic).
#[no_mangle]
pub extern "C" fn rust_bfd_orch_stats_tsa_shutdowns() -> u64 {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().tsa_shutdowns)
            .unwrap_or(0)
    })
}

/// Gets the number of TSA restores (statistic).
#[no_mangle]
pub extern "C" fn rust_bfd_orch_stats_tsa_restores() -> u64 {
    BFD_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().tsa_restores)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_register_unregister() {
        // Start clean
        unregister_bfd_orch();
        assert!(!rust_bfd_orch_is_registered());

        // Register
        let orch = Box::new(BfdOrch::new(BfdOrchConfig::default()));
        register_bfd_orch(orch);
        assert!(rust_bfd_orch_is_registered());

        // Check initial state
        assert!(!rust_bfd_orch_is_initialized());
        assert_eq!(rust_bfd_orch_session_count(), 0);

        // Unregister
        unregister_bfd_orch();
        assert!(!rust_bfd_orch_is_registered());
    }

    #[test]
    fn test_null_safety() {
        unregister_bfd_orch();
        let orch = Box::new(BfdOrch::new(BfdOrchConfig::default()));
        register_bfd_orch(orch);

        unsafe {
            assert!(!rust_bfd_orch_has_session(std::ptr::null()));
            assert_eq!(rust_bfd_orch_get_session_state(std::ptr::null()), -1);
            assert_eq!(rust_bfd_orch_get_session_oid(std::ptr::null()), 0);
        }

        unregister_bfd_orch();
    }

    #[test]
    fn test_session_not_found() {
        unregister_bfd_orch();
        let orch = Box::new(BfdOrch::new(BfdOrchConfig::default()));
        register_bfd_orch(orch);

        let key = CString::new("default::10.0.0.1").unwrap();

        unsafe {
            assert!(!rust_bfd_orch_has_session(key.as_ptr()));
            assert_eq!(rust_bfd_orch_get_session_state(key.as_ptr()), -1);
            assert_eq!(rust_bfd_orch_get_session_oid(key.as_ptr()), 0);
        }

        unregister_bfd_orch();
    }

    #[test]
    fn test_invalid_state_change() {
        unregister_bfd_orch();
        let orch = Box::new(BfdOrch::new(BfdOrchConfig::default()));
        register_bfd_orch(orch);

        // Invalid state value
        assert!(!rust_bfd_orch_handle_state_change(0x1234, 99));

        // Valid state but invalid OID
        assert!(!rust_bfd_orch_handle_state_change(0x1234, 3));

        unregister_bfd_orch();
    }

    #[test]
    fn test_statistics() {
        unregister_bfd_orch();
        let orch = Box::new(BfdOrch::new(BfdOrchConfig::default()));
        register_bfd_orch(orch);

        assert_eq!(rust_bfd_orch_stats_sessions_created(), 0);
        assert_eq!(rust_bfd_orch_stats_sessions_removed(), 0);
        assert_eq!(rust_bfd_orch_stats_state_changes(), 0);
        assert_eq!(rust_bfd_orch_stats_creation_retries(), 0);
        assert_eq!(rust_bfd_orch_stats_tsa_shutdowns(), 0);
        assert_eq!(rust_bfd_orch_stats_tsa_restores(), 0);

        unregister_bfd_orch();
    }
}
