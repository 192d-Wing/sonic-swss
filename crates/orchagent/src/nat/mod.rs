//! NatOrch - NAT (Network Address Translation) orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The Rust implementation uses:
//! - Type-safe NAT types (Source/Destination/DoubleNat)
//! - Type-safe protocol enum (TCP/UDP/All)
//! - Composite NatEntryKey with all 5-tuple fields
//! - std::net::Ipv4Addr for IP addresses
//! - Validated IP and port ranges
//! - Option types for translated addresses/ports

mod ffi;
mod orch;
mod types;

pub use ffi::{register_nat_orch, unregister_nat_orch};
pub use orch::{NatOrch, NatOrchCallbacks, NatOrchConfig, NatOrchError, NatOrchStats};
pub use types::{
    NatAclEntry, NatAclKey, NatEntry, NatEntryConfig, NatEntryKey, NatPoolConfig, NatPoolEntry,
    NatPoolKey, NatProtocol, NatStats, NatType,
};
