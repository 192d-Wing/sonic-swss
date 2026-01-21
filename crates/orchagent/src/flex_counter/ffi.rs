//! FFI exports for FlexCounterOrch.
//!
//! These functions allow C++ code to interact with the Rust FlexCounterOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use super::group::FlexCounterGroup;
use super::orch::FlexCounterOrch;

// Thread-local storage for the FlexCounterOrch instance
thread_local! {
    static FLEX_COUNTER_ORCH: RefCell<Option<Box<FlexCounterOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust FlexCounterOrch instance for C++ access.
///
/// Called during orchagent startup to make the Rust FlexCounterOrch
/// available to C++ code.
pub fn register_flex_counter_orch(orch: Box<FlexCounterOrch>) {
    FLEX_COUNTER_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust FlexCounterOrch instance.
pub fn unregister_flex_counter_orch() {
    FLEX_COUNTER_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the FlexCounterOrch is registered.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_is_registered() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns true if port counters are enabled.
///
/// # Safety
///
/// Safe to call from any thread after registration.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_port_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.port_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if port buffer drop counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_port_buffer_drop_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.port_buffer_drop_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if queue counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_queue_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.queue_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if queue watermark counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_queue_watermark_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.queue_watermark_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if PG counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_pg_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.pg_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if PG watermark counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_pg_watermark_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.pg_watermark_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if hostif trap counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_hostif_trap_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.hostif_trap_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if route flow counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_route_flow_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.route_flow_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if WRED queue counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_wred_queue_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.wred_queue_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if WRED port counters are enabled.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_wred_port_counters_enabled() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.wred_port_counters_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if create_only_config_db_buffers is set.
#[no_mangle]
pub extern "C" fn rust_flex_counter_orch_is_create_only_config_db_buffers() -> bool {
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_create_only_config_db_buffers())
            .unwrap_or(false)
    })
}

/// Returns true if the specified counter group is enabled.
///
/// # Safety
///
/// - `group_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_flex_counter_orch_is_group_enabled(
    group_name: *const c_char,
) -> bool {
    if group_name.is_null() {
        return false;
    }

    let group_str = match CStr::from_ptr(group_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let group = match group_str.parse::<FlexCounterGroup>() {
        Ok(g) => g,
        Err(_) => return false,
    };

    // Map group to the appropriate state check
    FLEX_COUNTER_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| match group {
                FlexCounterGroup::Port | FlexCounterGroup::PortRates => {
                    orch.port_counters_enabled()
                }
                FlexCounterGroup::PortBufferDrop => orch.port_buffer_drop_counters_enabled(),
                FlexCounterGroup::Queue => orch.queue_counters_enabled(),
                FlexCounterGroup::QueueWatermark => orch.queue_watermark_counters_enabled(),
                FlexCounterGroup::PgDrop => orch.pg_counters_enabled(),
                FlexCounterGroup::PgWatermark => orch.pg_watermark_counters_enabled(),
                FlexCounterGroup::FlowCntTrap => orch.hostif_trap_counters_enabled(),
                FlexCounterGroup::FlowCntRoute => orch.route_flow_counters_enabled(),
                FlexCounterGroup::WredEcnQueue => orch.wred_queue_counters_enabled(),
                FlexCounterGroup::WredEcnPort => orch.wred_port_counters_enabled(),
                // Other groups don't have explicit state tracking
                _ => false,
            })
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flex_counter::orch::FlexCounterOrchConfig;

    #[test]
    fn test_register_unregister() {
        // Start clean
        unregister_flex_counter_orch();
        assert!(!rust_flex_counter_orch_is_registered());

        // Register
        let orch = Box::new(FlexCounterOrch::new(FlexCounterOrchConfig::default()));
        register_flex_counter_orch(orch);
        assert!(rust_flex_counter_orch_is_registered());

        // Check initial state
        assert!(!rust_flex_counter_orch_port_counters_enabled());
        assert!(!rust_flex_counter_orch_queue_counters_enabled());

        // Unregister
        unregister_flex_counter_orch();
        assert!(!rust_flex_counter_orch_is_registered());
    }

    #[test]
    fn test_is_group_enabled() {
        unregister_flex_counter_orch();

        let orch = Box::new(FlexCounterOrch::new(FlexCounterOrchConfig::default()));
        register_flex_counter_orch(orch);

        let port_cstr = std::ffi::CString::new("PORT").unwrap();
        let result = unsafe { rust_flex_counter_orch_is_group_enabled(port_cstr.as_ptr()) };
        assert!(!result); // Not enabled by default

        let invalid_cstr = std::ffi::CString::new("INVALID_GROUP").unwrap();
        let result = unsafe { rust_flex_counter_orch_is_group_enabled(invalid_cstr.as_ptr()) };
        assert!(!result); // Invalid group returns false

        unregister_flex_counter_orch();
    }

    #[test]
    fn test_null_pointer_safety() {
        let result = unsafe { rust_flex_counter_orch_is_group_enabled(std::ptr::null()) };
        assert!(!result);
    }
}
