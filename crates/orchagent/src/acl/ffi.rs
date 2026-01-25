//! FFI exports for AclOrch.
//!
//! These functions allow C++ code to interact with the Rust AclOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use sonic_sai::types::RawSaiObjectId;

use super::orch::{AclOrch, AclOrchConfig};

// Thread-local storage for the AclOrch instance
thread_local! {
    static ACL_ORCH: RefCell<Option<Box<AclOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust AclOrch instance for C++ access.
pub fn register_acl_orch(orch: Box<AclOrch>) {
    ACL_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust AclOrch instance.
pub fn unregister_acl_orch() {
    ACL_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the AclOrch is registered.
#[no_mangle]
pub extern "C" fn rust_acl_orch_is_registered() -> bool {
    ACL_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns the number of ACL tables.
#[no_mangle]
pub extern "C" fn rust_acl_orch_table_count() -> usize {
    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.table_count())
            .unwrap_or(0)
    })
}

/// Returns the total number of ACL rules across all tables.
#[no_mangle]
pub extern "C" fn rust_acl_orch_total_rule_count() -> usize {
    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.total_rule_count())
            .unwrap_or(0)
    })
}

/// Checks if an ACL table exists.
///
/// # Safety
///
/// - `table_id` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_acl_orch_has_table(table_id: *const c_char) -> bool {
    if table_id.is_null() {
        return false;
    }

    let table_id_str = match CStr::from_ptr(table_id).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.has_table(table_id_str))
            .unwrap_or(false)
    })
}

/// Gets the SAI OID for an ACL table.
///
/// Returns 0 if the table doesn't exist or isn't created in SAI.
///
/// # Safety
///
/// - `table_id` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_acl_orch_get_table_oid(table_id: *const c_char) -> RawSaiObjectId {
    if table_id.is_null() {
        return 0;
    }

    let table_id_str = match CStr::from_ptr(table_id).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_table(table_id_str))
            .map(|table| table.sai_id())
            .unwrap_or(0)
    })
}

/// Gets the number of rules in an ACL table.
///
/// Returns 0 if the table doesn't exist.
///
/// # Safety
///
/// - `table_id` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_acl_orch_get_table_rule_count(table_id: *const c_char) -> usize {
    if table_id.is_null() {
        return 0;
    }

    let table_id_str = match CStr::from_ptr(table_id).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_table(table_id_str))
            .map(|table| table.rule_count())
            .unwrap_or(0)
    })
}

/// Checks if an ACL table type exists.
///
/// # Safety
///
/// - `type_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_acl_orch_has_table_type(type_name: *const c_char) -> bool {
    if type_name.is_null() {
        return false;
    }

    let type_name_str = match CStr::from_ptr(type_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.get_table_type(type_name_str).is_some())
            .unwrap_or(false)
    })
}

/// Returns true if the AclOrch is initialized.
#[no_mangle]
pub extern "C" fn rust_acl_orch_is_initialized() -> bool {
    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_initialized())
            .unwrap_or(false)
    })
}

/// Gets the minimum ACL priority.
#[no_mangle]
pub extern "C" fn rust_acl_orch_get_min_priority() -> u32 {
    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.config().min_priority)
            .unwrap_or(0)
    })
}

/// Gets the maximum ACL priority.
#[no_mangle]
pub extern "C" fn rust_acl_orch_get_max_priority() -> u32 {
    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.config().max_priority)
            .unwrap_or(999999)
    })
}

/// Gets the number of tables created (statistic).
#[no_mangle]
pub extern "C" fn rust_acl_orch_stats_tables_created() -> u64 {
    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().tables_created)
            .unwrap_or(0)
    })
}

/// Gets the number of rules created (statistic).
#[no_mangle]
pub extern "C" fn rust_acl_orch_stats_rules_created() -> u64 {
    ACL_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().rules_created)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_register_unregister() {
        // Start clean
        unregister_acl_orch();
        assert!(!rust_acl_orch_is_registered());

        // Register
        let orch = Box::new(AclOrch::new(AclOrchConfig::default()));
        register_acl_orch(orch);
        assert!(rust_acl_orch_is_registered());

        // Check initial state
        assert_eq!(rust_acl_orch_table_count(), 0);
        assert_eq!(rust_acl_orch_total_rule_count(), 0);

        // Unregister
        unregister_acl_orch();
        assert!(!rust_acl_orch_is_registered());
    }

    #[test]
    fn test_has_table_null_safety() {
        let result = unsafe { rust_acl_orch_has_table(ptr::null()) };
        assert!(!result);
    }

    #[test]
    fn test_has_table_type() {
        unregister_acl_orch();

        let orch = Box::new(AclOrch::new(AclOrchConfig::default()));
        register_acl_orch(orch);

        let l3_cstr = CString::new("L3").unwrap();
        assert!(unsafe { rust_acl_orch_has_table_type(l3_cstr.as_ptr()) });

        let invalid_cstr = CString::new("INVALID").unwrap();
        assert!(!unsafe { rust_acl_orch_has_table_type(invalid_cstr.as_ptr()) });

        unregister_acl_orch();
    }

    #[test]
    fn test_priority_config() {
        unregister_acl_orch();

        let orch = Box::new(AclOrch::new(AclOrchConfig::default()));
        register_acl_orch(orch);

        assert_eq!(rust_acl_orch_get_min_priority(), 0);
        assert_eq!(rust_acl_orch_get_max_priority(), 999999);

        unregister_acl_orch();
    }
}
