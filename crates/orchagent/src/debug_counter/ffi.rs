//! FFI exports for DebugCounterOrch.

use super::orch::{DebugCounterOrch, DebugCounterOrchConfig};
use std::cell::RefCell;
use std::ffi::{c_char, CStr};

thread_local! {
    static DEBUG_COUNTER_ORCH: RefCell<Option<Box<DebugCounterOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_debug_counter_orch() -> bool {
    DEBUG_COUNTER_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(DebugCounterOrch::new(
            DebugCounterOrchConfig::default(),
        )));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_debug_counter_orch() -> bool {
    DEBUG_COUNTER_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

#[no_mangle]
pub extern "C" fn debug_counter_orch_counter_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    DEBUG_COUNTER_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.counter_exists(name_str))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn debug_counter_orch_counter_count() -> u32 {
    DEBUG_COUNTER_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.counter_count() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn debug_counter_orch_get_stats_counters_created() -> u64 {
    DEBUG_COUNTER_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.stats().counters_created)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn debug_counter_orch_get_stats_drop_reasons_added() -> u64 {
    DEBUG_COUNTER_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.stats().drop_reasons_added)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn debug_counter_orch_get_free_counter_count() -> u32 {
    DEBUG_COUNTER_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.get_free_counters().len() as u32)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        assert!(register_debug_counter_orch());
        assert!(!register_debug_counter_orch()); // Already registered
        assert!(unregister_debug_counter_orch());
        assert!(!unregister_debug_counter_orch()); // Already unregistered
    }

    #[test]
    fn test_counter_count() {
        register_debug_counter_orch();
        assert_eq!(debug_counter_orch_counter_count(), 0);
        unregister_debug_counter_orch();
    }

    #[test]
    fn test_null_safety() {
        register_debug_counter_orch();
        assert!(!debug_counter_orch_counter_exists(std::ptr::null()));
        unregister_debug_counter_orch();
    }

    #[test]
    fn test_free_counter_count() {
        register_debug_counter_orch();
        assert_eq!(debug_counter_orch_get_free_counter_count(), 0);
        unregister_debug_counter_orch();
    }
}
