//! PortsOrch - Port orchestration for SONiC.
//!
//! This module manages physical and logical port configuration in SONiC, including:
//! - Physical port initialization and configuration
//! - Port state machine (CONFIG_MISSING → CONFIG_RECEIVED → CONFIG_DONE)
//! - Queue and scheduler configuration
//! - Port counters and statistics
//! - LAG (Link Aggregation Group) member management
//! - VLAN port membership
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation fixes several unsafe patterns from the C++ implementation:
//!
//! 1. **No auto-vivification**: Using `SyncMap` instead of `std::map` prevents
//!    silent creation of entries when accessing non-existent keys.
//!
//! 2. **No unprotected `.at()` calls**: The C++ implementation has 31+ `.at()`
//!    calls without try-catch, which can crash. Rust uses `Result` types.
//!
//! 3. **Owned data instead of raw pointers**: The C++ has 40+ raw `new` allocations.
//!    Rust uses owned types and smart pointers.
//!
//! 4. **Type-safe port types**: Uses enums instead of magic strings.
//!
//! # Architecture
//!
//! ```text
//! CONFIG_DB:PORT_TABLE
//!        │
//!        ▼
//!    PortsOrch
//!        │
//!        ├──> SAI Port API
//!        ├──> SAI Queue API
//!        ├──> SAI Scheduler API
//!        └──> STATE_DB:PORT_TABLE
//! ```

mod config;
mod ffi;
mod orch;
mod port;
mod queue;
mod types;

pub use config::{PortConfig, PortConfigError};
pub use ffi::{register_ports_orch, unregister_ports_orch};
pub use orch::{PortsOrch, PortsOrchCallbacks, PortsOrchConfig, PortsOrchError};
pub use port::{Port, PortType, PortRole, PortFecMode, PortAdminState, PortOperState};
pub use queue::{QueueInfo, QueueType, SchedulerInfo};
pub use types::{
    PortInitState, PortTable, LagTable, VlanTable, GearboxPortTable,
    SystemPortTable, PortSupportedSpeeds,
};
