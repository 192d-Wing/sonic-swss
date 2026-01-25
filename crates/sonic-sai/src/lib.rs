//! Safe Rust bindings for SAI (Switch Abstraction Interface).
//!
//! This crate provides type-safe wrappers around the SAI C API, preventing
//! common errors like mixing object IDs of different types and ensuring
//! proper error handling.
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`types`]: Core SAI types including type-safe object IDs
//! - [`error`]: Error types and status handling
//! - [`api`]: Safe wrappers around SAI API functions (port, route, acl, etc.)
//!
//! # Example
//!
//! ```ignore
//! use sonic_sai::{SaiContext, PortOid, SaiResult};
//!
//! fn configure_port(ctx: &SaiContext, port: PortOid) -> SaiResult<()> {
//!     // Type system prevents passing wrong OID type
//!     ctx.port_api().set_admin_status(port, true)?;
//!     ctx.port_api().set_speed(port, 100_000)?;
//!     Ok(())
//! }
//! ```

pub mod api;
pub mod error;
pub mod types;

// Re-export commonly used types
pub use types::{
    AclEntryKind, AclEntryOid, AclTableKind, AclTableOid, BridgeKind, BridgeOid, BridgePortKind,
    BridgePortOid, BufferPoolKind, BufferPoolOid, BufferProfileKind, BufferProfileOid,
    FdbEntryKind, FdbEntryOid, IngressPriorityGroupKind, IngressPriorityGroupOid, LagKind,
    LagMemberKind, LagMemberOid, LagOid, NeighborEntryKind, NeighborEntryOid, NextHopGroupKind,
    NextHopGroupMemberKind, NextHopGroupMemberOid, NextHopGroupOid, NextHopKind, NextHopOid,
    PortKind, PortOid, QueueKind, QueueOid, RouteEntryKind, RouteEntryOid, RouterInterfaceKind,
    RouterInterfaceOid, SaiObjectId, SaiObjectKind, SchedulerKind, SchedulerOid, SwitchKind,
    SwitchOid, VirtualRouterKind, VirtualRouterOid, VlanKind, VlanOid,
};

pub use error::{SaiError, SaiResult, SaiStatus};
