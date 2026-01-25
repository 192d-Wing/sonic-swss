//! FFI exports for CounterCheckOrch.

use super::orch::{CounterCheckOrch, CounterCheckOrchConfig};
use std::cell::RefCell;

thread_local! {
    static COUNTERCHECK_ORCH: RefCell<Option<Box<CounterCheckOrch>>> = const { RefCell::new(None) };
}

#[no_mangle]
pub extern "C" fn register_countercheck_orch() -> bool {
    COUNTERCHECK_ORCH.with(|orch| {
        if orch.borrow().is_some() {
            return false;
        }
        *orch.borrow_mut() = Some(Box::new(CounterCheckOrch::new(
            CounterCheckOrchConfig::default(),
        )));
        true
    })
}

#[no_mangle]
pub extern "C" fn unregister_countercheck_orch() -> bool {
    COUNTERCHECK_ORCH.with(|orch| {
        if orch.borrow().is_none() {
            return false;
        }
        *orch.borrow_mut() = None;
        true
    })
}
