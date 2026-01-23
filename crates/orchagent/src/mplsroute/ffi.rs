//! FFI exports for MplsRouteOrch.

use std::cell::RefCell;
use std::sync::Arc;
use super::orch::{MplsRouteOrch, MplsRouteOrchConfig, MplsRouteOrchCallbacks, Result};
use super::types::{MplsRouteConfig, RawSaiObjectId};

/// Default FFI stub callbacks that do nothing
pub struct FfiMplsRouteCallbacks;

impl MplsRouteOrchCallbacks for FfiMplsRouteCallbacks {
    fn create_mpls_route(&self, _label: u32, _config: &MplsRouteConfig) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn remove_mpls_route(&self, _label: u32, _route_oid: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn update_mpls_route(&self, _label: u32, _route_oid: RawSaiObjectId, _config: &MplsRouteConfig) -> Result<()> {
        Ok(())
    }

    fn create_next_hop(&self, _ip_address: &str) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn remove_next_hop(&self, _nh_oid: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn on_route_created(&self, _label: u32, _route_oid: RawSaiObjectId) {}
    fn on_route_removed(&self, _label: u32) {}
}

thread_local! {
    static MPLSROUTE_ORCH: RefCell<Option<Box<MplsRouteOrch<FfiMplsRouteCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_mplsroute_orch() -> bool {
    MPLSROUTE_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        let callbacks = Arc::new(FfiMplsRouteCallbacks);
        *orch.borrow_mut() = Some(Box::new(
            MplsRouteOrch::new(MplsRouteOrchConfig::default())
                .with_callbacks(callbacks)
        ));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_mplsroute_orch() -> bool {
    MPLSROUTE_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
