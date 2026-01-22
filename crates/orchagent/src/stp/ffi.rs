//! FFI exports for StpOrch.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use super::orch::{StpOrch, StpOrchConfig};

thread_local! {
    static STP_ORCH: RefCell<Option<Box<StpOrch>>> = const { RefCell::new(None) };
}

/// Registers the STP orch instance.
#[no_mangle]
pub extern "C" fn register_stp_orch() -> bool {
    STP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(StpOrch::new(StpOrchConfig::default())));
        true
    })
}

/// Unregisters the STP orch instance.
#[no_mangle]
pub extern "C" fn unregister_stp_orch() -> bool {
    STP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

/// Initializes STP orch with default instance and max instances.
#[no_mangle]
pub extern "C" fn stp_orch_initialize(default_stp_id: u64, max_stp_instance: u16) -> bool {
    STP_ORCH.with(|orch| {
        if let Some(ref mut o) = *orch.borrow_mut() {
            o.initialize(default_stp_id, max_stp_instance);
            true
        } else {
            false
        }
    })
}

/// Gets STP instance OID.
#[no_mangle]
pub extern "C" fn stp_orch_get_instance_oid(instance: u16, oid: *mut u64) -> bool {
    if oid.is_null() {
        return false;
    }

    STP_ORCH.with(|orch| {
        if let Some(ref o) = *orch.borrow() {
            if let Some(inst_oid) = o.get_instance_oid(instance) {
                unsafe {
                    *oid = inst_oid;
                }
                return true;
            }
        }
        false
    })
}

/// Gets STP instance count.
#[no_mangle]
pub extern "C" fn stp_orch_instance_count() -> u32 {
    STP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.instance_count() as u32)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        unregister_stp_orch();

        assert!(register_stp_orch());
        assert!(!register_stp_orch()); // Already registered

        assert!(unregister_stp_orch());
        assert!(!unregister_stp_orch()); // Already unregistered
    }

    #[test]
    fn test_initialize() {
        unregister_stp_orch();
        register_stp_orch();

        assert!(stp_orch_initialize(0x100, 256));

        let mut oid = 0u64;
        assert!(stp_orch_get_instance_oid(0, &mut oid));
        assert_eq!(oid, 0x100); // Default instance

        unregister_stp_orch();
    }

    #[test]
    fn test_null_safety() {
        unregister_stp_orch();
        register_stp_orch();

        // Null OID pointer
        assert!(!stp_orch_get_instance_oid(0, std::ptr::null_mut()));

        unregister_stp_orch();
    }
}
