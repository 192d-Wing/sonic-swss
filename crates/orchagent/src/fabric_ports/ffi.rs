//! FFI exports for FabricPortsOrch.

use std::cell::RefCell;
use sonic_sai::types::RawSaiObjectId;
use super::orch::{FabricPortsOrch, FabricPortsOrchCallbacks, FabricPortsOrchConfig, Result};
use super::types::{FabricPortState, IsolationState, LinkStatus};

/// FFI stub callbacks that do nothing (for C++ interop).
struct FfiFabricPortsCallbacks;

impl FabricPortsOrchCallbacks for FfiFabricPortsCallbacks {
    fn get_fabric_port_oid(&self, lane: u32) -> Result<RawSaiObjectId> {
        Ok(lane as u64)
    }

    fn get_link_status(&self, _oid: RawSaiObjectId) -> Result<LinkStatus> {
        Ok(LinkStatus::Down)
    }

    fn get_error_counters(&self, _oid: RawSaiObjectId) -> Result<u64> {
        Ok(0)
    }

    fn set_isolation(&self, _oid: RawSaiObjectId, _isolate: bool) -> Result<()> {
        Ok(())
    }

    fn write_state_db(&self, _lane: u32, _state: &FabricPortState) -> Result<()> {
        Ok(())
    }

    fn remove_state_db(&self, _lane: u32) -> Result<()> {
        Ok(())
    }

    fn on_link_status_changed(
        &self,
        _lane: u32,
        _old_status: LinkStatus,
        _new_status: LinkStatus,
    ) {
    }
    fn on_port_isolated(&self, _lane: u32, _reason: IsolationState) {}
    fn on_port_recovered(&self, _lane: u32) {}
}

thread_local! {
    static FABRIC_PORTS_ORCH: RefCell<Option<Box<FabricPortsOrch<FfiFabricPortsCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_fabric_ports_orch() -> bool {
    FABRIC_PORTS_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(FabricPortsOrch::new(FabricPortsOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_fabric_ports_orch() -> bool {
    FABRIC_PORTS_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
