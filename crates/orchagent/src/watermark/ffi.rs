//! FFI exports for WatermarkOrch.
//!
//! These functions allow C++ code to interact with the Rust WatermarkOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use sonic_sai::types::RawSaiObjectId;

use super::orch::{WatermarkOrch, WatermarkOrchConfig};
use super::types::{ClearRequest, QueueType, WatermarkGroup, WatermarkTable};

// Thread-local storage for the WatermarkOrch instance
thread_local! {
    static WATERMARK_ORCH: RefCell<Option<Box<WatermarkOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust WatermarkOrch instance for C++ access.
pub fn register_watermark_orch(orch: Box<WatermarkOrch>) {
    WATERMARK_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust WatermarkOrch instance.
pub fn unregister_watermark_orch() {
    WATERMARK_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the WatermarkOrch is registered.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_is_registered() -> bool {
    WATERMARK_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns true if the WatermarkOrch is initialized.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_is_initialized() -> bool {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_initialized())
            .unwrap_or(false)
    })
}

/// Returns true if any watermark collection is enabled.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_is_enabled() -> bool {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_enabled())
            .unwrap_or(false)
    })
}

/// Returns the telemetry interval in seconds.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_get_telemetry_interval() -> u64 {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.telemetry_interval().as_secs())
            .unwrap_or(0)
    })
}

/// Sets the telemetry interval in seconds.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_set_telemetry_interval(secs: u64) {
    WATERMARK_ORCH.with(|cell| {
        if let Some(orch) = cell.borrow_mut().as_mut() {
            orch.set_telemetry_interval_secs(secs);
        }
    })
}

/// Returns true if timer interval changed.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_timer_changed() -> bool {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.timer_changed())
            .unwrap_or(false)
    })
}

/// Clears the timer changed flag.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_clear_timer_changed() {
    WATERMARK_ORCH.with(|cell| {
        if let Some(orch) = cell.borrow_mut().as_mut() {
            orch.clear_timer_changed();
        }
    })
}

/// Handles flex counter status update.
///
/// Returns true if timer should be started.
///
/// # Safety
///
/// - `group` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_watermark_orch_handle_flex_counter_status(
    group: *const c_char,
    enabled: bool,
) -> bool {
    if group.is_null() {
        return false;
    }

    let group_str = match CStr::from_ptr(group).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let wm_group = match group_str.parse::<WatermarkGroup>() {
        Ok(g) => g,
        Err(_) => return false,
    };

    WATERMARK_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.handle_flex_counter_status(wm_group, enabled))
            .unwrap_or(false)
    })
}

/// Returns true if queue watermarks are enabled.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_queue_enabled() -> bool {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.status().queue_enabled())
            .unwrap_or(false)
    })
}

/// Returns true if PG watermarks are enabled.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_pg_enabled() -> bool {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.status().pg_enabled())
            .unwrap_or(false)
    })
}

/// Returns the raw status value.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_get_status() -> u8 {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.status().raw())
            .unwrap_or(0)
    })
}

/// Adds a PG ID.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_add_pg_id(id: RawSaiObjectId) {
    WATERMARK_ORCH.with(|cell| {
        if let Some(orch) = cell.borrow_mut().as_mut() {
            orch.add_pg_id(id);
        }
    })
}

/// Returns the number of PG IDs.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_pg_id_count() -> usize {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.pg_ids().len())
            .unwrap_or(0)
    })
}

/// Returns true if PG IDs are initialized.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_pg_ids_initialized() -> bool {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.pg_ids_initialized())
            .unwrap_or(false)
    })
}

/// Adds a queue ID.
///
/// # Safety
///
/// - `queue_type` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_watermark_orch_add_queue_id(
    queue_type: *const c_char,
    id: RawSaiObjectId,
) {
    if queue_type.is_null() {
        return;
    }

    let type_str = match CStr::from_ptr(queue_type).to_str() {
        Ok(s) => s,
        Err(_) => return,
    };

    let qt = match type_str.parse::<QueueType>() {
        Ok(t) => t,
        Err(_) => return,
    };

    WATERMARK_ORCH.with(|cell| {
        if let Some(orch) = cell.borrow_mut().as_mut() {
            orch.add_queue_id(qt, id);
        }
    })
}

/// Returns true if queue IDs are initialized.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_queue_ids_initialized() -> bool {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.queue_ids_initialized())
            .unwrap_or(false)
    })
}

/// Handles timer expiration.
#[no_mangle]
pub extern "C" fn rust_watermark_orch_handle_timer_expiration() {
    WATERMARK_ORCH.with(|cell| {
        if let Some(orch) = cell.borrow_mut().as_mut() {
            orch.handle_timer_expiration();
        }
    })
}

/// Gets the number of timer expirations (statistic).
#[no_mangle]
pub extern "C" fn rust_watermark_orch_stats_timer_expirations() -> u64 {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().timer_expirations)
            .unwrap_or(0)
    })
}

/// Gets the number of clears processed (statistic).
#[no_mangle]
pub extern "C" fn rust_watermark_orch_stats_clears_processed() -> u64 {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().clears_processed)
            .unwrap_or(0)
    })
}

/// Gets the number of config updates (statistic).
#[no_mangle]
pub extern "C" fn rust_watermark_orch_stats_config_updates() -> u64 {
    WATERMARK_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().config_updates)
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
        unregister_watermark_orch();
        assert!(!rust_watermark_orch_is_registered());

        // Register
        let orch = Box::new(WatermarkOrch::new(WatermarkOrchConfig::default()));
        register_watermark_orch(orch);
        assert!(rust_watermark_orch_is_registered());

        // Check initial state
        assert!(!rust_watermark_orch_is_enabled());
        assert_eq!(rust_watermark_orch_get_telemetry_interval(), 120);

        // Unregister
        unregister_watermark_orch();
        assert!(!rust_watermark_orch_is_registered());
    }

    #[test]
    fn test_telemetry_interval() {
        unregister_watermark_orch();
        let orch = Box::new(WatermarkOrch::new(WatermarkOrchConfig::default()));
        register_watermark_orch(orch);

        rust_watermark_orch_set_telemetry_interval(60);
        assert_eq!(rust_watermark_orch_get_telemetry_interval(), 60);
        assert!(rust_watermark_orch_timer_changed());

        rust_watermark_orch_clear_timer_changed();
        assert!(!rust_watermark_orch_timer_changed());

        unregister_watermark_orch();
    }

    #[test]
    fn test_flex_counter_status() {
        unregister_watermark_orch();
        let orch = Box::new(WatermarkOrch::new(WatermarkOrchConfig::default()));
        register_watermark_orch(orch);

        let group = CString::new("QUEUE_WATERMARK").unwrap();
        let start_timer = unsafe { rust_watermark_orch_handle_flex_counter_status(group.as_ptr(), true) };
        assert!(start_timer);
        assert!(rust_watermark_orch_is_enabled());
        assert!(rust_watermark_orch_queue_enabled());

        unregister_watermark_orch();
    }

    #[test]
    fn test_pg_ids() {
        unregister_watermark_orch();
        let orch = Box::new(WatermarkOrch::new(WatermarkOrchConfig::default()));
        register_watermark_orch(orch);

        assert!(!rust_watermark_orch_pg_ids_initialized());

        rust_watermark_orch_add_pg_id(1);
        rust_watermark_orch_add_pg_id(2);

        assert!(rust_watermark_orch_pg_ids_initialized());
        assert_eq!(rust_watermark_orch_pg_id_count(), 2);

        unregister_watermark_orch();
    }

    #[test]
    fn test_queue_ids() {
        unregister_watermark_orch();
        let orch = Box::new(WatermarkOrch::new(WatermarkOrchConfig::default()));
        register_watermark_orch(orch);

        assert!(!rust_watermark_orch_queue_ids_initialized());

        let unicast = CString::new("SAI_QUEUE_TYPE_UNICAST").unwrap();
        let multicast = CString::new("SAI_QUEUE_TYPE_MULTICAST").unwrap();

        unsafe {
            rust_watermark_orch_add_queue_id(unicast.as_ptr(), 1);
            rust_watermark_orch_add_queue_id(multicast.as_ptr(), 2);
        }

        assert!(rust_watermark_orch_queue_ids_initialized());

        unregister_watermark_orch();
    }
}
