//! FFI exports for FabricPortsOrch.

use std::cell::RefCell;
use super::orch::{FabricPortsOrch, FabricPortsOrchConfig};

thread_local! {
    static FABRIC_PORTS_ORCH: RefCell<Option<Box<FabricPortsOrch>>> = const { RefCell::new(None) };
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
