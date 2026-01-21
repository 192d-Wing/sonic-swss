//! Rust functions exported for C++ to call.
//!
//! These functions allow C++ code to call into Rust Orch modules
//! as they are migrated. Each function uses `extern "C"` ABI and
//! follows C naming conventions.

use std::ffi::{c_char, CStr};
use std::cell::RefCell;

// =============================================================================
// Thread-local storage for Rust Orch instances
// =============================================================================

// These thread-local variables hold references to Rust Orch instances
// that C++ code can access via the exported functions.
//
// Note: In the actual implementation, these would be initialized by
// the orchdaemon during startup.

thread_local! {
    /// Thread-local reference to Rust PortsOrch (when migrated)
    static RUST_PORTS_ORCH: RefCell<Option<Box<dyn RustPortsOrchTrait>>> = RefCell::new(None);

    /// Thread-local reference to Rust RouteOrch (when migrated)
    static RUST_ROUTE_ORCH: RefCell<Option<Box<dyn RustRouteOrchTrait>>> = RefCell::new(None);
}

// =============================================================================
// Trait definitions for Rust Orch modules
// =============================================================================

/// Trait defining the interface that Rust PortsOrch must implement
/// for C++ interoperability.
pub trait RustPortsOrchTrait: Send {
    /// Returns true if all ports are ready.
    fn all_ports_ready(&self) -> bool;

    /// Gets the port OID for a given alias.
    fn get_port_oid(&self, alias: &str) -> Option<u64>;

    /// Gets the port speed for a given alias.
    fn get_port_speed(&self, alias: &str) -> Option<u32>;
}

/// Trait defining the interface that Rust RouteOrch must implement
/// for C++ interoperability.
pub trait RustRouteOrchTrait: Send {
    /// Gets the next-hop group ID for a route.
    fn get_nhg_id(&self, prefix: &str) -> Option<u64>;

    /// Returns true if a route exists.
    fn has_route(&self, prefix: &str) -> bool;
}

// =============================================================================
// Registration functions (called from Rust during startup)
// =============================================================================

/// Registers the Rust PortsOrch instance for C++ access.
pub fn register_rust_ports_orch(orch: Box<dyn RustPortsOrchTrait>) {
    RUST_PORTS_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust PortsOrch instance.
pub fn unregister_rust_ports_orch() {
    RUST_PORTS_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Registers the Rust RouteOrch instance for C++ access.
pub fn register_rust_route_orch(orch: Box<dyn RustRouteOrchTrait>) {
    RUST_ROUTE_ORCH.with(|cell| {
        *cell.borrow_mut() = Some(orch);
    });
}

/// Unregisters the Rust RouteOrch instance.
pub fn unregister_rust_route_orch() {
    RUST_ROUTE_ORCH.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

// =============================================================================
// Exported C functions (called from C++)
// =============================================================================

/// Returns true if all ports are ready (Rust PortsOrch).
///
/// # Safety
///
/// This function is safe to call from any thread, but should only
/// be called after the Rust PortsOrch has been registered.
#[no_mangle]
pub extern "C" fn rust_ports_orch_all_ports_ready() -> bool {
    RUST_PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.all_ports_ready())
            .unwrap_or(false)
    })
}

/// Gets the port OID for a given alias (Rust PortsOrch).
///
/// Returns 0 if the port is not found or PortsOrch is not registered.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_port_oid(alias: *const c_char) -> u64 {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    RUST_PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port_oid(alias_str))
            .unwrap_or(0)
    })
}

/// Gets the port speed for a given alias (Rust PortsOrch).
///
/// Returns 0 if the port is not found or PortsOrch is not registered.
///
/// # Safety
///
/// - `alias` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_ports_orch_get_port_speed(alias: *const c_char) -> u32 {
    if alias.is_null() {
        return 0;
    }

    let alias_str = match CStr::from_ptr(alias).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    RUST_PORTS_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_port_speed(alias_str))
            .unwrap_or(0)
    })
}

/// Gets the next-hop group ID for a route (Rust RouteOrch).
///
/// Returns 0 if the route is not found or RouteOrch is not registered.
///
/// # Safety
///
/// - `prefix` must be a valid null-terminated C string (e.g., "10.0.0.0/24")
#[no_mangle]
pub unsafe extern "C" fn rust_route_orch_get_nhg_id(prefix: *const c_char) -> u64 {
    if prefix.is_null() {
        return 0;
    }

    let prefix_str = match CStr::from_ptr(prefix).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    RUST_ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|orch| orch.get_nhg_id(prefix_str))
            .unwrap_or(0)
    })
}

/// Returns true if a route exists (Rust RouteOrch).
///
/// # Safety
///
/// - `prefix` must be a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn rust_route_orch_has_route(prefix: *const c_char) -> bool {
    if prefix.is_null() {
        return false;
    }

    let prefix_str = match CStr::from_ptr(prefix).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };

    RUST_ROUTE_ORCH.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|orch| orch.has_route(prefix_str))
            .unwrap_or(false)
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPortsOrch {
        ready: bool,
    }

    impl RustPortsOrchTrait for MockPortsOrch {
        fn all_ports_ready(&self) -> bool {
            self.ready
        }

        fn get_port_oid(&self, alias: &str) -> Option<u64> {
            if alias == "Ethernet0" {
                Some(0x1000000000001)
            } else {
                None
            }
        }

        fn get_port_speed(&self, alias: &str) -> Option<u32> {
            if alias == "Ethernet0" {
                Some(100000)
            } else {
                None
            }
        }
    }

    #[test]
    fn test_rust_ports_orch_not_registered() {
        // Ensure it's not registered
        unregister_rust_ports_orch();

        assert!(!rust_ports_orch_all_ports_ready());
    }

    #[test]
    fn test_rust_ports_orch_registered() {
        let orch = Box::new(MockPortsOrch { ready: true });
        register_rust_ports_orch(orch);

        assert!(rust_ports_orch_all_ports_ready());

        // Clean up
        unregister_rust_ports_orch();
    }

    #[test]
    fn test_get_port_oid() {
        let orch = Box::new(MockPortsOrch { ready: true });
        register_rust_ports_orch(orch);

        let alias = std::ffi::CString::new("Ethernet0").unwrap();
        let oid = unsafe { rust_ports_orch_get_port_oid(alias.as_ptr()) };
        assert_eq!(oid, 0x1000000000001);

        let missing = std::ffi::CString::new("Ethernet999").unwrap();
        let oid = unsafe { rust_ports_orch_get_port_oid(missing.as_ptr()) };
        assert_eq!(oid, 0);

        unregister_rust_ports_orch();
    }
}
