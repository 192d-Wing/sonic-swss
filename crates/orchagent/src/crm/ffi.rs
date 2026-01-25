//! FFI exports for CrmOrch.
//!
//! These functions allow C++ code to interact with the Rust CrmOrch
//! during the migration period.

use std::cell::RefCell;
use std::ffi::{c_char, CStr};

use sonic_sai::types::RawSaiObjectId;

use super::orch::{CrmOrch, CrmOrchConfig};
use super::types::{AclBindPoint, AclStage, CrmResourceType, CrmThresholdType};

// Thread-local storage for the CrmOrch instance
thread_local! {
    static CRM_ORCH: RefCell<Option<Box<CrmOrch>>> = const { RefCell::new(None) };
}

/// Registers the Rust CrmOrch instance for C++ access.
pub fn register_crm_orch(orch: Box<CrmOrch>) {
    CRM_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust CrmOrch instance.
pub fn unregister_crm_orch() {
    CRM_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Returns true if the CrmOrch is registered.
#[no_mangle]
pub extern "C" fn rust_crm_orch_is_registered() -> bool {
    CRM_ORCH.with(|cell| cell.borrow().is_some())
}

/// Returns true if the CrmOrch is initialized.
#[no_mangle]
pub extern "C" fn rust_crm_orch_is_initialized() -> bool {
    CRM_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.is_initialized())
            .unwrap_or(false)
    })
}

/// Returns the polling interval in seconds.
#[no_mangle]
pub extern "C" fn rust_crm_orch_get_polling_interval() -> u64 {
    CRM_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.polling_interval().as_secs())
            .unwrap_or(0)
    })
}

/// Sets the polling interval in seconds.
#[no_mangle]
pub extern "C" fn rust_crm_orch_set_polling_interval(secs: u64) {
    CRM_ORCH.with(|cell| {
        if let Some(orch) = cell.borrow_mut().as_mut() {
            orch.set_polling_interval(std::time::Duration::from_secs(secs));
        }
    })
}

/// Increments the used counter for a global resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_inc_res_used(resource: *const c_char) -> bool {
    if resource.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.increment_used(resource_type).is_ok())
            .unwrap_or(false)
    })
}

/// Decrements the used counter for a global resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_dec_res_used(resource: *const c_char) -> bool {
    if resource.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.decrement_used(resource_type).is_ok())
            .unwrap_or(false)
    })
}

/// Increments the used counter for an ACL resource.
///
/// # Safety
///
/// - `stage` must be a valid null-terminated C string ("INGRESS" or "EGRESS")
/// - `bind_point` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_inc_acl_used(
    resource: *const c_char,
    stage: *const c_char,
    bind_point: *const c_char,
) -> bool {
    if resource.is_null() || stage.is_null() || bind_point.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let stage_str = match CStr::from_ptr(stage).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let bind_point_str = match CStr::from_ptr(bind_point).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };
    let acl_stage = match stage_str.parse::<AclStage>() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let acl_bind_point = match bind_point_str.parse::<AclBindPoint>() {
        Ok(b) => b,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| {
                orch.increment_acl_used(resource_type, acl_stage, acl_bind_point)
                    .is_ok()
            })
            .unwrap_or(false)
    })
}

/// Decrements the used counter for an ACL resource.
///
/// # Safety
///
/// - `resource`, `stage`, `bind_point` must be valid null-terminated C strings
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_dec_acl_used(
    resource: *const c_char,
    stage: *const c_char,
    bind_point: *const c_char,
    table_id: RawSaiObjectId,
) -> bool {
    if resource.is_null() || stage.is_null() || bind_point.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let stage_str = match CStr::from_ptr(stage).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let bind_point_str = match CStr::from_ptr(bind_point).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };
    let acl_stage = match stage_str.parse::<AclStage>() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let acl_bind_point = match bind_point_str.parse::<AclBindPoint>() {
        Ok(b) => b,
        Err(_) => return false,
    };

    let table_id_opt = if table_id == 0 { None } else { Some(table_id) };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| {
                orch.decrement_acl_used(resource_type, acl_stage, acl_bind_point, table_id_opt)
                    .is_ok()
            })
            .unwrap_or(false)
    })
}

/// Increments the used counter for a per-table ACL resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_inc_acl_table_used(
    resource: *const c_char,
    table_id: RawSaiObjectId,
) -> bool {
    if resource.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| {
                orch.increment_acl_table_used(resource_type, table_id)
                    .is_ok()
            })
            .unwrap_or(false)
    })
}

/// Decrements the used counter for a per-table ACL resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_dec_acl_table_used(
    resource: *const c_char,
    table_id: RawSaiObjectId,
) -> bool {
    if resource.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| {
                orch.decrement_acl_table_used(resource_type, table_id)
                    .is_ok()
            })
            .unwrap_or(false)
    })
}

/// Increments the used counter for an extension table.
///
/// # Safety
///
/// - `table_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_inc_ext_table_used(table_name: *const c_char) -> bool {
    if table_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(table_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.increment_ext_table_used(name_str).is_ok())
            .unwrap_or(false)
    })
}

/// Decrements the used counter for an extension table.
///
/// # Safety
///
/// - `table_name` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_dec_ext_table_used(table_name: *const c_char) -> bool {
    if table_name.is_null() {
        return false;
    }

    let name_str = match CStr::from_ptr(table_name).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.decrement_ext_table_used(name_str).is_ok())
            .unwrap_or(false)
    })
}

/// Increments the used counter for a DASH ACL resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_inc_dash_acl_used(
    resource: *const c_char,
    group_id: RawSaiObjectId,
) -> bool {
    if resource.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| {
                orch.increment_dash_acl_used(resource_type, group_id)
                    .is_ok()
            })
            .unwrap_or(false)
    })
}

/// Decrements the used counter for a DASH ACL resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_dec_dash_acl_used(
    resource: *const c_char,
    group_id: RawSaiObjectId,
) -> bool {
    if resource.is_null() {
        return false;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| {
                orch.decrement_dash_acl_used(resource_type, group_id)
                    .is_ok()
            })
            .unwrap_or(false)
    })
}

/// Handles a configuration field update.
///
/// # Safety
///
/// - `field` and `value` must be valid null-terminated C strings
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_handle_config(
    field: *const c_char,
    value: *const c_char,
) -> bool {
    if field.is_null() || value.is_null() {
        return false;
    }

    let field_str = match CStr::from_ptr(field).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let value_str = match CStr::from_ptr(value).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .map(|orch| orch.handle_config_field(field_str, value_str).is_ok())
            .unwrap_or(false)
    })
}

/// Handles timer expiration.
#[no_mangle]
pub extern "C" fn rust_crm_orch_handle_timer() {
    CRM_ORCH.with(|cell| {
        if let Some(orch) = cell.borrow_mut().as_mut() {
            orch.handle_timer_expiration();
        }
    })
}

/// Gets the used counter for a resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_get_used(resource: *const c_char) -> u32 {
    if resource.is_null() {
        return 0;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return 0,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_used(resource_type))
            .unwrap_or(0)
    })
}

/// Gets the available counter for a resource.
///
/// # Safety
///
/// - `resource` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_crm_orch_get_available(resource: *const c_char) -> u32 {
    if resource.is_null() {
        return 0;
    }

    let resource_str = match CStr::from_ptr(resource).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let resource_type = match resource_str.parse::<CrmResourceType>() {
        Ok(t) => t,
        Err(_) => return 0,
    };

    CRM_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_available(resource_type))
            .unwrap_or(0)
    })
}

/// Gets the number of timer expirations (statistic).
#[no_mangle]
pub extern "C" fn rust_crm_orch_stats_timer_expirations() -> u64 {
    CRM_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().timer_expirations)
            .unwrap_or(0)
    })
}

/// Gets the number of threshold events (statistic).
#[no_mangle]
pub extern "C" fn rust_crm_orch_stats_threshold_events() -> u64 {
    CRM_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().threshold_events)
            .unwrap_or(0)
    })
}

/// Gets the number of config updates (statistic).
#[no_mangle]
pub extern "C" fn rust_crm_orch_stats_config_updates() -> u64 {
    CRM_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.stats().config_updates)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_register_unregister() {
        // Start clean
        unregister_crm_orch();
        assert!(!rust_crm_orch_is_registered());

        // Register
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);
        assert!(rust_crm_orch_is_registered());

        // Check initial state
        assert!(!rust_crm_orch_is_initialized());
        assert_eq!(rust_crm_orch_get_polling_interval(), 300); // 5 minutes

        // Unregister
        unregister_crm_orch();
        assert!(!rust_crm_orch_is_registered());
    }

    #[test]
    fn test_polling_interval() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        rust_crm_orch_set_polling_interval(60);
        assert_eq!(rust_crm_orch_get_polling_interval(), 60);

        unregister_crm_orch();
    }

    #[test]
    fn test_inc_dec_res_used() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        let resource = CString::new("ipv4_route").unwrap();

        unsafe {
            assert!(rust_crm_orch_inc_res_used(resource.as_ptr()));
            assert_eq!(rust_crm_orch_get_used(resource.as_ptr()), 1);

            assert!(rust_crm_orch_inc_res_used(resource.as_ptr()));
            assert_eq!(rust_crm_orch_get_used(resource.as_ptr()), 2);

            assert!(rust_crm_orch_dec_res_used(resource.as_ptr()));
            assert_eq!(rust_crm_orch_get_used(resource.as_ptr()), 1);
        }

        unregister_crm_orch();
    }

    #[test]
    fn test_null_safety() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        unsafe {
            assert!(!rust_crm_orch_inc_res_used(std::ptr::null()));
            assert!(!rust_crm_orch_dec_res_used(std::ptr::null()));
            assert_eq!(rust_crm_orch_get_used(std::ptr::null()), 0);
        }

        unregister_crm_orch();
    }

    #[test]
    fn test_acl_operations() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        let resource = CString::new("acl_table").unwrap();
        let stage = CString::new("INGRESS").unwrap();
        let bind_point = CString::new("PORT").unwrap();

        unsafe {
            assert!(rust_crm_orch_inc_acl_used(
                resource.as_ptr(),
                stage.as_ptr(),
                bind_point.as_ptr()
            ));

            assert!(rust_crm_orch_dec_acl_used(
                resource.as_ptr(),
                stage.as_ptr(),
                bind_point.as_ptr(),
                0
            ));
        }

        unregister_crm_orch();
    }

    #[test]
    fn test_acl_table_operations() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        let resource = CString::new("acl_entry").unwrap();
        let table_id: RawSaiObjectId = 0x1234;

        unsafe {
            assert!(rust_crm_orch_inc_acl_table_used(
                resource.as_ptr(),
                table_id
            ));
            assert!(rust_crm_orch_dec_acl_table_used(
                resource.as_ptr(),
                table_id
            ));
        }

        unregister_crm_orch();
    }

    #[test]
    fn test_ext_table_operations() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        let table_name = CString::new("my_p4_table").unwrap();

        unsafe {
            assert!(rust_crm_orch_inc_ext_table_used(table_name.as_ptr()));
            assert!(rust_crm_orch_dec_ext_table_used(table_name.as_ptr()));
        }

        unregister_crm_orch();
    }

    #[test]
    fn test_dash_acl_operations() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        let resource = CString::new("dash_acl_group").unwrap();
        let group_id: RawSaiObjectId = 0xabcd;

        unsafe {
            assert!(rust_crm_orch_inc_dash_acl_used(resource.as_ptr(), group_id));
            assert!(rust_crm_orch_dec_dash_acl_used(resource.as_ptr(), group_id));
        }

        unregister_crm_orch();
    }

    #[test]
    fn test_config_handling() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        let field = CString::new("polling_interval").unwrap();
        let value = CString::new("60").unwrap();

        unsafe {
            assert!(rust_crm_orch_handle_config(field.as_ptr(), value.as_ptr()));
        }
        assert_eq!(rust_crm_orch_get_polling_interval(), 60);

        let field = CString::new("ipv4_route_threshold_type").unwrap();
        let value = CString::new("used").unwrap();

        unsafe {
            assert!(rust_crm_orch_handle_config(field.as_ptr(), value.as_ptr()));
        }

        unregister_crm_orch();
    }

    #[test]
    fn test_timer_handling() {
        unregister_crm_orch();
        let orch = Box::new(CrmOrch::new(CrmOrchConfig::default()));
        register_crm_orch(orch);

        rust_crm_orch_handle_timer();
        assert_eq!(rust_crm_orch_stats_timer_expirations(), 1);

        rust_crm_orch_handle_timer();
        assert_eq!(rust_crm_orch_stats_timer_expirations(), 2);

        unregister_crm_orch();
    }
}
