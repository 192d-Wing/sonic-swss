//! FFI exports for DtelOrch.

use std::cell::RefCell;
use sonic_sai::types::RawSaiObjectId;
use super::orch::{DtelOrch, DtelOrchCallbacks, DtelOrchConfig, Result};
use super::types::{DtelEventType, IntSessionConfig};

/// FFI stub callbacks that do nothing (for C++ interop).
struct FfiDtelCallbacks;

impl DtelOrchCallbacks for FfiDtelCallbacks {
    fn create_int_session(&self, _config: &IntSessionConfig) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn remove_int_session(&self, _session_oid: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn enable_event(&self, _event_type: DtelEventType) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn disable_event(&self, _event_oid: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn set_dtel_attribute(&self, _attr_name: &str, _attr_value: &str) -> Result<()> {
        Ok(())
    }

    fn write_state_db(&self, _session_id: &str, _state: &str) -> Result<()> {
        Ok(())
    }

    fn remove_state_db(&self, _session_id: &str) -> Result<()> {
        Ok(())
    }

    fn on_session_created(&self, _session_id: &str, _session_oid: RawSaiObjectId) {}
    fn on_session_removed(&self, _session_id: &str) {}
    fn on_event_state_changed(&self, _event_type: DtelEventType, _enabled: bool) {}
}

thread_local! {
    static DTEL_ORCH: RefCell<Option<Box<DtelOrch<FfiDtelCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_dtel_orch() -> bool {
    DTEL_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(DtelOrch::new(DtelOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_dtel_orch() -> bool {
    DTEL_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
