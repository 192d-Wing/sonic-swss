//! AclOrch - Access Control List orchestration for SONiC.
//!
//! This module manages ACL tables and rules in SONiC, including:
//! - ACL table creation with configurable match fields and actions
//! - ACL rule management with match conditions and actions
//! - Port binding for ingress/egress ACL enforcement
//! - Integration with mirror sessions, next-hops, and DTEL
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation fixes several unsafe patterns from C++:
//!
//! 1. **No unsafe `.at()` calls**: The C++ implementation has 12+ `.at()` calls
//!    without try-catch. Rust uses `Result` types for all lookups.
//!
//! 2. **No auto-vivification**: Using `SyncMap` instead of `std::map` prevents
//!    silent creation of entries when accessing non-existent keys.
//!
//! 3. **Type-safe enums**: ACL stages, table types, match fields, and actions
//!    are represented as type-safe enums instead of integer constants.
//!
//! 4. **Explicit error handling**: All operations that can fail return `Result`
//!    instead of silently failing or throwing exceptions.
//!
//! # Architecture
//!
//! ```text
//! CONFIG_DB:ACL_TABLE / ACL_RULE
//!        │
//!        ▼
//!    AclOrch
//!        │
//!        ├──> SAI ACL API
//!        ├──> MirrorOrch (for mirror rules)
//!        ├──> NeighOrch (for redirect to NH)
//!        └──> RouteOrch (for redirect to NHG)
//! ```
//!
//! # Key Components
//!
//! - [`AclTable`]: Represents an ACL table with match/action capabilities
//! - [`AclRule`]: Represents a rule within a table with match conditions and actions
//! - [`AclOrch`]: Main orchestrator managing tables and rules
//! - [`AclTableType`]: Defines table capabilities (matches, actions, bind points)

mod ffi;
mod orch;
mod range;
mod rule;
mod table;
mod table_type;
mod types;

pub use ffi::{register_acl_orch, unregister_acl_orch};
pub use orch::{AclOrch, AclOrchCallbacks, AclOrchConfig, AclOrchError};
pub use range::{AclRange, AclRangeType};
pub use rule::{AclRule, AclRuleAction, AclRuleMatch, AclRuleType};
pub use table::{AclTable, AclTableConfig};
pub use table_type::{AclTableType, AclTableTypeBuilder};
pub use types::{
    AclActionType, AclBindPointType, AclMatchField, AclPacketAction, AclStage,
    AclTableId, AclRuleId, AclPriority, MetaDataValue,
};
