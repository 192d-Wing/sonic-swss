//! FFI exports for SflowOrch.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use super::orch::{SflowOrch, SflowOrchConfig};
use super::types::SflowConfig;

thread_local! {
    static SFLOW_ORCH: RefCell<Option<Box<SflowOrch>>> = const { RefCell::new(None) };
}

/// Registers the sflow orch instance.
#[no_mangle]
pub extern "C" fn register_sflow_orch() -> bool {
    SFLOW_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(SflowOrch::new(SflowOrchConfig::default())));
        true
    })
}

/// Unregisters the sflow orch instance.
#[no_mangle]
pub extern "C" fn unregister_sflow_orch() -> bool {
    SFLOW_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

/// Sets the global sflow enable/disable status.
#[no_mangle]
pub extern "C" fn sflow_orch_set_enabled(enabled: bool) -> bool {
    SFLOW_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            o.set_enabled(enabled);
            true
        } else {
            false
        }
    })
}

/// Gets the global sflow enable/disable status.
#[no_mangle]
pub extern "C" fn sflow_orch_is_enabled() -> bool {
    SFLOW_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.is_enabled())
            .unwrap_or(false)
    })
}

/// Configures sflow on a port.
#[no_mangle]
pub extern "C" fn sflow_orch_configure_port(
    alias: *const c_char,
    admin_state: bool,
    rate: u32,
    direction: *const c_char,
) -> bool {
    if alias.is_null() || direction.is_null() {
        return false;
    }

    let alias_str = match unsafe { CStr::from_ptr(alias) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let direction_str = match unsafe { CStr::from_ptr(direction) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    SFLOW_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            let mut config = SflowConfig::new();
            config.admin_state = admin_state;

            if let Err(e) = config.parse_field("sample_rate", &rate.to_string()) {
                eprintln!("Failed to parse sample_rate: {}", e);
                return false;
            }

            if let Err(e) = config.parse_field("sample_direction", direction_str) {
                eprintln!("Failed to parse sample_direction: {}", e);
                return false;
            }

            match o.configure_port(alias_str, config) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("Failed to configure sflow port {}: {}", alias_str, e);
                    false
                }
            }
        } else {
            false
        }
    })
}

/// Removes sflow configuration from a port.
#[no_mangle]
pub extern "C" fn sflow_orch_remove_port(alias: *const c_char) -> bool {
    if alias.is_null() {
        return false;
    }

    let alias_str = match unsafe { CStr::from_ptr(alias) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    SFLOW_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            match o.remove_port(alias_str) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("Failed to remove sflow port {}: {}", alias_str, e);
                    false
                }
            }
        } else {
            false
        }
    })
}

/// Gets the number of configured ports.
#[no_mangle]
pub extern "C" fn sflow_orch_port_count() -> u32 {
    SFLOW_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.port_count() as u32)
            .unwrap_or(0)
    })
}

/// Gets the number of active sessions.
#[no_mangle]
pub extern "C" fn sflow_orch_session_count() -> u32 {
    SFLOW_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.session_count() as u32)
            .unwrap_or(0)
    })
}

/// Gets sflow statistics.
#[no_mangle]
pub extern "C" fn sflow_orch_get_stats(
    sessions_created: *mut u64,
    sessions_destroyed: *mut u64,
    ports_configured: *mut u64,
    ports_unconfigured: *mut u64,
) -> bool {
    if sessions_created.is_null()
        || sessions_destroyed.is_null()
        || ports_configured.is_null()
        || ports_unconfigured.is_null()
    {
        return false;
    }

    SFLOW_ORCH.with(|orch| {
        if let Some(ref o) = *orch.borrow() {
            let stats = o.stats();
            unsafe {
                *sessions_created = stats.sessions_created;
                *sessions_destroyed = stats.sessions_destroyed;
                *ports_configured = stats.ports_configured;
                *ports_unconfigured = stats.ports_unconfigured;
            }
            true
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_register_unregister() {
        // Clean up any existing instance
        unregister_sflow_orch();

        assert!(register_sflow_orch());
        assert!(!register_sflow_orch()); // Already registered

        assert!(unregister_sflow_orch());
        assert!(!unregister_sflow_orch()); // Already unregistered
    }

    #[test]
    fn test_set_enabled() {
        unregister_sflow_orch();
        register_sflow_orch();

        assert!(sflow_orch_set_enabled(true));
        assert!(sflow_orch_is_enabled());

        assert!(sflow_orch_set_enabled(false));
        assert!(!sflow_orch_is_enabled());

        unregister_sflow_orch();
    }

    // Note: test_configure_port would require setting up callbacks
    // which isn't possible through the FFI layer. This functionality
    // is tested in the orch module tests.

    #[test]
    fn test_null_safety() {
        unregister_sflow_orch();
        register_sflow_orch();

        let alias = CString::new("Ethernet0").unwrap();
        let direction = CString::new("rx").unwrap();

        // Null alias
        assert!(!sflow_orch_configure_port(
            std::ptr::null(),
            true,
            4096,
            direction.as_ptr()
        ));

        // Null direction
        assert!(!sflow_orch_configure_port(
            alias.as_ptr(),
            true,
            4096,
            std::ptr::null()
        ));

        // Null remove
        assert!(!sflow_orch_remove_port(std::ptr::null()));

        unregister_sflow_orch();
    }

    // Note: test_get_stats would require configuring ports which needs callbacks.
    // Stats functionality is tested in the orch module tests.

    #[test]
    fn test_stats_null_safety() {
        unregister_sflow_orch();
        register_sflow_orch();

        let mut val = 0u64;

        // Null pointers should return false
        assert!(!sflow_orch_get_stats(
            std::ptr::null_mut(),
            &mut val,
            &mut val,
            &mut val
        ));

        unregister_sflow_orch();
    }
}
