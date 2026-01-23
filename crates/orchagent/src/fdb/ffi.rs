//! FFI exports for FdbOrch.

use std::cell::RefCell;
use std::sync::Arc;
use super::orch::{FdbOrch, FdbOrchCallbacks, FdbOrchConfig, Result};
use super::types::{FdbEntry, FdbKey};

/// Stub FFI callbacks for FdbOrch.
/// These are placeholder implementations used by FFI. In production,
/// these would be replaced with actual SAI integration callbacks.
pub struct FdbOrchFfiCallbacks;

impl FdbOrchCallbacks for FdbOrchFfiCallbacks {
    fn add_fdb_entry(&self, _entry: &FdbEntry) -> Result<()> {
        // FFI stub: would call SAI in production
        Ok(())
    }

    fn remove_fdb_entry(&self, _key: &FdbKey) -> Result<()> {
        // FFI stub: would call SAI in production
        Ok(())
    }

    fn update_fdb_entry(&self, _key: &FdbKey, _entry: &FdbEntry) -> Result<()> {
        // FFI stub: would call SAI in production
        Ok(())
    }

    fn get_fdb_entry(&self, _key: &FdbKey) -> Result<Option<FdbEntry>> {
        // FFI stub: would call SAI in production
        Ok(None)
    }

    fn flush_entries_by_port(&self, _port: Option<&str>) -> Result<u32> {
        // FFI stub: would call SAI in production
        Ok(0)
    }

    fn flush_entries_by_vlan(&self, _vlan: Option<u16>) -> Result<u32> {
        // FFI stub: would call SAI in production
        Ok(0)
    }

    fn on_fdb_entry_added(&self, _entry: &FdbEntry) {
        // FFI stub: notification callback
    }

    fn on_fdb_entry_removed(&self, _key: &FdbKey) {
        // FFI stub: notification callback
    }

    fn on_fdb_flush(&self, _port: Option<&str>, _vlan: Option<u16>, _count: u32) {
        // FFI stub: notification callback
    }
}

thread_local! {
    static FDB_ORCH: RefCell<Option<Box<FdbOrch<FdbOrchFfiCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_fdb_orch() -> bool {
    FDB_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        let fdb_orch = FdbOrch::new(FdbOrchConfig::default())
            .with_callbacks(Arc::new(FdbOrchFfiCallbacks));
        *orch.borrow_mut() = Some(Box::new(fdb_orch));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_fdb_orch() -> bool {
    FDB_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
