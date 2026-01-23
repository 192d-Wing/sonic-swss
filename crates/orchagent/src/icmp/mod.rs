//! IcmpOrch - ICMP echo (ping) response orchestration for SONiC.
//!
//! # Features
//!
//! The ICMP module provides:
//! - ICMP echo (ping) response handling
//! - ICMP redirect message management
//! - Neighbor discovery configuration
//! - Support for both IPv4 and IPv6
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Composite key (VRF + IP address)
//! - std::net::IpAddr for IPv4/IPv6 support
//! - Type-safe enable/disable mode
//! - Generic callbacks with Arc for thread safety
//! - Result<T> error handling pattern

mod ffi;
mod orch;
pub mod types;

pub use ffi::{register_icmp_orch, unregister_icmp_orch};
pub use orch::{IcmpOrch, IcmpOrchCallbacks, IcmpOrchConfig, IcmpOrchError, IcmpOrchStats, Result};
pub use types::{IcmpEchoEntry, IcmpEchoKey, IcmpMode, IcmpStats, IcmpRedirectConfig, NeighborDiscoveryConfig};
