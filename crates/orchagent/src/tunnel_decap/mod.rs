//! TunnelDecapOrch - Tunnel decapsulation orchestration for SONiC.
//!
//! # Safety Improvements over C++
//!
//! The C++ implementation (tunneldecaporch.cpp, 1,576 lines) has critical safety issues:
//! - CRITICAL: Unchecked pointer dereference after find() (line 1180) - CRASH if tunnel missing
//! - CRITICAL: Operator[] auto-creates entries silently (lines 116, 165, 257, 272)
//! - CRITICAL: Unchecked nested map access creates intermediate entries (lines 1285, 1290)
//! - HIGH: Potential dangling pointer after map reallocation (line 1309)
//! - HIGH: SAI failure ignored, resource counted anyway (line 1337)
//! - MEDIUM: Map iteration with modification and no rollback (lines 1141-1161)
//! - MEDIUM: String parsing without bounds validation
//!
//! The Rust implementation uses:
//! - Safe map access with `.get()` and `.entry()` API
//! - Result-based error propagation
//! - Type-safe mode enums (Uniform/Pipe, ECN modes)
//! - Validated reference counting (cannot underflow)
//! - Transactional updates with rollback
//! - Bounds-checked string parsing

mod ffi;
mod orch;
mod types;

pub use ffi::{register_tunnel_decap_orch, unregister_tunnel_decap_orch};
pub use orch::{
    TunnelDecapOrch, TunnelDecapOrchCallbacks, TunnelDecapOrchConfig, TunnelDecapOrchError,
    TunnelDecapOrchStats,
};
pub use types::{
    EcnMode, NexthopTunnel, SubnetType, TunnelConfig, TunnelDecapConfig, TunnelDecapEntry,
    TunnelEntry, TunnelMode, TunnelTermEntry, TunnelTermType,
};
