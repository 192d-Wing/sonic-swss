//! FFI exports for IcmpOrch.

use std::cell::RefCell;
use super::orch::{IcmpOrch, IcmpOrchCallbacks, IcmpOrchConfig, Result};
use super::types::{IcmpRedirectConfig, NeighborDiscoveryConfig, IcmpStats};

/// FFI stub callbacks that do nothing (for C++ interop).
struct FfiIcmpCallbacks;

impl IcmpOrchCallbacks for FfiIcmpCallbacks {
    fn configure_icmp_redirect(&self, _config: &IcmpRedirectConfig) -> Result<()> {
        Ok(())
    }

    fn configure_neighbor_discovery(&self, _config: &NeighborDiscoveryConfig) -> Result<()> {
        Ok(())
    }

    fn process_redirect(&self, _src_ip: &str, _dst_ip: &str, _gateway_ip: &str) -> Result<()> {
        Ok(())
    }

    fn get_icmp_statistics(&self) -> Result<IcmpStats> {
        Ok(IcmpStats::default())
    }

    fn on_redirect_processed(&self, _src_ip: &str) {}
    fn on_neighbor_discovery_complete(&self, _neighbor_ip: &str) {}
}

thread_local! {
    static ICMP_ORCH: RefCell<Option<Box<IcmpOrch<FfiIcmpCallbacks>>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_icmp_orch() -> bool {
    ICMP_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(IcmpOrch::new(IcmpOrchConfig::default())));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_icmp_orch() -> bool {
    ICMP_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
