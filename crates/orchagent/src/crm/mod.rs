//! CrmOrch - Capacity Resource Management orchestration for SONiC.
//!
//! This module manages resource utilization monitoring for the network ASIC,
//! tracking used and available counters for various resource types and
//! triggering alerts when configurable thresholds are exceeded.
//!
//! # Architecture
//!
//! ```text
//! CONFIG_DB:CRM
//!      │
//!      ▼
//!   CrmOrch ───> SAI Switch API (resource queries)
//!      │
//!      ├──> COUNTERS_DB (CRM:STATS, ACL_STATS, etc.)
//!      ├──> Event system (threshold alerts)
//!      └──> Timer (periodic polling at 5-minute default)
//! ```
//!
//! # Resource Types
//!
//! CRM monitors 57+ resource types across categories:
//! - IP routing: IPv4/IPv6 routes
//! - Nexthops: IPv4/IPv6 nexthops, nexthop groups and members
//! - Neighbors: IPv4/IPv6 neighbors
//! - ACL: Tables, groups, entries, counters
//! - Forwarding: FDB entries, IPMC entries
//! - NAT: SNAT/DNAT entries
//! - MPLS: Label routes, nexthops
//! - SRv6: My SID entries, nexthops
//! - DASH (DPU): VNets, ENIs, routing, ACLs, meters
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation has several `.at()` calls that can throw exceptions:
//! - `m_resourcesMap.at(resourceType)` in multiple locations
//! - `crmThreshTypeMap.at(value)` without validation
//! - Map access creating auto-vivification bugs
//!
//! The Rust implementation uses `Option` and `Result` types to handle these
//! cases safely without exceptions, and explicit counter creation prevents
//! auto-vivification issues.

mod ffi;
mod orch;
mod types;

pub use ffi::{register_crm_orch, unregister_crm_orch};
pub use orch::{CrmOrch, CrmOrchCallbacks, CrmOrchConfig, CrmOrchError, CrmOrchStats};
pub use types::{
    crm_acl_key, crm_acl_table_key, crm_dash_acl_group_key, crm_ext_table_key, AclBindPoint,
    AclStage, CrmResourceCounter, CrmResourceEntry, CrmResourceStatus, CrmResourceType,
    CrmThresholdField, CrmThresholdType, ThresholdCheck, CRM_COUNTERS_TABLE_KEY,
    CRM_EXCEEDED_MSG_MAX, DEFAULT_HIGH_THRESHOLD, DEFAULT_LOW_THRESHOLD, DEFAULT_POLLING_INTERVAL,
};
