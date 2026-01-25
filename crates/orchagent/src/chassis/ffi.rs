//! FFI exports for ChassisOrch.

use super::orch::{ChassisOrch, ChassisOrchCallbacks, ChassisOrchConfig, Result};
use super::types::{
    FabricPortKey, RawSaiObjectId, SystemPortConfig, SystemPortEntry, SystemPortKey,
};
use std::cell::RefCell;

/// FFI stub callbacks that do nothing (for C++ interop).
struct FfiChassisCallbacks;

impl ChassisOrchCallbacks for FfiChassisCallbacks {
    fn create_system_port(&self, _config: &SystemPortConfig) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn remove_system_port(&self, _oid: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn set_system_port_attribute(
        &self,
        _oid: RawSaiObjectId,
        _attr_name: &str,
        _attr_value: &str,
    ) -> Result<()> {
        Ok(())
    }

    fn create_fabric_port(&self, _port_id: u32) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn remove_fabric_port(&self, _oid: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn set_fabric_port_isolate(&self, _oid: RawSaiObjectId, _isolate: bool) -> Result<()> {
        Ok(())
    }

    fn write_system_port_state(&self, _key: &SystemPortKey, _state: &str) -> Result<()> {
        Ok(())
    }

    fn remove_system_port_state(&self, _key: &SystemPortKey) -> Result<()> {
        Ok(())
    }

    fn on_system_port_created(&self, _entry: &SystemPortEntry) {}
    fn on_system_port_removed(&self, _key: &SystemPortKey) {}
    fn on_fabric_port_isolate_changed(&self, _key: &FabricPortKey, _isolate: bool) {}
}

thread_local! {
    static CHASSIS_ORCH: RefCell<Option<Box<ChassisOrch<FfiChassisCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_chassis_orch() -> bool {
    CHASSIS_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(ChassisOrch::new(ChassisOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_chassis_orch() -> bool {
    CHASSIS_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
