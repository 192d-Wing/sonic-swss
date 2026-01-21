//! SONiC Orchagent - Switch Orchestration Daemon
//!
//! This is the Rust implementation of the SONiC orchagent, responsible for
//! orchestrating switch configuration by translating high-level intent from
//! Redis databases into SAI API calls.
//!
//! # Architecture
//!
//! The orchagent follows an event-driven architecture:
//!
//! ```text
//! [CONFIG_DB] ─┐
//!              ├──> [OrchDaemon] ──> [SAI Redis] ──> [syncd] ──> [ASIC]
//! [APPL_DB] ───┘        │
//!                       ↓
//!                 [STATE_DB]
//! ```
//!
//! # Key Components
//!
//! - [`daemon::OrchDaemon`]: Main event loop and Orch coordination
//! - [`orch`]: Individual orchestration modules (PortsOrch, RouteOrch, etc.)
//!
//! # Migration Status
//!
//! This crate is part of an ongoing migration from C++ to Rust. During the
//! migration period, it coexists with the C++ orchagent via FFI bridges.

pub mod daemon;
pub mod flex_counter;
pub mod orch;
pub mod route;

// Re-export commonly used types
pub use sonic_orch_common::{
    Consumer, ConsumerConfig, KeyOpFieldsValues, Operation,
    Orch, OrchContext, TaskStatus, TaskResult,
    SyncMap, RetryCache, Constraint,
};
pub use sonic_sai::{SaiError, SaiResult, PortOid, SwitchOid};
pub use sonic_types::{MacAddress, IpAddress, IpPrefix, VlanId};

// Re-export FlexCounterOrch and related types
pub use flex_counter::{
    FlexCounterCallbacks, FlexCounterError, FlexCounterGroup, FlexCounterGroupMap,
    FlexCounterOrch, FlexCounterOrchConfig, FlexCounterPgStates, FlexCounterQueueStates,
    PgConfigurations, QueueConfigurations, register_flex_counter_orch, unregister_flex_counter_orch,
};

// Re-export RouteOrch and related types
pub use route::{
    NextHopFlags, NextHopGroupEntry, NextHopGroupKey, NextHopGroupTable, NextHopKey,
    RouteEntry, RouteError, RouteKey, RouteNhg, RouteOrch, RouteOrchCallbacks, RouteOrchConfig,
    RouteTables, register_route_orch, unregister_route_orch,
};
