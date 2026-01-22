//! VRFOrch - Virtual Routing and Forwarding orchestration for SONiC.
//!
//! This module manages VRF (Virtual Routing and Forwarding) instances, enabling
//! network segmentation and multi-tenancy support. Each VRF maintains its own
//! routing table and can be associated with VXLAN VNI for overlay networking.
//!
//! # Architecture
//!
//! ```text
//! CONFIG_DB:VRF
//!      │
//!      ▼
//!   VRFOrch ───> SAI Virtual Router API
//!      │
//!      ├──> RouteOrch (per-VRF routing)
//!      ├──> IntfsOrch (VRF interface binding)
//!      └──> VxlanOrch (L3 VNI mapping)
//! ```
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation has several `.at()` calls that can throw exceptions:
//! - `vrf_table_.at(name)` in `getVRFid`, `increaseVrfRefCount`, etc.
//! - `vrf_id_table_.at(vrf_id)` in `getVRFname`
//! - `vrf_vni_map_table_.at(vrf_name)` in `getVRFmappedVNI`
//! - `l3vni_table_.at(vni)` in `getL3VniVlan`, `isL3VniVlan`
//!
//! The Rust implementation uses `Option` and `Result` types to handle these
//! cases safely without exceptions.

mod ffi;
mod orch;
mod types;

pub use ffi::{register_vrf_orch, unregister_vrf_orch};
pub use orch::{VrfOrch, VrfOrchCallbacks, VrfOrchConfig, VrfOrchError};
pub use types::{L3VniEntry, VrfEntry, VrfId, VrfName, VrfVlanId, Vni};
