//! FFI exports for IsolationGroupOrch.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};
use super::orch::{IsolationGroupOrch, IsolationGroupOrchConfig};

thread_local! {
    static ISOLATION_GROUP_ORCH: RefCell<Option<Box<IsolationGroupOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_isolation_group_orch() -> bool {
    ISOLATION_GROUP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(IsolationGroupOrch::new(IsolationGroupOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_isolation_group_orch() -> bool {
    ISOLATION_GROUP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}

#[no_mangle]
pub extern "C" fn isolation_group_orch_group_exists(name: *const c_char) -> bool {
    if name.is_null() {
        return false;
    }

    let name_str = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    ISOLATION_GROUP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.group_exists(name_str))
            .unwrap_or(false)
    })
}

#[no_mangle]
pub extern "C" fn isolation_group_orch_group_count() -> u32 {
    ISOLATION_GROUP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.group_count() as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn isolation_group_orch_get_stats_groups_created() -> u64 {
    ISOLATION_GROUP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.stats().groups_created)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn isolation_group_orch_get_stats_members_added() -> u64 {
    ISOLATION_GROUP_ORCH.with(|orch| {
        orch.borrow()
            .as_ref()
            .map(|o| o.stats().members_added)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        assert!(register_isolation_group_orch());
        assert!(!register_isolation_group_orch()); // Already registered
        assert!(unregister_isolation_group_orch());
        assert!(!unregister_isolation_group_orch()); // Already unregistered
    }

    #[test]
    fn test_group_count() {
        register_isolation_group_orch();
        assert_eq!(isolation_group_orch_group_count(), 0);
        unregister_isolation_group_orch();
    }

    #[test]
    fn test_null_safety() {
        register_isolation_group_orch();
        assert!(!isolation_group_orch_group_exists(std::ptr::null()));
        unregister_isolation_group_orch();
    }
}
