//! FFI exports for CoppOrch.

use std::cell::RefCell;
use std::sync::Arc;
use super::orch::{CoppOrch, CoppOrchConfig, CoppOrchCallbacks, Result};
use super::types::{CoppTrapKey, CoppTrapConfig, RawSaiObjectId};

/// Stub callbacks that do nothing - used for FFI initialization
struct StubCoppCallbacks;

impl CoppOrchCallbacks for StubCoppCallbacks {
    fn create_trap(&self, _key: &CoppTrapKey, _config: &CoppTrapConfig) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn remove_trap(&self, _trap_id: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn update_trap_rate(&self, _trap_id: RawSaiObjectId, _cir: u64, _cbs: u64) -> Result<()> {
        Ok(())
    }

    fn get_trap_stats(&self, _trap_id: RawSaiObjectId) -> Result<(u64, u64)> {
        Ok((0, 0))
    }

    fn on_trap_created(&self, _key: &CoppTrapKey, _trap_id: RawSaiObjectId) {}
    fn on_trap_removed(&self, _key: &CoppTrapKey) {}
}

thread_local! {
    static COPP_ORCH: RefCell<Option<Box<CoppOrch<StubCoppCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_copp_orch() -> bool {
    COPP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        let copp = CoppOrch::new(CoppOrchConfig::default())
            .with_callbacks(Arc::new(StubCoppCallbacks));
        *orch.borrow_mut() = Some(Box::new(copp));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_copp_orch() -> bool {
    COPP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
