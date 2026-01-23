//! FFI exports for SwitchOrch.

use std::cell::RefCell;
use super::orch::{SwitchOrch, SwitchOrchCallbacks, SwitchOrchConfig, Result};
use super::types::{SwitchCapabilities, SwitchHashConfig, SwitchState};

/// FFI stub callbacks that do nothing (for C++ interop).
struct FfiSwitchCallbacks;

impl SwitchOrchCallbacks for FfiSwitchCallbacks {
    fn initialize_switch(&self, _capabilities: &SwitchCapabilities) -> Result<SwitchState> {
        Ok(SwitchState::default())
    }

    fn set_hash_algorithm(&self, _is_ecmp: bool, _config: &SwitchHashConfig) -> Result<()> {
        Ok(())
    }

    fn get_capabilities(&self) -> Result<SwitchCapabilities> {
        Ok(SwitchCapabilities::default())
    }

    fn set_switch_attribute(&self, _attr_name: &str, _attr_value: &str) -> Result<()> {
        Ok(())
    }

    fn get_switch_attribute(&self, _attr_name: &str) -> Result<String> {
        Ok(String::new())
    }

    fn on_switch_initialized(&self, _state: &SwitchState) {}
    fn on_hash_updated(&self, _is_ecmp: bool) {}
    fn on_warm_restart_begin(&self) {}
    fn on_warm_restart_end(&self, _success: bool) {}
}

thread_local! {
    static SWITCH_ORCH: RefCell<Option<Box<SwitchOrch<FfiSwitchCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_switch_orch() -> bool {
    SWITCH_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(SwitchOrch::new(SwitchOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_switch_orch() -> bool {
    SWITCH_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
