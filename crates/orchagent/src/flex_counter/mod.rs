//! FlexCounterOrch - Flexible counter management for SONiC.
//!
//! This module manages the configuration and lifecycle of flexible counters
//! in SONiC. It handles:
//! - Counter group enable/disable
//! - Polling interval configuration
//! - Queue and PG counter state management
//! - Integration with PortsOrch for counter map generation

mod ffi;
mod group;
mod orch;
mod state;

pub use ffi::{register_flex_counter_orch, unregister_flex_counter_orch};
pub use group::{FlexCounterGroup, FlexCounterGroupMap};
pub use orch::{
    fields, FlexCounterCallbacks, FlexCounterError, FlexCounterOrch, FlexCounterOrchConfig,
};
pub use state::{
    FlexCounterPgStates, FlexCounterQueueStates, PgConfigurations, QueueConfigurations,
};
