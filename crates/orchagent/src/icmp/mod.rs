//! IcmpOrch - ICMP echo (ping) response orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Composite key (VRF + IP address)
//! - std::net::IpAddr for IPv4/IPv6 support
//! - Type-safe enable/disable mode

mod ffi;
mod orch;
mod types;

pub use ffi::{register_icmp_orch, unregister_icmp_orch};
pub use orch::{IcmpOrch, IcmpOrchCallbacks, IcmpOrchConfig, IcmpOrchError, IcmpOrchStats};
pub use types::{IcmpEchoEntry, IcmpEchoKey, IcmpMode, IcmpStats};
