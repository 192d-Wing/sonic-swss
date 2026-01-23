//! FFI exports for MirrorOrch.

use std::cell::RefCell;
use super::orch::{MirrorOrch, MirrorOrchCallbacks, MirrorOrchConfig, Result};
use super::types::{MirrorSessionConfig, MirrorSessionType, RawSaiObjectId};

/// FFI stub callbacks that do nothing (for C++ interop).
struct FfiMirrorCallbacks;

impl MirrorOrchCallbacks for FfiMirrorCallbacks {
    fn create_mirror_session(&self, _config: &MirrorSessionConfig) -> Result<RawSaiObjectId> {
        Ok(0)
    }

    fn remove_mirror_session(&self, _session_id: RawSaiObjectId) -> Result<()> {
        Ok(())
    }

    fn update_mirror_session(&self, _session_id: RawSaiObjectId, _config: &MirrorSessionConfig) -> Result<()> {
        Ok(())
    }

    fn get_mirror_sessions_by_type(&self, _session_type: MirrorSessionType) -> Result<Vec<RawSaiObjectId>> {
        Ok(vec![])
    }

    fn on_session_created(&self, _name: &str, _session_id: RawSaiObjectId) {}
    fn on_session_removed(&self, _name: &str) {}
}

thread_local! {
    static MIRROR_ORCH: RefCell<Option<Box<MirrorOrch<FfiMirrorCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_mirror_orch() -> bool {
    MIRROR_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(MirrorOrch::new(MirrorOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_mirror_orch() -> bool {
    MIRROR_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
